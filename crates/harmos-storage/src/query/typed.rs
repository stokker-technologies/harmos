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
use super::reader::{query_slot, retry};
use super::{ObjectSelection, ObjectVersions, RecordSelection};
use crate::storage::backend::ScanPage;
use crate::storage::capacity::{Held, Pool, Refusal, Reservation};
use crate::storage::manager::{ENTRY_OVERHEAD, Inner};
use crate::storage::memory::MemoryBank;
use crate::storage::selection::Entry;
use crate::storage::snapshot::Snapshot;
use crate::storage::{ObjectCatalog, ObjectEntry, ObjectRevision, SchemaKey, StorageCursor, StorageManager, StoredObject, StoredRecord};
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
    held: Held,
    selection: E::Selection,
    snapshot: Option<Snapshot>,
    active: Option<OwnedSemaphorePermit>,
    _description: Reservation,
    after: u64,
    rows: NonZeroUsize,
    failed: bool,
    inflight: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: ObjectCatalog> StorageManager<C> {
    #[tracing::instrument(name = "storage.query.typed.read_objects", level = "trace", skip_all)]
    pub async fn read_objects<T: StoredObject>(&self, read: ObjectRead<'_, T>) -> Result<ObjectReader<T>>
    where
        C: Contains<T>,
    {
        crate::ReadSession::standalone(self).read_objects(read).await
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.query.typed.read_records", level = "trace", skip_all)]
    pub async fn read_records<T: Message + MemorySize>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        crate::ReadSession::standalone(self).read_records(read).await
    }
}

impl<C: ObjectCatalog> crate::ReadSession<C> {
    #[tracing::instrument(name = "storage.read_session.read_objects", level = "trace", skip_all)]
    pub async fn read_objects<T: StoredObject>(&self, read: ObjectRead<'_, T>) -> Result<ObjectReader<T>>
    where
        C: Contains<T>,
    {
        let schema = object_schema::<T>()?;
        let key = read.key.map(|key| object_key(key, self.manager.inner.settings.scan_bytes())).transpose()?;
        let selection = ObjectSelection { schema: Some(schema.clone()), key, versions: read.versions };
        let pages = Pages::open(&self.manager, &self.held, self.queued, read.at, selection).await?;
        Ok(ObjectReader { pages, schema, _value: PhantomData })
    }
}

impl<C> crate::ReadSession<C> {
    #[tracing::instrument(name = "storage.read_session.read_records", level = "trace", skip_all)]
    pub async fn read_records<T: Message + MemorySize>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        super::Source::Records(read.selection.clone()).validate()?;
        read.selection.schema.require_postcard()?;
        let mut pages = Pages::open(&self.manager, &self.held, self.queued, read.at, read.selection).await?;
        pages.after = read.after;
        Ok(RecordReader { pages, _payload: PhantomData })
    }
}

impl<T: StoredObject> ObjectReader<T> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    pub async fn next(&mut self) -> Result<Option<ReadBatch<Vec<ObjectRevision<T>>>>> {
        let schema = &self.schema;
        self.pages
            .next(
                |entry| decoding::<T>(&entry.value.payload),
                |entry| {
                    Ok(ObjectRevision {
                        value: decode_admitted::<T>(&entry.value, schema)?,
                        position: entry.position,
                        version: entry.version,
                    })
                },
            )
            .await
    }

    #[tracing::instrument(name = "storage.query.typed.collect", level = "trace", skip_all)]
    pub async fn collect(mut self) -> Result<crate::ReadPages<ObjectRevision<T>>> {
        let mut collection =
            Reservation::wait(&self.pages.inner, Pool::Read, &self.pages.held, ENTRY_OVERHEAD, None).await.map_err(Refusal::into_error)?;
        let mut pages = Vec::new();
        while let Some(page) = self.next().await? {
            let bytes = pages
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(4 * std::mem::size_of_val(&page)))
                .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD))
                .ok_or(Error::ResourceExhausted { resource: "typed page collection" })?;
            collection.grow(bytes.saturating_sub(collection.bytes()), None).await.map_err(Refusal::into_error)?;
            pages.push(page);
        }
        Ok(ReadBatch::new(pages, collection))
    }
}

impl<T: Message + MemorySize> RecordReader<T> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    pub async fn next(&mut self) -> Result<Option<ReadBatch<Vec<StoredRecord<T>>>>> {
        self.pages
            .next(
                |record| decoding::<T>(&record.payload),
                |record| {
                    Ok(StoredRecord {
                        payload: T::decode(&record.payload)
                            .map_err(|error| Error::backend("decode record", BackendFailure::Corrupt, error))?,
                        position: record.position,
                        schema: record.schema,
                        scope: record.scope,
                        item: record.item,
                        source: record.source,
                        source_time: record.source_time,
                    })
                },
            )
            .await
    }

    #[tracing::instrument(name = "storage.query.typed.collect", level = "trace", skip_all)]
    pub async fn collect(mut self) -> Result<crate::ReadPages<StoredRecord<T>>> {
        let mut collection =
            Reservation::wait(&self.pages.inner, Pool::Read, &self.pages.held, ENTRY_OVERHEAD, None).await.map_err(Refusal::into_error)?;
        let mut pages = Vec::new();
        while let Some(page) = self.next().await? {
            let bytes = pages
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(4 * std::mem::size_of_val(&page)))
                .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD))
                .ok_or(Error::ResourceExhausted { resource: "typed page collection" })?;
            collection.grow(bytes.saturating_sub(collection.bytes()), None).await.map_err(Refusal::into_error)?;
            pages.push(page);
        }
        Ok(ReadBatch::new(pages, collection))
    }
}

impl<E: Entry> Pages<E>
where
    E::Selection: MemorySize,
{
    #[tracing::instrument(name = "storage.query.typed.open", level = "trace", skip_all)]
    async fn open<C>(manager: &StorageManager<C>, held: &Held, queued: bool, at: StorageCursor, selection: E::Selection) -> Result<Self> {
        let inner = &manager.inner;
        let active = if queued {
            query_slot(inner).await?
        } else {
            Arc::clone(&inner.query_active).try_acquire_owned().map_err(|error| match error {
                tokio::sync::TryAcquireError::Closed => Error::Closing { domain: "storage queries" },
                tokio::sync::TryAcquireError::NoPermits => Error::ResourceExhausted { resource: "active storage queries" },
            })?
        };
        let held = held.clone();
        let description = selection.memory_size().saturating_add(ENTRY_OVERHEAD);
        let description = Reservation::wait(inner, Pool::Read, &held, description, None).await.map_err(Refusal::into_error)?;
        let snapshot = manager.snapshot(&at, &held).await?;
        Ok(Self {
            context: crate::execution::Context::current(),
            inner: Arc::clone(inner),
            held,
            selection,
            snapshot: Some(snapshot),
            active: Some(active),
            _description: description,
            after: 0,
            rows: inner.settings.response.batch_rows,
            failed: false,
            inflight: false,
        })
    }
}

impl<E: Entry> Pages<E> {
    #[tracing::instrument(name = "storage.query.typed.next", level = "trace", skip_all)]
    async fn next<V: MemorySize>(
        &mut self, memory: impl Fn(&E) -> Result<usize>, build: impl Fn(E) -> Result<V>,
    ) -> Result<Option<ReadBatch<Vec<V>>>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            if self.failed || self.inflight {
                self.fail();
                return Err(Error::NotReady { domain: "failed or interrupted typed storage read" });
            }
            self.inflight = true;
            let result = self.advance(&memory, &build).await;
            self.inflight = false;
            if result.is_err() {
                self.fail();
            }
            result
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    #[tracing::instrument(name = "storage.query.typed.advance", level = "trace", skip_all)]
    async fn advance<V: MemorySize>(
        &mut self, memory: &impl Fn(&E) -> Result<usize>, build: &impl Fn(E) -> Result<V>,
    ) -> Result<Option<ReadBatch<Vec<V>>>> {
        while self.snapshot.is_some() {
            let mut credit = 0usize;
            let page = loop {
                let bank = match MemoryBank::admit(&self.inner, &self.held, credit, None).await {
                    Ok(bank) => bank,
                    Err(Refusal::Exceeded(_)) if self.rows.get() > 1 => {
                        self.rows = NonZeroUsize::new(self.rows.get() / 2).unwrap();
                        credit = 0;
                        continue;
                    }
                    Err(refusal) => return Err(refusal.into_error()),
                };
                match self.attempt(&bank, memory, build).await {
                    Ok(page) => break page,
                    Err(error) => {
                        let mut rows = self.rows;
                        credit = retry(&bank, error, Some(&mut rows))?;
                        self.rows = rows;
                    }
                }
            };
            match page {
                None => self.release(),
                Some(values) if values.is_empty() => {}
                Some(values) => return Ok(Some(values)),
            }
        }
        Ok(None)
    }

    // Scan And Decode One Page; State Advances Only After Every Claim Succeeded.
    #[tracing::instrument(name = "storage.query.typed.attempt", level = "trace", skip_all)]
    async fn attempt<V: MemorySize>(
        &mut self, bank: &MemoryBank, memory: &impl Fn(&E) -> Result<usize>, build: &impl Fn(E) -> Result<V>,
    ) -> Result<Option<ReadBatch<Vec<V>>>> {
        let bytes = self.inner.settings.scan_bytes().min(self.inner.settings.response.batch_bytes);
        let page = ScanPage { after: self.after, rows: self.rows, bytes };
        let mut raw = bank.scope();
        raw.claim(self.rows.get().saturating_mul(ENTRY_OVERHEAD).saturating_add(bytes.get()))?;
        let snapshot = self.snapshot.as_ref().expect("running reader owns its snapshot");
        let raw_memory = raw.retain();
        let (entries, size) = snapshot.scan::<E>(&self.selection, page, raw_memory.clone()).await?;
        let mut raw = bank.scope();
        // A Single Oversized Entry Is Charged At Its Actual Size.
        raw.claim(size.saturating_sub(raw_memory.bytes()))?;
        let Some(last) = entries.last().map(|entry| entry.position().sequence) else {
            return Ok(None);
        };
        let mut decoded = bank.scope();
        for entry in &entries {
            decoded.claim(memory(entry)?)?;
        }
        let values = entries.into_iter().map(build).collect::<Result<Vec<_>>>()?;
        drop(decoded);
        drop(raw);
        drop(raw_memory);
        let bytes =
            values.memory_size().checked_add(ENTRY_OVERHEAD).ok_or(Error::ResourceExhausted { resource: "typed page ownership" })?;
        let memory = bank.reserve(bytes)?;
        self.after = last;
        Ok(Some(ReadBatch::retained(values, memory)))
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

fn decoding<T: Message>(payload: &[u8]) -> Result<usize> {
    T::decoding_memory(payload).map_err(|error| Error::backend("preflight payload", BackendFailure::Corrupt, error))
}

/* === Tests === */
