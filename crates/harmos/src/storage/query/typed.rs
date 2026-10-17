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
use crate::storage::query::transform::FilterExecution;
// Standard Library Imports
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::reader::{find_schema, query_slot, retry};
use super::transform::filter::Filter;
use super::{ObjectSelection, ObjectVersions, RecordSelection};
use crate::catalog::Contains;
use crate::storage::arrow::{RetainedMemory, resident_bytes};
use crate::storage::backend::ScanPage;
use crate::storage::capacity::{Held, Pool, Refusal, Reservation};
use crate::storage::catalog::{decode_admitted, object_key, object_schema};
use crate::storage::manager::{ENTRY_OVERHEAD, Inner};
use crate::storage::memory::MemoryBank;
use crate::storage::selection::Entry;
use crate::storage::snapshot::Snapshot;
use crate::storage::{
    ObjectCatalog, ObjectEntry, ObjectRevision, SchemaKey, StorageCursor, StorageManager,
    StorageSchema, StoredObject, StoredRecord,
};
use crate::{BackendFailure, Error, MemorySize, Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// A typed read of one object type's saved revisions.
pub struct ObjectRead<'a, T: StoredObject> {
    pub at: StorageCursor,
    pub key: Option<&'a T::Key>,
    pub versions: ObjectVersions,
    /// Conjunctive filters over the query source columns (`_storage.*`, `payload.*`).
    pub filters: Vec<Filter>,
}

/// A typed read of one record schema's saved records after a storage position.
pub struct RecordRead {
    pub at: StorageCursor,
    pub selection: RecordSelection,
    pub after: u64,
    /// Conjunctive filters over the query source columns (`_storage.*`, `payload.*`).
    pub filters: Vec<Filter>,
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
    key: SchemaKey,
    filters: Vec<Filter>,
    schema: Option<(StorageSchema, RetainedMemory)>,
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
// Native Entries With An Arrow Source Layout For Filter Evaluation.
trait Filtered: Entry {
    fn arrow(schema: &StorageSchema, rows: &[Self], bank: &MemoryBank) -> Result<RecordBatch>;
}

/* === Implementations === */
impl<C: ObjectCatalog> StorageManager<C> {
    /// Read saved revisions of `T`, latest or complete, decoded from their
    /// stored bytes without Arrow payload expansion unless filters are given.
    /// A type whose schema was never written reads as no revisions.
    pub async fn read_objects<T: StoredObject>(
        &self,
        read: ObjectRead<'_, T>,
    ) -> Result<ObjectReader<T>>
    where
        C: Contains<T>,
    {
        let schema = object_schema::<T>()?;
        let key = read
            .key
            .map(|key| object_key(key, self.inner.settings.scan_bytes()))
            .transpose()?;
        let selection = ObjectSelection {
            schema: Some(schema.clone()),
            key,
            versions: read.versions,
        };
        let pages = Pages::open(self, read.at, selection, schema.clone(), read.filters).await?;
        Ok(ObjectReader {
            pages,
            schema,
            _value: PhantomData,
        })
    }
}

impl<C> StorageManager<C> {
    /// Read saved records of one postcard schema after `read.after`, decoded as
    /// `T`. A schema that was never written reads as no records.
    pub async fn read_records<T: Message>(&self, read: RecordRead) -> Result<RecordReader<T>> {
        super::Source::Records(read.selection.clone()).validate()?;
        read.selection.schema.require_postcard()?;
        let key = read.selection.schema.clone();
        let mut pages = Pages::open(self, read.at, read.selection, key, read.filters).await?;
        pages.after = read.after;
        Ok(RecordReader {
            pages,
            _payload: PhantomData,
        })
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
                        payload: T::decode(&record.payload).map_err(|error| {
                            Error::backend("decode record", BackendFailure::Corrupt, error)
                        })?,
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

impl<E: Filtered> Pages<E>
where
    E::Selection: MemorySize,
{
    async fn open<C>(
        manager: &StorageManager<C>,
        at: StorageCursor,
        selection: E::Selection,
        key: SchemaKey,
        filters: Vec<Filter>,
    ) -> Result<Self> {
        let inner = &manager.inner;
        if filters.len() > 64 {
            return Err(Error::InvalidInput {
                field: "typed storage read",
                reason: "too many filters",
            });
        }
        let held = Held::default();
        let description = selection
            .memory_size()
            .saturating_add(filters.memory_size())
            .saturating_add(ENTRY_OVERHEAD);
        let description = Reservation::wait(inner, Pool::Read, &held, description, None)
            .await
            .map_err(Refusal::into_error)?;
        let active = query_slot(inner).await?;
        let snapshot = manager.snapshot(&at).await?;
        let mut pages = Self {
            inner: Arc::clone(inner),
            held,
            selection,
            key,
            filters,
            schema: None,
            snapshot: Some(snapshot),
            active: Some(active),
            _description: description,
            after: 0,
            rows: inner.settings.response.batch_rows,
            failed: false,
            inflight: false,
        };
        // Filters Read The Stored Schema; Never Written Means Nothing To Read.
        if !pages.filters.is_empty() {
            let snapshot = pages
                .snapshot
                .as_ref()
                .expect("opened reader owns its snapshot");
            match find_schema(&pages.inner, &pages.held, snapshot, &pages.key).await? {
                Some(schema) => pages.schema = Some(schema),
                None => pages.release(),
            }
        }
        Ok(pages)
    }
}

impl<E: Filtered> Pages<E> {
    async fn next<V>(
        &mut self,
        memory: impl Fn(&E) -> Result<usize>,
        build: impl Fn(E) -> Result<V>,
    ) -> Result<Option<Vec<V>>> {
        if self.failed || self.inflight {
            self.fail();
            return Err(Error::NotReady {
                domain: "failed or interrupted typed storage read",
            });
        }
        self.inflight = true;
        let result = self.advance(&memory, &build).await;
        self.inflight = false;
        if result.is_err() {
            self.fail();
        }
        result
    }

    async fn advance<V>(
        &mut self,
        memory: &impl Fn(&E) -> Result<usize>,
        build: &impl Fn(E) -> Result<V>,
    ) -> Result<Option<Vec<V>>> {
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

    // Scan, Filter And Decode One Page; State Advances Only After Every Claim Succeeded.
    async fn attempt<V>(
        &mut self,
        bank: &MemoryBank,
        memory: &impl Fn(&E) -> Result<usize>,
        build: &impl Fn(E) -> Result<V>,
    ) -> Result<Option<Vec<V>>> {
        let bytes = self
            .inner
            .settings
            .scan_bytes()
            .min(self.inner.settings.response.batch_bytes);
        let page = ScanPage {
            after: self.after,
            rows: self.rows,
            bytes,
        };
        let mut raw = bank.scope();
        raw.claim(
            self.rows
                .get()
                .saturating_mul(ENTRY_OVERHEAD)
                .saturating_add(bytes.get()),
        )?;
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("running reader owns its snapshot");
        let (entries, size) = snapshot.scan::<E>(&self.selection, page).await?;
        // A Single Oversized Entry Is Charged At Its Actual Size.
        raw.claim(size.saturating_sub(bytes.get()))?;
        let Some(last) = entries.last().map(|entry| entry.position().sequence) else {
            return Ok(None);
        };
        let entries = self.select(entries, bank)?;
        let mut decoded = bank.scope();
        for entry in &entries {
            decoded.claim(memory(entry)?)?;
        }
        let values = entries.into_iter().map(build).collect::<Result<Vec<_>>>()?;
        self.after = last;
        Ok(Some(values))
    }

    // Keep The Native Entries Whose Arrow Source Rows Every Filter Selects.
    fn select(&self, entries: Vec<E>, bank: &MemoryBank) -> Result<Vec<E>> {
        let Some((schema, _)) = &self.schema else {
            return Ok(entries);
        };
        let batch = E::arrow(schema, &entries, bank)?;
        let mut scratch = bank.scope();
        scratch.claim(resident_bytes(&batch)?)?;
        let mut keep = vec![true; entries.len()];
        for filter in &self.filters {
            filter.output_schema(batch.schema().as_ref())?;
            let mut selected = vec![false; entries.len()];
            for row in filter.select(&batch, &mut || Ok(()))? {
                selected[row as usize] = true;
            }
            for (kept, selected) in keep.iter_mut().zip(selected) {
                *kept &= selected;
            }
        }
        Ok(entries
            .into_iter()
            .zip(keep)
            .filter_map(|(entry, kept)| kept.then_some(entry))
            .collect())
    }

    fn fail(&mut self) {
        self.failed = true;
        self.schema = None;
        self.release();
    }

    fn release(&mut self) {
        self.snapshot = None;
        self.active = None;
    }
}

impl Filtered for ObjectEntry {
    fn arrow(schema: &StorageSchema, rows: &[Self], bank: &MemoryBank) -> Result<RecordBatch> {
        super::source::objects(schema, rows, bank)
    }
}

impl Filtered for StoredRecord {
    fn arrow(schema: &StorageSchema, rows: &[Self], bank: &MemoryBank) -> Result<RecordBatch> {
        super::source::records(schema, rows, bank)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn decoding<T: Message>(payload: &[u8]) -> Result<usize> {
    T::decoding_memory(payload)
        .map_err(|error| Error::backend("preflight payload", BackendFailure::Corrupt, error))
}

/* === Tests === */
