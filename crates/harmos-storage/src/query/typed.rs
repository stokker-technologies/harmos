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
use crate::{BackendFailure, Error, MemorySize, Message, Result};
use harmos_domain::catalog::Contains;
use harmos_domain::model::storage::catalog::{decode_admitted, object_schema};
use harmos_domain::model::storage::object_key;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A typed read of one object type's saved revisions.
pub struct ObjectRead<'a, T: StoredObject> {
    pub at: StorageCursor,
    pub key: Option<&'a T::Key>,
    pub versions: ObjectVersions,
}

/// A typed read of one record schema's saved records after a storage position.
pub struct RecordRead {
    pub at: StorageCursor,
    pub selection: RecordSelection,
    pub after: u64,
}

/// Delivers bounded pages of decoded object revisions at one saved cursor.
pub struct ObjectReader<T> {
    pages: Pages<ObjectEntry>,
    schema: SchemaKey,
    _value: PhantomData<fn() -> T>,
}

/// Delivers bounded pages of decoded records at one saved cursor.
pub struct RecordReader<T> {
    pages: Pages<StoredRecord>,
    _payload: PhantomData<fn() -> T>,
}

// Scan State Shared By Both Readers. The Active Slot And Snapshot Release Once
// The Source Is Exhausted; Dropping An In-Progress `next` Poisons The Reader.
struct Pages<E: Entry> {
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
    /// Read saved revisions of `T`, latest or complete, decoded from their
    /// stored bytes without Arrow payload expansion.
    /// A type whose schema was never written reads as no revisions.
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
    /// Read saved records of one postcard schema after `read.after`, decoded as
    /// `T`. A schema that was never written reads as no records.
    pub async fn read_records<T: Message>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        super::Source::Records(read.selection.clone()).validate()?;
        read.selection.schema.require_postcard()?;
        let mut pages = Pages::open(self, read.at, read.selection).await?;
        pages.after = read.after;
        Ok(RecordReader { pages, _payload: PhantomData })
    }
}

impl<T: StoredObject> ObjectReader<T> {
    /// The next nonempty page within the response row bound, or `None` once exhausted.
    pub async fn next(&mut self) -> Result<Option<Vec<ObjectRevision<T>>>> {
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

    /// Every remaining revision, for selections bounded by their domain.
    pub async fn collect(mut self) -> Result<Vec<ObjectRevision<T>>> {
        let mut values = Vec::new();
        while let Some(page) = self.next().await? {
            values.extend(page);
        }
        Ok(values)
    }
}

impl<T: Message> RecordReader<T> {
    /// The next nonempty page within the response row bound, or `None` once exhausted.
    pub async fn next(&mut self) -> Result<Option<Vec<StoredRecord<T>>>> {
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

    /// Every remaining record, for selections bounded by their domain.
    pub async fn collect(mut self) -> Result<Vec<StoredRecord<T>>> {
        let mut values = Vec::new();
        while let Some(page) = self.next().await? {
            values.extend(page);
        }
        Ok(values)
    }
}

impl<E: Entry> Pages<E>
where
    E::Selection: MemorySize,
{
    async fn open<C>(manager: &StorageManager<C>, at: StorageCursor, selection: E::Selection) -> Result<Self> {
        let inner = &manager.inner;
        let active = query_slot(inner).await?;
        let held = Held::default();
        let description = selection.memory_size().saturating_add(ENTRY_OVERHEAD);
        let description = Reservation::wait(inner, Pool::Read, &held, description, None).await.map_err(Refusal::into_error)?;
        let snapshot = manager.snapshot(&at).await?;
        Ok(Self {
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
    async fn next<V>(&mut self, memory: impl Fn(&E) -> Result<usize>, build: impl Fn(E) -> Result<V>) -> Result<Option<Vec<V>>> {
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
    }

    async fn advance<V>(&mut self, memory: &impl Fn(&E) -> Result<usize>, build: &impl Fn(E) -> Result<V>) -> Result<Option<Vec<V>>> {
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
    async fn attempt<V>(
        &mut self, bank: &MemoryBank, memory: &impl Fn(&E) -> Result<usize>, build: &impl Fn(E) -> Result<V>,
    ) -> Result<Option<Vec<V>>> {
        let bytes = self.inner.settings.scan_bytes().min(self.inner.settings.response.batch_bytes);
        let page = ScanPage { after: self.after, rows: self.rows, bytes };
        let mut raw = bank.scope();
        raw.claim(self.rows.get().saturating_mul(ENTRY_OVERHEAD).saturating_add(bytes.get()))?;
        let snapshot = self.snapshot.as_ref().expect("running reader owns its snapshot");
        let (entries, size) = snapshot.scan::<E>(&self.selection, page).await?;
        // A Single Oversized Entry Is Charged At Its Actual Size.
        raw.claim(size.saturating_sub(bytes.get()))?;
        let Some(last) = entries.last().map(|entry| entry.position().sequence) else {
            return Ok(None);
        };
        let mut decoded = bank.scope();
        for entry in &entries {
            decoded.claim(memory(entry)?)?;
        }
        let values = entries.into_iter().map(build).collect::<Result<Vec<_>>>()?;
        self.after = last;
        Ok(Some(values))
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
