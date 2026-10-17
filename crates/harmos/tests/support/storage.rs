/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "storage_settings.rs"]
mod settings;
pub use settings::storage_settings;
#[path = "storage/journal.rs"]
mod journal;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

// Local Imports
use harmos::catalog::Projection;
use harmos::storage::query::{
    FileSelection, ObjectSelection, ObjectVersions, RecordSelection, SchemaSelection,
    StreamSelection,
};
use harmos::storage::state::*;
use harmos::storage::*;
use harmos::{BackendFailure, Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type AcceptedRead = (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
);

/* === Structs === */
#[derive(Default)]
pub struct Control {
    pub delete_failures: AtomicUsize,
    pub block_delete: AtomicBool,
    pub delete_entered: Notify,
    pub delete_release: Notify,
    pub checkpoint_failures: AtomicUsize,
    pub receipt_failures: AtomicUsize,
    pub fence_failures: AtomicUsize,
    pub block_fence: AtomicBool,
    pub fence_entered: Notify,
    pub fence_release: Notify,
    pub block_checkpoint: AtomicBool,
    pub checkpoint_entered: Notify,
    pub checkpoint_release: Notify,
    pub lost_objects_through: AtomicUsize,
    pub prune_calls: Mutex<Vec<(u64, usize)>>,
    pub prune_removed: Mutex<Vec<usize>>,
    pub block_prune: AtomicBool,
    pub prune_entered: Notify,
    pub prune_release: Notify,
    pub prune_failures: AtomicUsize,
    pub block_prune_result: AtomicBool,
    pub prune_result_entered: Notify,
    pub prune_result_release: Notify,
    pub block_create: AtomicBool,
    pub create_entered: Notify,
    pub create_release: Notify,
    pub fail_create: AtomicBool,
    pub create_failures: AtomicUsize,
    pub abandon_failures: AtomicUsize,
    pub block_chunk: AtomicBool,
    pub chunk_entered: Notify,
    pub chunk_release: Notify,
    pub chunk_failures: AtomicUsize,
    pub chunk_sizes: Mutex<Vec<usize>>,
    pub chunk_offsets: Mutex<Vec<u64>>,
    pub block_finish: AtomicBool,
    pub finish_entered: Notify,
    pub finish_release: Notify,
    pub fail_finish: AtomicBool,
    pub finish_failures: AtomicUsize,
    pub finish_positions: Mutex<Vec<u64>>,
    pub fail_abort: AtomicBool,
    pub block_abort: AtomicBool,
    pub abort_entered: Notify,
    pub abort_release: Notify,
    pub aborts: AtomicUsize,
    pub corrupt_file: AtomicBool,
    pub permanent: AtomicBool,
    pub failures: AtomicUsize,
    pub invalid_page: AtomicBool,
    pub block_view: AtomicBool,
    pub view_entered: Notify,
    pub view_release: Notify,
    pub fail_release: AtomicBool,
    pub block_release: AtomicBool,
    pub release_entered: Notify,
    pub release_release: Notify,
    pub releases: AtomicUsize,
    pub releases_completed: AtomicUsize,
    pub block_read: AtomicBool,
    pub read_entered: Notify,
    pub read_release: Notify,
    pub reads_gated: AtomicUsize,
    pub reads_gated_peak: AtomicUsize,
    pub accepted_file_read: Mutex<Option<AcceptedRead>>,
    pub block_write: AtomicBool,
    pub write_entered: Notify,
    pub write_release: Notify,
    pub block_flush: AtomicBool,
    pub flush_entered: Notify,
    pub flush_release: Notify,
    pub fail_close: AtomicBool,
    pub fail_open: AtomicBool,
    pub block_open: AtomicBool,
    pub open_entered: Notify,
    pub open_release: Notify,
    pub wrong_receipt: AtomicBool,
    pub flush_advance: AtomicUsize,
    pub flush_failures: AtomicUsize,
    pub regressed_saved: AtomicBool,
    pub open: AtomicBool,
    pub closes: AtomicUsize,
    pub writes: Mutex<Vec<u64>>,
    pub write_sizes: Mutex<Vec<usize>>,
    pub write_bytes: Mutex<Vec<usize>>,
    pub flushes: Mutex<Vec<u64>>,
}

#[derive(Clone)]
struct Snapshot {
    lost_objects_through: u64,
    objects: BTreeMap<u64, ObjectEntry>,
    files: BTreeMap<u64, FileReference>,
    schemas: BTreeMap<u64, StorageSchema>,
    records: BTreeMap<u64, StoredRecord>,
    rows: BTreeMap<u64, StoredRow>,
}

/// One persisted object lifecycle release frontier.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub schema: SchemaKey,
    pub item: ObjectKey,
    pub through: StorageCursor,
}

pub struct Data<C = ()> {
    pub object_releases: Mutex<Vec<Release>>,
    catalog: PhantomData<fn() -> C>,
    journal: Option<journal::Journal>,
    pub objects: Mutex<BTreeMap<u64, ObjectEntry>>,
    pub object_schemas: Mutex<BTreeMap<(String, u32), Vec<harmos::schema::Field>>>,
    pub staged: Mutex<BTreeMap<String, (FileMetadata, Vec<u8>)>>,
    pub abandoned_files: Mutex<std::collections::BTreeSet<String>>,
    pub resumable_files: Mutex<std::collections::BTreeSet<String>>,
    pub file_generations: Mutex<BTreeMap<String, u64>>,
    pub saved_productions: Mutex<BTreeMap<String, (FileCheckpoint, Vec<u8>)>>,
    pub mutation_receipts: Mutex<BTreeMap<MutationId, MutationReceipt>>,
    pub deleted_files: Mutex<std::collections::BTreeSet<String>>,
    pub lost_files_through: AtomicUsize,
    file_io: Mutex<()>,
    pub files: Mutex<BTreeMap<u64, FileReference>>,
    pub blobs: Mutex<BTreeMap<String, (FileReference, Vec<u8>)>>,
    snapshots: Mutex<BTreeMap<Vec<u8>, Snapshot>>,
    next_snapshot: AtomicUsize,
    pub history: StoreHistory,
    pub control: Control,
    pub records: Mutex<BTreeMap<u64, StoredRecord>>,
    pub rows: Mutex<BTreeMap<u64, StoredRow>>,
    pub schemas: Mutex<BTreeMap<u64, StorageSchema>>,
    pub saved: AtomicUsize,
}

pub struct Recovery {
    pub history: StoreHistory,
    pub control: Control,
    pub entries: Mutex<BTreeMap<u64, RecoveryEntry>>,
    pub saved: AtomicUsize,
    pub snapshot: Mutex<Option<RecoverySnapshot>>,
    pending_snapshot: Mutex<Option<RecoverySnapshot>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Control {
    // Hold A Gated Read Inside The Backend, Recording How Many Overlap.
    async fn gate_read(&self) {
        if !self.block_read.load(Ordering::SeqCst) {
            return;
        }
        let active = self.reads_gated.fetch_add(1, Ordering::SeqCst) + 1;
        self.reads_gated_peak.fetch_max(active, Ordering::SeqCst);
        self.read_entered.notify_one();
        self.read_release.notified().await;
        self.reads_gated.fetch_sub(1, Ordering::SeqCst);
    }

    async fn write(&self, positions: impl IntoIterator<Item = u64>) -> Result<()> {
        let positions: Vec<_> = positions.into_iter().collect();
        self.write_sizes.lock().unwrap().push(positions.len());
        self.writes.lock().unwrap().extend(positions);
        if self.block_write.load(Ordering::SeqCst) {
            self.write_entered.notify_one();
            self.write_release.notified().await;
        }
        if self.permanent.load(Ordering::SeqCst) {
            return Err(failure(BackendFailure::Permanent));
        }
        if self
            .failures
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(failure(BackendFailure::Transient));
        }
        Ok(())
    }

    async fn flush(&self, sequence: u64) -> Result<()> {
        self.flushes.lock().unwrap().push(sequence);
        if self.block_flush.load(Ordering::SeqCst) {
            self.flush_entered.notify_one();
            self.flush_release.notified().await;
        }
        Ok(())
    }

    fn close(&self) -> Result<()> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        if self.fail_close.load(Ordering::SeqCst) {
            return Err(failure(BackendFailure::Transient));
        }
        self.open.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl Data {
    pub fn new(history: StoreHistory) -> Self {
        Self::with_catalog(history)
    }
}

impl<C: ObjectCatalog> Data<C> {
    pub fn with_catalog(history: StoreHistory) -> Self {
        Self {
            object_releases: Mutex::new(Vec::new()),
            catalog: PhantomData,
            journal: None,
            objects: Mutex::default(),
            object_schemas: Mutex::default(),
            staged: Mutex::default(),
            abandoned_files: Mutex::default(),
            resumable_files: Mutex::default(),
            file_generations: Mutex::default(),
            saved_productions: Mutex::default(),
            mutation_receipts: Mutex::default(),
            deleted_files: Mutex::default(),
            lost_files_through: AtomicUsize::new(0),
            file_io: Mutex::default(),
            files: Mutex::default(),
            blobs: Mutex::default(),
            snapshots: Mutex::default(),
            next_snapshot: AtomicUsize::new(1),
            history,
            control: Control::default(),
            records: Mutex::new(BTreeMap::new()),
            rows: Mutex::new(BTreeMap::new()),
            schemas: Mutex::new(BTreeMap::new()),
            saved: AtomicUsize::new(0),
        }
    }
    #[allow(
        dead_code,
        reason = "Shared fixture: only journal-backed story binaries use this constructor"
    )]
    pub fn journaled(root: &std::path::Path, history: StoreHistory) -> Result<Self> {
        let mut data = Self::with_catalog(history);
        data.journal = Some(journal::Journal::new(root)?);
        data.journal.as_ref().unwrap().restore(&data)?;
        Ok(data)
    }

    fn at(&self, sequence: u64) -> StorageCursor {
        StorageCursor {
            history: self.history.clone(),
            sequence,
        }
    }
}

impl<C: ObjectCatalog> StorageBackend for Data<C> {
    fn release_object_history<'a>(
        &'a self,
        schema: &'a SchemaKey,
        item: &'a ObjectKey,
        through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            if through.sequence > self.saved.load(Ordering::SeqCst) as u64 {
                return Err(failure(BackendFailure::Conflict));
            }
            let release = Release {
                schema: schema.clone(),
                item: item.clone(),
                through: through.clone(),
            };
            if let Some(journal) = &self.journal {
                journal.release(&release)?;
            }
            upsert_release(&mut self.object_releases.lock().unwrap(), release);
            Ok(())
        })
    }
    fn history(&self) -> &StoreHistory {
        &self.history
    }
    fn open(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async {
            self.control.open.store(true, Ordering::SeqCst);
            if self.control.block_open.load(Ordering::SeqCst) {
                self.control.open_entered.notify_one();
                self.control.open_release.notified().await;
            }
            if self.control.fail_open.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            Ok(self.at(self.saved.load(Ordering::SeqCst) as u64))
        })
    }
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async {
            if self.control.regressed_saved.load(Ordering::SeqCst) {
                return Ok(self.at(0));
            }
            Ok(self.at(self.saved.load(Ordering::SeqCst) as u64))
        })
    }
    fn write<'a>(&'a self, batch: StorageBatch<'a>) -> StorageFuture<'a, StorageCursor> {
        Box::pin(async move {
            self.history.check(&batch.first().history)?;
            self.control
                .write_bytes
                .lock()
                .unwrap()
                .push(batch.encoded_bytes());
            self.control
                .write(batch.entries().map(|entry| entry.position().sequence))
                .await?;
            // Validate The Entire Atomic Batch Before Publishing Any Entry.
            for entry in batch.entries() {
                let position = entry.position().sequence;
                let conflict = match entry {
                    StorageEntry::Schema(value) => self
                        .schemas
                        .lock()
                        .unwrap()
                        .get(&position)
                        .is_some_and(|prior| prior != value),
                    StorageEntry::Object(value) => {
                        C::validate_value(&value.value, NonZeroUsize::MAX)?;
                        self.objects
                            .lock()
                            .unwrap()
                            .get(&position)
                            .is_some_and(|prior| prior != value)
                    }
                    StorageEntry::Record(value) => self
                        .records
                        .lock()
                        .unwrap()
                        .get(&position)
                        .is_some_and(|prior| prior != value),
                    StorageEntry::Row(value) => self
                        .rows
                        .lock()
                        .unwrap()
                        .get(&position)
                        .is_some_and(|prior| prior != value),
                    StorageEntry::File(value) => !self
                        .blobs
                        .lock()
                        .unwrap()
                        .get(value.metadata.id.as_str())
                        .is_some_and(|(prior, _)| prior == value),
                };
                if conflict {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            if let Some(journal) = &self.journal {
                journal.commit(batch)?;
            }
            for entry in batch.entries() {
                let position = entry.position().sequence;
                match entry {
                    StorageEntry::Schema(schema) => {
                        let mut stored = self.schemas.lock().unwrap();
                        if stored
                            .get(&position)
                            .is_some_and(|previous| previous != schema)
                        {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        stored.insert(position, schema.clone());
                    }
                    StorageEntry::Object(object) => {
                        C::validate_value(&object.value, NonZeroUsize::MAX)?;
                        let mut stored = self.objects.lock().unwrap();
                        if stored
                            .get(&position)
                            .is_some_and(|previous| previous != object)
                        {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        let definition = C::definitions()
                            .try_for_each(|definition| {
                                if definition.schema().unwrap() == object.value.schema {
                                    Err(*definition)
                                } else {
                                    Ok(())
                                }
                            })
                            .err()
                            .unwrap();
                        self.object_schemas.lock().unwrap().insert(
                            (
                                object.value.schema.id.to_string(),
                                object.value.schema.version.get(),
                            ),
                            (definition.declaration().fields)().to_vec(),
                        );
                        stored.insert(position, object.clone());
                    }
                    StorageEntry::Record(record) => {
                        let mut stored = self.records.lock().unwrap();
                        if stored
                            .get(&position)
                            .is_some_and(|previous| previous != record)
                        {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        stored.insert(position, record.clone());
                    }
                    StorageEntry::Row(row) => {
                        let mut stored = self.rows.lock().unwrap();
                        if stored
                            .get(&position)
                            .is_some_and(|previous| previous != row)
                        {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        stored.insert(position, row.clone());
                    }
                    StorageEntry::File(file) => {
                        if !self
                            .blobs
                            .lock()
                            .unwrap()
                            .get(file.metadata.id.as_str())
                            .is_some_and(|(finished, _)| finished == file)
                        {
                            return Err(failure(BackendFailure::Corrupt));
                        }
                        self.files.lock().unwrap().insert(position, file.clone());
                    }
                }
            }
            let through = batch.through();
            for position in (self.saved.load(Ordering::SeqCst) as u64 + 1)..=through.sequence {
                let count = usize::from(self.schemas.lock().unwrap().contains_key(&position))
                    + usize::from(self.objects.lock().unwrap().contains_key(&position))
                    + usize::from(self.records.lock().unwrap().contains_key(&position))
                    + usize::from(self.rows.lock().unwrap().contains_key(&position))
                    + usize::from(self.files.lock().unwrap().contains_key(&position));
                if count != 1 {
                    return Err(failure(BackendFailure::Corrupt));
                }
            }
            self.control.flush(through.sequence).await?;
            if self.control.wrong_receipt.load(Ordering::SeqCst) {
                return Ok(self.at(0));
            }
            let sequence = through
                .sequence
                .max(self.saved.load(Ordering::SeqCst) as u64)
                .max(self.control.flush_advance.load(Ordering::SeqCst) as u64);
            self.saved.store(sequence as usize, Ordering::SeqCst);
            if self
                .control
                .flush_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(self.at(sequence))
        })
    }
    fn prune_objects<'a>(
        &'a self,
        through: &'a StorageCursor,
        max_entries: NonZeroUsize,
    ) -> StorageFuture<'a, bool> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            assert!(through.sequence <= self.saved.load(Ordering::SeqCst) as u64);
            self.control
                .prune_calls
                .lock()
                .unwrap()
                .push((through.sequence, max_entries.get()));
            if self.control.block_prune.load(Ordering::SeqCst) {
                self.control.prune_entered.notify_one();
                self.control.prune_release.notified().await;
            }
            let done = {
                let mut objects = self.objects.lock().unwrap();
                let definitions = C::definitions();
                let mut remove = Vec::new();
                let mut done = true;
                // Controlled test storage uses a scan; real adapters use their key index.
                // Only a bounded removal list is allocated, independent of key count.
                for (&position, object) in objects.range(..=through.sequence) {
                    let retention = definitions
                        .try_for_each(|definition| {
                            if definition.schema().unwrap().id == object.value.schema.id {
                                Err(*definition)
                            } else {
                                Ok(())
                            }
                        })
                        .err()
                        .unwrap()
                        .retention();
                    if retention == ObjectRetention::UntilReleased
                        && !self.object_releases.lock().unwrap().iter().any(|release| {
                            release.schema == object.value.schema
                                && release.item == object.value.item
                                && release.through.sequence >= position
                        })
                    {
                        continue;
                    }
                    let keep = retention.versions().get();
                    let newer = objects
                        .range((
                            std::ops::Bound::Excluded(position),
                            std::ops::Bound::Included(through.sequence),
                        ))
                        .filter(|(_, other)| {
                            other.value.schema.id == object.value.schema.id
                                && other.value.item == object.value.item
                        })
                        .take(keep)
                        .count();
                    if newer == keep {
                        if remove.len() == max_entries.get() {
                            done = false;
                            break;
                        }
                        remove.push(position);
                    }
                }
                if let Some(journal) = &self.journal {
                    journal.prune(through, &remove)?;
                }
                for position in &remove {
                    objects.remove(position);
                }
                // Images Before This Cut May Have Needed A Reclaimed Revision.
                if !remove.is_empty() {
                    self.control
                        .lost_objects_through
                        .fetch_max(through.sequence as usize, Ordering::SeqCst);
                }
                self.control
                    .prune_removed
                    .lock()
                    .unwrap()
                    .push(remove.len());
                if self
                    .control
                    .prune_failures
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
                {
                    return Err(failure(BackendFailure::Transient));
                }
                done
            };
            if self.control.block_prune_result.load(Ordering::SeqCst) {
                self.control.prune_result_entered.notify_one();
                self.control.prune_result_release.notified().await;
            }
            Ok(done)
        })
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async { self.control.close() })
    }
    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            if self.control.block_view.load(Ordering::SeqCst) {
                self.control.view_entered.notify_one();
                self.control.view_release.notified().await;
            }
            self.history.check(&at.history)?;
            let lost_objects_through =
                self.control.lost_objects_through.load(Ordering::SeqCst) as u64;
            // Pruned Revisions No Longer Reconstruct An Older Image.
            if at.sequence < lost_objects_through {
                return Err(Error::Gap {
                    requested: at.sequence,
                    available: lost_objects_through,
                });
            }
            let snapshot = Snapshot {
                lost_objects_through,
                objects: self.objects.lock().unwrap().clone(),
                files: self.files.lock().unwrap().clone(),
                schemas: self.schemas.lock().unwrap().clone(),
                records: self.records.lock().unwrap().clone(),
                rows: self.rows.lock().unwrap().clone(),
            };
            let token = self
                .next_snapshot
                .fetch_add(1, Ordering::SeqCst)
                .to_le_bytes()
                .to_vec();
            self.snapshots
                .lock()
                .unwrap()
                .insert(token.clone(), snapshot);
            Ok(BackendSnapshot {
                at: at.clone(),
                token,
            })
        })
    }
    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.releases.fetch_add(1, Ordering::SeqCst);
            if self.control.block_release.load(Ordering::SeqCst) {
                self.control.release_entered.notify_one();
                self.control.release_release.notified().await;
            }
            if self.control.fail_release.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.snapshots.lock().unwrap().remove(&snapshot.token);
            self.control
                .releases_completed
                .fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
    fn read_schemas<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        selection: &'a SchemaSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            scan(
                image.schemas.values(),
                snapshot,
                page,
                |row| &row.position,
                |row| selection.schema.as_ref().is_none_or(|key| key == &row.key),
            )
        })
    }
    fn read_objects<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        selection: &'a ObjectSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            // Complete Revision History Cannot Begin Before Pruned Revisions.
            if selection.versions == ObjectVersions::Revisions
                && page.after < image.lost_objects_through
            {
                return Err(Error::Gap {
                    requested: page.after + 1,
                    available: image.lost_objects_through + 1,
                });
            }
            let selected = |object: &ObjectEntry| {
                selection
                    .schema
                    .as_ref()
                    .is_none_or(|schema| schema == &object.value.schema)
                    && selection
                        .key
                        .as_ref()
                        .is_none_or(|item| item == &object.value.item)
            };
            // Latest Keeps Each Selected Key's Newest Revision At The Snapshot.
            let latest = (selection.versions == ObjectVersions::Latest).then(|| {
                let mut latest = BTreeMap::new();
                for object in image.objects.values() {
                    if object.position.sequence <= snapshot.at.sequence && selected(object) {
                        latest.insert(
                            (object.value.schema.id.clone(), object.value.item.clone()),
                            object.position.sequence,
                        );
                    }
                }
                latest
            });
            let mut entries = scan(
                image.objects.values(),
                snapshot,
                page,
                |row| &row.position,
                |object| {
                    selected(object)
                        && latest.as_ref().is_none_or(|latest| {
                            latest.get(&(object.value.schema.id.clone(), object.value.item.clone()))
                                == Some(&object.position.sequence)
                        })
                },
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                entries[0].value.payload = vec![0xff];
            }
            Ok(entries)
        })
    }
    fn read_records<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        selection: &'a RecordSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let mut entries = scan(
                image.records.values(),
                snapshot,
                page,
                |row| &row.position,
                |row| {
                    record_matches(row, &selection.schema, &selection.scope, &selection.item)
                        && window(
                            selection.source_start,
                            selection.source_end,
                            row.source_time,
                        )
                },
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                entries[0].item = Identifier::new("wrong-item").unwrap();
            }
            Ok(entries)
        })
    }
    fn read_stream<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        selection: &'a StreamSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            scan(
                image.rows.values(),
                snapshot,
                page,
                |row| &row.record.position,
                |row| {
                    record_matches(
                        &row.record,
                        &selection.schema,
                        &selection.scope,
                        &selection.item,
                    ) && selection.stream.as_ref().is_none_or(|id| id == &row.stream)
                        && window(
                            selection.source_start,
                            selection.source_end,
                            row.record.source_time,
                        )
                },
            )
        })
    }
    fn read_files<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        selection: &'a FileSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            scan(
                image.files.values(),
                snapshot,
                page,
                |file| &file.position,
                |file| {
                    selection
                        .item
                        .as_ref()
                        .is_none_or(|id| id == &file.metadata.id)
                        && selection
                            .scope
                            .as_ref()
                            .is_none_or(|scope| Some(scope) == file.metadata.scope.as_ref())
                },
            )
        })
    }
    fn read_object<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        query: &'a ObjectPoint,
    ) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let object = image.objects.values().rev().find(|object| {
                object.position.sequence <= snapshot.at.sequence
                    && object.value.schema.id == query.schema
                    && object.value.item == query.item
                    && match &query.version {
                        ObjectVersion::Latest => true,
                        ObjectVersion::Exact(version) => version == &object.version,
                    }
            });
            let mut object = object.cloned();
            if self.control.invalid_page.load(Ordering::SeqCst)
                && let Some(object) = &mut object
            {
                object.value.payload = vec![0xff];
            }
            Ok(object)
        })
    }
    fn supports_file_sessions(&self) -> bool {
        true
    }
    fn supports_mutation_receipts(&self) -> bool {
        true
    }
    fn save_mutations<'a>(&'a self, receipts: &'a [MutationReceipt]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut saved = self.mutation_receipts.lock().unwrap();
            let now = std::time::SystemTime::now();
            saved.retain(|_, receipt| receipt.expires_at > now);
            for receipt in receipts {
                if receipt.expires_at <= now {
                    continue;
                }
                if saved
                    .get(&receipt.mutation)
                    .is_some_and(|prior| prior != receipt)
                {
                    return Err(failure(BackendFailure::Conflict));
                }
                saved.insert(receipt.mutation.clone(), receipt.clone());
            }
            if self
                .control
                .receipt_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }
    fn recover_mutations(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        Box::pin(async move {
            let mut saved = self.mutation_receipts.lock().unwrap();
            saved.retain(|_, receipt| receipt.expires_at > std::time::SystemTime::now());
            let mut newest: Vec<_> = saved.values().collect();
            newest.sort_by_key(|receipt| std::cmp::Reverse(receipt.expires_at));
            let mut bytes = std::mem::size_of::<Vec<MutationReceipt>>();
            let mut receipts = Vec::new();
            for receipt in newest.into_iter().take(limits.rows.get()) {
                bytes = bytes.saturating_add(receipt.memory_size());
                if bytes > limits.bytes.get() {
                    break;
                }
                receipts.push(receipt.clone());
            }
            Ok(receipts)
        })
    }
    fn create_production<'a>(
        &'a self,
        metadata: &'a FileMetadata,
    ) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            if self
                .staged
                .lock()
                .unwrap()
                .contains_key(metadata.id.as_str())
                && !self
                    .resumable_files
                    .lock()
                    .unwrap()
                    .contains(metadata.id.as_str())
            {
                return Err(failure(BackendFailure::Conflict));
            }
            if self
                .blobs
                .lock()
                .unwrap()
                .values()
                .any(|(file, _)| file.metadata.id == metadata.id)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.resumable_files
                .lock()
                .unwrap()
                .insert(metadata.id.to_string());
            self.file_generations
                .lock()
                .unwrap()
                .entry(metadata.id.to_string())
                .or_insert(1);
            self.create_file(metadata).await
        })
    }
    fn fence_production<'a>(
        &'a self,
        file: &'a StagedFile,
        expected: u64,
        next: u64,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.control.block_fence.load(Ordering::SeqCst) {
                self.control.fence_entered.notify_one();
                self.control.fence_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            self.history.check(&file.history)?;
            if expected.checked_add(1) != Some(next) {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut generations = self.file_generations.lock().unwrap();
            let generation = generations
                .get_mut(file.id.as_str())
                .ok_or_else(|| failure(BackendFailure::Permanent))?;
            if *generation != expected && *generation != next {
                return Err(failure(BackendFailure::Conflict));
            }
            *generation = next;
            if self
                .control
                .fence_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }
    fn save_production<'a>(&'a self, checkpoint: &'a FileCheckpoint) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self.control.block_checkpoint.load(Ordering::SeqCst) {
                self.control.checkpoint_entered.notify_one();
                self.control.checkpoint_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            self.history.check(&checkpoint.session.history)?;
            let id = checkpoint.session.id.as_str();
            if self.deleted_files.lock().unwrap().contains(id)
                || self.abandoned_files.lock().unwrap().contains(id)
                || self
                    .blobs
                    .lock()
                    .unwrap()
                    .values()
                    .any(|(file, _)| file.metadata.id == checkpoint.session.id)
            {
                return Ok(());
            }
            if self
                .file_generations
                .lock()
                .unwrap()
                .get(id)
                .is_none_or(|generation| *generation < checkpoint.session.generation)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            let staged = self.staged.lock().unwrap();
            let (metadata, bytes) = staged
                .get(id)
                .ok_or_else(|| failure(BackendFailure::Permanent))?;
            let length =
                usize::try_from(checkpoint.length).map_err(|_| failure(BackendFailure::Corrupt))?;
            let prefix = bytes
                .get(..length)
                .ok_or_else(|| failure(BackendFailure::Corrupt))?;
            if *metadata != checkpoint.metadata
                || <[u8; 32]>::from(Sha256::digest(prefix)) != checkpoint.sha256()?
            {
                return Err(failure(BackendFailure::Corrupt));
            }
            let mut saved = self.saved_productions.lock().unwrap();
            if let Some((prior, prior_bytes)) = saved.get(id) {
                if prior.length > checkpoint.length {
                    return Ok(());
                }
                if prior.length == checkpoint.length && prior_bytes != prefix {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            saved.insert(id.to_string(), (checkpoint.clone(), prefix.to_vec()));
            if self
                .control
                .checkpoint_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }
    fn recover_productions(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        Box::pin(async move {
            let _io = self.file_io.lock().unwrap();
            let saved = self.saved_productions.lock().unwrap();
            let required = std::mem::size_of::<Vec<FileCheckpoint>>().saturating_add(
                saved
                    .values()
                    .map(|(checkpoint, _)| checkpoint.memory_size())
                    .sum::<usize>(),
            );
            if saved.len() > limits.rows.get() || required > limits.bytes.get() {
                return Err(failure(BackendFailure::Permanent));
            }
            let mut restored = Vec::with_capacity(saved.len());
            for (id, (checkpoint, bytes)) in saved.iter() {
                if <[u8; 32]>::from(Sha256::digest(bytes)) != checkpoint.sha256()?
                    || bytes.len() as u64 != checkpoint.length
                {
                    return Err(failure(BackendFailure::Corrupt));
                }
                let mut checkpoint = checkpoint.clone();
                checkpoint.session.generation = *self
                    .file_generations
                    .lock()
                    .unwrap()
                    .get(id)
                    .ok_or_else(|| failure(BackendFailure::Corrupt))?;
                self.staged
                    .lock()
                    .unwrap()
                    .insert(id.clone(), (checkpoint.metadata.clone(), bytes.clone()));
                restored.push(checkpoint);
            }
            // Unsaved Productions Have No Durable Resume Boundary.
            for id in self.resumable_files.lock().unwrap().iter() {
                if !saved.contains_key(id) && self.staged.lock().unwrap().contains_key(id) {
                    self.abandoned_files.lock().unwrap().insert(id.clone());
                }
            }
            Ok(restored)
        })
    }
    fn file_progress<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        Box::pin(async move {
            let _io = self.file_io.lock().unwrap();
            self.history.check(&file.history)?;
            if let Some((reference, _)) = self
                .blobs
                .lock()
                .unwrap()
                .values()
                .find(|(reference, _)| reference.metadata.id == file.id)
            {
                return Ok(FileProgress {
                    length: reference.length,
                    phase: FilePhase::Finished,
                });
            }
            if !self
                .resumable_files
                .lock()
                .unwrap()
                .contains(file.id.as_str())
            {
                return Err(failure(BackendFailure::Permanent));
            }
            let staged = self.staged.lock().unwrap();
            let (_, bytes) = staged
                .get(file.id.as_str())
                .ok_or_else(|| failure(BackendFailure::Permanent))?;
            let phase = if self
                .abandoned_files
                .lock()
                .unwrap()
                .contains(file.id.as_str())
            {
                FilePhase::Abandoned
            } else {
                FilePhase::Producing
            };
            Ok(FileProgress {
                length: bytes.len() as u64,
                phase,
            })
        })
    }
    fn read_production<'a>(
        &'a self,
        file: &'a StagedFile,
        offset: u64,
        max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.history.check(&file.history)?;
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            let copy = |bytes: &[u8]| -> Result<Vec<u8>> {
                let offset =
                    usize::try_from(offset).map_err(|_| failure(BackendFailure::Permanent))?;
                let end = bytes.len().min(offset.saturating_add(max_bytes.get()));
                let chunk = bytes
                    .get(offset..end)
                    .ok_or_else(|| failure(BackendFailure::Permanent))?;
                Ok(Box::<[u8]>::from(chunk).into_vec())
            };
            if let Some((_, bytes)) = self
                .blobs
                .lock()
                .unwrap()
                .values()
                .find(|(reference, _)| reference.metadata.id == file.id)
            {
                return copy(bytes);
            }
            if !self
                .resumable_files
                .lock()
                .unwrap()
                .contains(file.id.as_str())
            {
                return Err(failure(BackendFailure::Permanent));
            }
            let staged = self.staged.lock().unwrap();
            let (_, bytes) = staged
                .get(file.id.as_str())
                .ok_or_else(|| failure(BackendFailure::Permanent))?;
            copy(bytes)
        })
    }

    fn supports_file_deletion(&self) -> bool {
        true
    }
    fn delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&file.history)?;
            if self.control.block_delete.load(Ordering::SeqCst) {
                self.control.delete_entered.notify_one();
                self.control.delete_release.notified().await;
            }
            let _io = self.file_io.lock().unwrap();
            let id = file.id.as_str();
            let mut staged = self.staged.lock().unwrap();
            if staged.contains_key(id) && !self.abandoned_files.lock().unwrap().contains(id) {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut files = self.files.lock().unwrap();
            let mut deleted = self.deleted_files.lock().unwrap();
            if deleted.insert(id.to_owned()) {
                let last = files
                    .values()
                    .filter(|value| value.metadata.id == file.id)
                    .map(|value| value.position.sequence)
                    .max()
                    .unwrap_or(0);
                self.lost_files_through
                    .fetch_max(last as usize, Ordering::SeqCst);
            }
            files.retain(|_, value| value.metadata.id != file.id);
            staged.remove(id);
            self.blobs.lock().unwrap().remove(id);
            self.saved_productions.lock().unwrap().remove(id);
            if self
                .control
                .delete_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }

    fn abandon_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let _io = self.file_io.lock().unwrap();
            self.history.check(&staging.history)?;
            if self
                .blobs
                .lock()
                .unwrap()
                .values()
                .any(|(file, _)| file.metadata.id == staging.id)
            {
                return Ok(());
            }
            if self.control.fail_abort.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.saved_productions
                .lock()
                .unwrap()
                .remove(staging.id.as_str());
            self.abandoned_files
                .lock()
                .unwrap()
                .insert(staging.id.to_string());
            if self
                .control
                .abandon_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            if self
                .abandoned_files
                .lock()
                .unwrap()
                .contains(metadata.id.as_str())
            {
                return Err(failure(BackendFailure::Conflict));
            }
            {
                let _io = self.file_io.lock().unwrap();
                if self
                    .deleted_files
                    .lock()
                    .unwrap()
                    .contains(metadata.id.as_str())
                {
                    return Err(failure(BackendFailure::Conflict));
                }
                let mut files = self.staged.lock().unwrap();
                let existing = files
                    .entry(metadata.id.to_string())
                    .or_insert_with(|| (metadata.clone(), Vec::new()));
                if existing.0 != *metadata {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            if self.control.block_create.load(Ordering::SeqCst) {
                self.control.create_entered.notify_one();
                self.control.create_release.notified().await;
            }
            if self.control.fail_create.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            if self
                .control
                .create_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            if let Some(journal) = &self.journal {
                journal.stage(&metadata.id, &[])?;
            }
            Ok(StagedFile {
                history: self.history.clone(),
                id: metadata.id.clone(),
            })
        })
    }
    fn write_file<'a>(
        &'a self,
        staging: &'a StagedFile,
        offset: u64,
        bytes: &'a [u8],
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self
                .abandoned_files
                .lock()
                .unwrap()
                .contains(staging.id.as_str())
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control.chunk_sizes.lock().unwrap().push(bytes.len());
            self.control.chunk_offsets.lock().unwrap().push(offset);
            if self.control.block_chunk.load(Ordering::SeqCst) {
                self.control.chunk_entered.notify_one();
                self.control.chunk_release.notified().await;
            }
            {
                let _io = self.file_io.lock().unwrap();
                let mut files = self.staged.lock().unwrap();
                let (_, stored) = files
                    .get_mut(staging.id.as_str())
                    .ok_or_else(|| failure(BackendFailure::Permanent))?;
                let offset = usize::try_from(offset).unwrap();
                if offset == stored.len() {
                    stored.extend_from_slice(bytes);
                } else if stored.get(offset..offset + bytes.len()) != Some(bytes) {
                    return Err(failure(BackendFailure::Conflict));
                }
                if let Some(journal) = &self.journal {
                    journal.stage(&staging.id, stored)?;
                }
            }
            if self
                .control
                .chunk_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(())
        })
    }
    fn finish_file<'a>(
        &'a self,
        staging: &'a StagedFile,
        position: &'a StorageCursor,
        length: u64,
        sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            if self
                .abandoned_files
                .lock()
                .unwrap()
                .contains(staging.id.as_str())
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control
                .finish_positions
                .lock()
                .unwrap()
                .push(position.sequence);
            if self.control.block_finish.load(Ordering::SeqCst) {
                self.control.finish_entered.notify_one();
                self.control.finish_release.notified().await;
            }
            if self.control.fail_finish.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            let _io = self.file_io.lock().unwrap();
            if let Some((file, _)) = self.blobs.lock().unwrap().get(staging.id.as_str()) {
                if file.position != *position
                    || file.length != length
                    || file.sha256 != sha256
                    || file.metadata.id != staging.id
                {
                    return Err(failure(BackendFailure::Conflict));
                }
                return Ok(file.clone());
            }
            let mut staged = self.staged.lock().unwrap();
            let (_, bytes) = staged
                .get(staging.id.as_str())
                .ok_or_else(|| failure(BackendFailure::Permanent))?;
            if bytes.len() as u64 != length || <[u8; 32]>::from(Sha256::digest(bytes)) != sha256 {
                return Err(failure(BackendFailure::Corrupt));
            }
            if let Some(journal) = &self.journal {
                journal.finish(&staging.id, bytes, &sha256)?;
            }
            // A Failed Integrity Check Must Retain The Last Recoverable Prefix.
            let (metadata, bytes) = staged
                .remove(staging.id.as_str())
                .expect("verified staging");
            self.saved_productions
                .lock()
                .unwrap()
                .remove(staging.id.as_str());
            let reference = FileReference {
                position: position.clone(),
                metadata,
                length,
                sha256,
            };
            self.blobs
                .lock()
                .unwrap()
                .insert(staging.id.to_string(), (reference.clone(), bytes));
            if self
                .control
                .finish_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(reference)
        })
    }
    fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.aborts.fetch_add(1, Ordering::SeqCst);
            if self.control.block_abort.load(Ordering::SeqCst) {
                self.control.abort_entered.notify_one();
                self.control.abort_release.notified().await;
            }
            if self.control.fail_abort.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.staged.lock().unwrap().remove(staging.id.as_str());
            Ok(())
        })
    }
    fn read_file<'a>(
        &'a self,
        snapshot: &'a BackendSnapshot,
        file: &'a FileReference,
        offset: u64,
        max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            self.image(snapshot)?;
            let mut result = {
                let blobs = self.blobs.lock().unwrap();
                let (actual, bytes) = blobs
                    .get(file.metadata.id.as_str())
                    .ok_or_else(|| failure(BackendFailure::Permanent))?;
                if actual != file {
                    return Err(failure(BackendFailure::Conflict));
                }
                let offset = usize::try_from(offset).unwrap();
                let end = bytes.len().min(offset.saturating_add(max_bytes.get()));
                bytes[offset..end].to_vec()
            };
            if self.control.corrupt_file.load(Ordering::SeqCst) && !result.is_empty() {
                result[0] ^= 1;
            }
            let accepted = self.control.accepted_file_read.lock().unwrap().take();
            if let Some((started, release)) = accepted {
                started.send(()).unwrap();
                release.await.unwrap();
            }
            Ok(result)
        })
    }
}

impl<C: ObjectCatalog> Data<C> {
    fn image(&self, snapshot: &BackendSnapshot) -> Result<Snapshot> {
        self.snapshots
            .lock()
            .unwrap()
            .get(&snapshot.token)
            .cloned()
            .ok_or(Error::Closing {
                domain: "test snapshot",
            })
    }
}

impl Recovery {
    pub fn new(history: StoreHistory) -> Self {
        Self {
            history,
            control: Control::default(),
            entries: Mutex::new(BTreeMap::new()),
            saved: AtomicUsize::new(0),
            snapshot: Mutex::new(None),
            pending_snapshot: Mutex::new(None),
        }
    }
    fn at(&self, sequence: u64) -> RecoveryPosition {
        RecoveryPosition {
            history: self.history.clone(),
            sequence,
        }
    }
}

impl StateBackend for Recovery {
    fn history(&self) -> &StoreHistory {
        &self.history
    }
    fn open(&self) -> StorageFuture<'_, RecoveryPosition> {
        Box::pin(async {
            self.control.open.store(true, Ordering::SeqCst);
            if self.control.block_open.load(Ordering::SeqCst) {
                self.control.open_entered.notify_one();
                self.control.open_release.notified().await;
            }
            if self.control.fail_open.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            Ok(self.at(self.saved.load(Ordering::SeqCst) as u64))
        })
    }
    fn write_entries<'a>(&'a self, entries: &'a [RecoveryEntry]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control
                .write(entries.iter().map(|e| e.position.sequence))
                .await?;
            let mut stored = self.entries.lock().unwrap();
            for entry in entries {
                stored.insert(entry.position.sequence, entry.clone());
            }
            Ok(())
        })
    }
    fn write_snapshot<'a>(&'a self, snapshot: &'a RecoverySnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.write([snapshot.through.sequence]).await?;
            *self.pending_snapshot.lock().unwrap() = Some(snapshot.clone());
            Ok(())
        })
    }
    fn read_snapshot(&self, _: NonZeroUsize) -> StorageFuture<'_, Option<RecoverySnapshot>> {
        Box::pin(async {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            Ok(self.snapshot.lock().unwrap().clone())
        })
    }
    fn read_entries<'a>(
        &'a self,
        after: &'a RecoveryPosition,
        through: &'a RecoveryPosition,
        limits: ReadLimits,
    ) -> StorageFuture<'a, RecoveryPage> {
        Box::pin(async move {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let stored = self.entries.lock().unwrap();
            let mut bytes = 0;
            let mut entries = Vec::new();
            for (_, entry) in stored.range((after.sequence + 1)..=through.sequence) {
                let size = serde_json::to_vec(entry).unwrap().len();
                if entries.len() == limits.rows.get() || bytes + size > limits.bytes.get() {
                    break;
                }
                bytes += size;
                entries.push(entry.clone());
            }
            let last = entries
                .last()
                .map_or(after.sequence, |e| e.position.sequence);
            Ok(RecoveryPage {
                entries,
                next: (last < through.sequence).then(|| self.at(last)),
            })
        })
    }
    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition> {
        Box::pin(async move {
            self.control.flush(through.sequence).await?;
            self.saved
                .store(through.sequence as usize, Ordering::SeqCst);
            let mut pending = self.pending_snapshot.lock().unwrap();
            if pending
                .as_ref()
                .is_some_and(|snapshot| snapshot.through.sequence <= through.sequence)
            {
                *self.snapshot.lock().unwrap() = pending.take();
            }
            if self
                .control
                .flush_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(through.clone())
        })
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async { self.control.close() })
    }
}

/* === Public Functions === */
pub fn history() -> StoreHistory {
    StoreHistory {
        store: Identifier::new("test-store").unwrap(),
        epoch: Identifier::new("test-epoch").unwrap(),
    }
}

/* === Private Functions === */
fn failure(kind: BackendFailure) -> Error {
    Error::backend(
        "test backend",
        kind,
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected failure"),
    )
}

fn record_matches(
    row: &StoredRecord,
    schema: &SchemaKey,
    scope: &Option<Identifier>,
    item: &Option<Identifier>,
) -> bool {
    &row.schema == schema
        && item.as_ref().is_none_or(|id| id == &row.item)
        && scope
            .as_ref()
            .is_none_or(|scope| Some(scope) == row.scope.as_ref())
}

fn window(
    start: Option<std::time::SystemTime>,
    end: Option<std::time::SystemTime>,
    time: Option<std::time::SystemTime>,
) -> bool {
    match (start, end) {
        (Some(start), Some(end)) => time.is_some_and(|time| time >= start && time < end),
        _ => true,
    }
}

// Ascending Entries In (page.after, snapshot.at]; At Least One While Any Remain.
fn scan<'a, T: Clone + serde::Serialize + 'a>(
    entries: impl Iterator<Item = &'a T>,
    snapshot: &BackendSnapshot,
    page: ScanPage,
    position: impl Fn(&T) -> &StorageCursor,
    matches: impl Fn(&T) -> bool,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    let mut bytes = 0usize;
    for entry in entries {
        let at = position(entry).sequence;
        if at <= page.after || at > snapshot.at.sequence || !matches(entry) {
            continue;
        }
        let size = serde_json::to_vec(entry).unwrap().len();
        if output.len() == page.rows.get()
            || (!output.is_empty() && bytes + size > page.bytes.get())
        {
            break;
        }
        bytes += size;
        output.push(entry.clone());
    }
    Ok(output)
}

fn upsert_release(releases: &mut Vec<Release>, release: Release) {
    if let Some(prior) = releases
        .iter_mut()
        .find(|prior| prior.schema == release.schema && prior.item == release.item)
    {
        if prior.through.sequence < release.through.sequence {
            *prior = release;
        }
    } else {
        releases.push(release);
    }
}

/* === Tests === */
