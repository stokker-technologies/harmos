/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Historical Reads Decoded from Stored Bytes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::ObjectSelection;
use super::reader::query_slot;
use crate::backend::ScanPage;
use crate::manager::Inner;
use crate::selection::Entry;
use crate::snapshot::Snapshot;
use crate::{Error, MemorySize, Message, ReadBatch, Result};
use crate::{StorageCursor, StorageManager, StoredObject};
use harmos_domain::model::storage::catalog::object_schema;
use harmos_domain::model::storage::object_key;
use harmos_query::batches::QueryFuture;
use harmos_query::typed::{ObjectRead, ObjectReader, PageSource, RecordRead, RecordReader, StreamRead, StreamReader};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

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

impl StorageManager {
    #[tracing::instrument(name = "storage.manager.list_object_revisions", level = "trace", skip_all)]
    pub async fn list_object_revisions<T: StoredObject>(&self, read: ObjectRead<'_, T>) -> Result<ObjectReader<T>> {
        self.inner.catalog.handle::<T>()?;
        let schema = object_schema::<T>()?;
        let key = read.key.map(|key| object_key(key, self.inner.settings.scan_bytes())).transpose()?;
        let selection = ObjectSelection {
            schema: Some(schema.clone()),
            key,
            versions: read.versions,
        };
        let pages = Pages::open(self, read.at, selection).await?;
        Ok(ObjectReader::new(Box::new(pages), schema))
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.manager.list_records", level = "trace", skip_all)]
    pub async fn list_records<T: Message + MemorySize>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        super::Source::Records(read.selection.clone()).validate()?;

        let mut pages = Pages::open(self, read.at, read.selection).await?;
        pages.after = read.after;
        Ok(RecordReader::new(Box::new(pages)))
    }
}

impl StorageManager {
    pub async fn list_samples(&self, read: StreamRead) -> Result<StreamReader> {
        super::Source::Stream(read.selection.clone()).validate()?;

        let after = read.selection.after;
        let mut pages = Pages::open(self, read.at, read.selection).await?;
        pages.after = after;
        Ok(StreamReader::new(Box::new(pages)))
    }
}

impl<E: Entry> Pages<E>
where
    E::Selection: MemorySize,
{
    #[tracing::instrument(name = "storage.query.typed.open", level = "trace", skip_all)]
    async fn open(manager: &StorageManager, at: StorageCursor, selection: E::Selection) -> Result<Self> {
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
                return Err(Error::NotReady {
                    domain: "failed or interrupted typed storage read",
                });
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
        let page = ScanPage {
            after: self.after,
            rows: self.rows,
            bytes,
        };
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

impl<E: Entry + MemorySize> PageSource<E> for Pages<E> {
    fn next(&mut self) -> QueryFuture<'_, Option<ReadBatch<Vec<E>>>> {
        Box::pin(async move {
            Pages::next(self, Ok)
                .await
                .map_err(|error| harmos_query::Error::backend("read typed source", harmos_query::BackendFailure::Permanent, error))
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
