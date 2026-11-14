/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use crate::batches::QueryFuture;
use crate::{BackendFailure, Error, Result};
use harmos_domain::model::storage::catalog::decode_admitted;
use harmos_domain::model::storage::query::{ObjectVersions, RecordSelection, StreamSelection};
use harmos_domain::model::storage::{
    ObjectEntry, ObjectRevision, ReadBatch, ReadPages, SchemaKey, StorageCursor, StoredObject, StoredRecord, StoredRow,
};
use harmos_domain::{MemorySize, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ObjectRead<'a, T: StoredObject> {
    pub at: StorageCursor,
    pub key: Option<&'a T::Key>,
    pub versions: ObjectVersions,
}

pub struct RecordRead {
    pub at: StorageCursor,
    pub selection: RecordSelection,
    pub after: u64,
}

pub struct StreamRead {
    pub at: StorageCursor,
    pub selection: StreamSelection,
}

pub struct StreamReader {
    pages: DecodedPages<StoredRow>,
}

pub struct ObjectReader<T> {
    pages: DecodedPages<ObjectEntry>,
    schema: SchemaKey,
    _value: PhantomData<fn() -> T>,
}

pub struct RecordReader<T> {
    pages: DecodedPages<StoredRecord>,
    _payload: PhantomData<fn() -> T>,
}

struct DecodedPages<E> {
    source: Option<Box<dyn PageSource<E>>>,
    failed: bool,
}

/* === Enums === */

/* === Traits === */

pub trait PageSource<E>: Send {
    fn next(&mut self) -> QueryFuture<'_, Option<ReadBatch<Vec<E>>>>;
}

/* === Implementations === */

impl<E> DecodedPages<E> {
    fn new(source: Box<dyn PageSource<E>>) -> Self {
        Self { source: Some(source), failed: false }
    }
    async fn next<V: MemorySize>(&mut self, build: impl Fn(E) -> Result<V>) -> Result<Option<ReadBatch<Vec<V>>>> {
        if self.failed {
            return Err(Error::NotReady { domain: "read failed typed query" });
        }
        let Some(source) = &mut self.source else {
            return Ok(None);
        };
        self.failed = true;
        let page = source.next().await?;
        let Some(page) = page else {
            self.source = None;
            self.failed = false;
            return Ok(None);
        };
        let values = page.try_map(|entries| entries.into_iter().map(build).collect::<Result<Vec<_>>>())?;
        self.failed = false;
        Ok(Some(values))
    }
}

impl StreamReader {
    pub fn new(source: Box<dyn PageSource<StoredRow>>) -> Self {
        Self { pages: DecodedPages::new(source) }
    }
}

impl<T> ObjectReader<T> {
    pub fn new(source: Box<dyn PageSource<ObjectEntry>>, schema: SchemaKey) -> Self {
        Self {
            pages: DecodedPages::new(source),
            schema,
            _value: PhantomData,
        }
    }
}

impl<T> RecordReader<T> {
    pub fn new(source: Box<dyn PageSource<StoredRecord>>) -> Self {
        Self {
            pages: DecodedPages::new(source),
            _payload: PhantomData,
        }
    }
}

impl StreamReader {
    pub async fn next(&mut self) -> Result<Option<ReadBatch<Vec<StoredRow>>>> {
        self.pages.next(Ok).await
    }
}

impl<T: StoredObject> ObjectReader<T> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    pub async fn next(&mut self) -> Result<Option<ReadBatch<Vec<ObjectRevision<T>>>>> {
        let schema = &self.schema;
        self.pages
            .next(|entry| {
                Ok(ObjectRevision {
                    value: decode_admitted::<T>(&entry.value, schema)?,
                    position: entry.position,
                    version: entry.version,
                })
            })
            .await
    }

    #[tracing::instrument(name = "storage.query.typed.collect", level = "trace", skip_all)]
    pub async fn collect(mut self) -> Result<ReadPages<ObjectRevision<T>>> {
        let mut pages = Vec::new();
        while let Some(page) = self.next().await? {
            pages.try_reserve(1).map_err(|_| Error::ResourceExhausted { resource: "typed page collection" })?;
            pages.push(page);
        }
        Ok(ReadBatch::new(pages))
    }
}

impl<T: Message + MemorySize> RecordReader<T> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    pub async fn next(&mut self) -> Result<Option<ReadBatch<Vec<StoredRecord<T>>>>> {
        self.pages
            .next(|record| {
                Ok(StoredRecord {
                    payload: T::decode(&record.payload).map_err(|error| Error::backend("decode record", BackendFailure::Corrupt, error))?,
                    position: record.position,
                    schema: record.schema,
                    scope: record.scope,
                    item: record.item,
                    source: record.source,
                    source_time: record.source_time,
                })
            })
            .await
    }

    #[tracing::instrument(name = "storage.query.typed.collect", level = "trace", skip_all)]
    pub async fn collect(mut self) -> Result<ReadPages<StoredRecord<T>>> {
        let mut pages = Vec::new();
        while let Some(page) = self.next().await? {
            pages.try_reserve(1).map_err(|_| Error::ResourceExhausted { resource: "typed page collection" })?;
            pages.push(page);
        }
        Ok(ReadBatch::new(pages))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
