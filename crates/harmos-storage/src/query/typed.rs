/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Historical Reads Decoded From Stored Bytes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::reader::query_slot;
use super::{ObjectSelection, ObjectVersions, RecordSelection, StreamSelection};
use crate::storage::backend::ScanPage;
use crate::storage::manager::Inner;
use crate::storage::selection::Entry;
use crate::storage::snapshot::Snapshot;
use crate::storage::{
    ObjectCatalog, ObjectEntry, ObjectRevision, SchemaKey, StorageCursor, StorageManager, StoredObject, StoredRecord, StoredRow,
};
use crate::{BackendFailure, Error, MemorySize, Message, ReadBatch, Result};
use harmos_domain::catalog::Contains;
use harmos_domain::model::storage::catalog::{decode_admitted, object_schema};
use harmos_domain::model::storage::object_key;

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
    pages: Pages<StoredRow>,
}

pub struct ObjectReader<T> {
    pages: Pages<ObjectEntry>,
    schema: SchemaKey,
    _value: PhantomData<fn() -> T>,
}

pub struct RecordReader<T> {
    pages: Pages<StoredRecord>,
    _payload: PhantomData<fn() -> T>,
}

// Scan State Shared By Both Readers. The Active Slot And Snapshot Release Once
// The Source Is Exhausted; Dropping An In-Progress `next` Poisons The Reader.
struct Pages<E: Entry> {
    context: crate::execution::Context,
    inner: Arc<Inner>,
    selection: E::Selection,
    snapshot: Option<Snapshot>,
    active: Option<OwnedSemaphorePermit>,
    after: u64,
    rows: NonZeroUsize,
    failed: bool,
    inflight: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: ObjectCatalog> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.read_objects", level = "trace", skip_all)]
    pub async fn read_objects<T: StoredObject>(&self, read: ObjectRead<'_, T>) -> Result<ObjectReader<T>>
    where
        C: Contains<T>,
    {
        let schema = object_schema::<T>()?;
        let key = read.key.map(|key| object_key(key, self.inner.settings.scan_bytes())).transpose()?;
        let selection = ObjectSelection { schema: Some(schema.clone()), key, versions: read.versions };
        let pages = Pages::open(self, read.at, selection).await?;
        Ok(ObjectReader { pages, schema, _value: PhantomData })
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.read_records", level = "trace", skip_all)]
    pub async fn read_records<T: Message + MemorySize>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        super::Source::Records(read.selection.clone()).validate()?;
        read.selection.schema.require_postcard()?;
        let mut pages = Pages::open(self, read.at, read.selection).await?;
        pages.after = read.after;
        Ok(RecordReader { pages, _payload: PhantomData })
    }
}

impl<C> StorageManager<C> {
    pub async fn read_stream(&self, read: StreamRead) -> Result<StreamReader> {
        super::Source::Stream(read.selection.clone()).validate()?;
        read.selection.schema.require_postcard()?;
        let after = read.selection.after;
        let mut pages = Pages::open(self, read.at, read.selection).await?;
        pages.after = after;
        Ok(StreamReader { pages })
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
                Ok(ObjectRevision { value: decode_admitted::<T>(&entry.value, schema)?, position: entry.position, version: entry.version })
            })
            .await
    }

    #[tracing::instrument(name = "storage.query.typed.collect", level = "trace", skip_all)]
    pub async fn collect(mut self) -> Result<crate::ReadPages<ObjectRevision<T>>> {
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
    pub async fn collect(mut self) -> Result<crate::ReadPages<StoredRecord<T>>> {
        let mut pages = Vec::new();
        while let Some(page) = self.next().await? {
            pages.try_reserve(1).map_err(|_| Error::ResourceExhausted { resource: "typed page collection" })?;
            pages.push(page);
        }
        Ok(ReadBatch::new(pages))
    }
}

impl<E: Entry> Pages<E>
where
    E::Selection: MemorySize,
{
    #[tracing::instrument(name = "storage.query.typed.open", level = "trace", skip_all)]
    async fn open<C>(manager: &StorageManager<C>, at: StorageCursor, selection: E::Selection) -> Result<Self> {
        let inner = &manager.inner;
        let active = query_slot(inner).await?;
        let snapshot = manager.snapshot(&at).await?;
        Ok(Self {
            context: crate::execution::Context::current(),
            inner: Arc::clone(inner),
            selection,
            snapshot: Some(snapshot),
            active: Some(active),
            after: 0,
            rows: inner.settings.response.batch_rows,
            failed: false,
            inflight: false,
        })
    }
}

impl<E: Entry> Pages<E> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    async fn next<V: MemorySize>(&mut self, build: impl Fn(E) -> Result<V>) -> Result<Option<ReadBatch<Vec<V>>>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            if self.failed || self.inflight {
                self.fail();
                return Err(Error::NotReady { domain: "failed or interrupted typed storage read" });
            }
            self.inflight = true;
            let result = self.advance(&build).await;
            self.inflight = false;
            if result.is_err() {
                self.fail();
            }
            result
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    #[tracing::instrument(name = "storage.query.typed.advance", level = "trace", skip_all)]
    async fn advance<V: MemorySize>(&mut self, build: &impl Fn(E) -> Result<V>) -> Result<Option<ReadBatch<Vec<V>>>> {
        while self.snapshot.is_some() {
            match self.attempt(build).await? {
                None => self.release(),
                Some(values) if values.is_empty() => {}
                Some(values) => return Ok(Some(values)),
            }
        }
        Ok(None)
    }

    // Advance Only After The Complete Page Decodes.
    #[tracing::instrument(name = "storage.query.typed.attempt", level = "trace", skip_all)]
    async fn attempt<V: MemorySize>(&mut self, build: &impl Fn(E) -> Result<V>) -> Result<Option<ReadBatch<Vec<V>>>> {
        let bytes = self.inner.settings.scan_bytes().min(self.inner.settings.response.batch_bytes);
        let page = ScanPage { after: self.after, rows: self.rows, bytes };
        let snapshot = self.snapshot.as_ref().expect("running reader owns its snapshot");
        let (entries, _) = snapshot.scan::<E>(&self.selection, page).await?;
        let Some(last) = entries.last().map(|entry| entry.position().sequence) else {
            return Ok(None);
        };
        let mut values = Vec::new();
        values.try_reserve_exact(entries.len()).map_err(|_| Error::ResourceExhausted { resource: "typed page" })?;
        for entry in entries {
            values.push(build(entry)?);
        }
        self.after = last;
        Ok(Some(ReadBatch::new(values)))
    }

    fn fail(&mut self) {
        self.failed = true;
        self.release();
    }

    fn release(&mut self) {
        self.snapshot = None;
        self.active = None;
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
