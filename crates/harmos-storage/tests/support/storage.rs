/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "file_backend.rs"]
mod file_backend;
#[path = "read_backend.rs"]
mod read_backend;
#[path = "settings.rs"]
mod settings;
pub use settings::storage_settings;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use harmos_domain::catalog::Projection;
use harmos_storage::query::{FileSelection, ObjectSelection, ObjectVersions, RecordSelection, SchemaSelection, StreamSelection};
use harmos_storage::state::*;
use harmos_storage::*;
use harmos_storage::{BackendFailure, Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type AcceptedRead = (tokio::sync::oneshot::Sender<()>, tokio::sync::oneshot::Receiver<()>);

/* === Structs === */

#[derive(Default)]
pub struct Control {
    pub block_chunk_file: Mutex<Option<String>>,
    pub permanent_chunk_file: Mutex<Option<String>>,
    pub chunk_active: Mutex<BTreeMap<String, usize>>,
    pub chunk_peak_by_file: Mutex<BTreeMap<String, usize>>,
    pub chunk_peak: AtomicUsize,
    pub chunk_changed: Notify,
    pub chunk_pointers: Mutex<Vec<(String, u64, usize, usize)>>,
    pub retain_recovery: AtomicBool,
    pub retained_recovery: Mutex<Option<StorageBatch<RecoveryEntry>>>,
    pub retain_snapshot: AtomicBool,
    pub retained_snapshot: Mutex<Option<StorageBatch<RecoverySnapshot>>>,
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
    pub fenced: Notify,
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
    pub pruned: Notify,
    pub block_prune_result: AtomicBool,
    pub prune_result_entered: Notify,
    pub prune_result_release: Notify,
    pub block_create: AtomicBool,
    pub create_entered: Notify,
    pub create_release: Notify,
    pub fail_create: AtomicBool,
    pub create_failures: AtomicUsize,
    pub abandon_failures: AtomicUsize,
    pub abandoned: Notify,
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
    pub write_failure_at: AtomicUsize,
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
    pub release_completed: Notify,
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
    pub snapshot_written: Notify,
    pub wrong_receipt: AtomicBool,
    pub flush_advance: AtomicUsize,
    pub flush_failures: AtomicUsize,
    pub regressed_saved: AtomicBool,
    pub open: AtomicBool,
    pub closes: AtomicUsize,
    pub writes: Mutex<Vec<u64>>,
    pub write_sizes: Mutex<Vec<usize>>,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub schema: SchemaKey,
    pub item: ObjectKey,
    pub through: StorageCursor,
}

pub struct Data<C = ()> {
    pub object_releases: Mutex<Vec<Release>>,
    catalog: PhantomData<fn() -> C>,
    pub objects: Mutex<BTreeMap<u64, ObjectEntry>>,
    pub object_schemas: Mutex<BTreeMap<(String, u32), Vec<harmos_domain::schema::Field>>>,
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
        self.writes.lock().unwrap().extend(positions.iter().copied());
        if self.block_write.load(Ordering::SeqCst) {
            self.write_entered.notify_one();
            self.write_release.notified().await;
        }
        if self.permanent.load(Ordering::SeqCst) {
            return Err(failure(BackendFailure::Permanent));
        }
        let failure_at = self.write_failure_at.load(Ordering::SeqCst);
        if failure_at != 0 && positions.contains(&(failure_at as u64)) {
            self.write_failure_at.store(0, Ordering::SeqCst);
            return Err(failure(BackendFailure::Transient));
        }
        if self.failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
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
    fn at(&self, sequence: u64) -> StorageCursor {
        StorageCursor { history: self.history.clone(), sequence }
    }
}

impl<C: ObjectCatalog> StorageBackend for Data<C> {
    fn release_object_history<'a>(
        &'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor,
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
            upsert_release(&mut self.object_releases.lock().unwrap(), release);
            Ok(())
        })
    }
    fn history(&self) -> &StoreHistory {
        &self.history
    }
    fn open(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
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
        Box::pin(async move {
            if self.control.regressed_saved.load(Ordering::SeqCst) {
                return Ok(self.at(0));
            }
            Ok(self.at(self.saved.load(Ordering::SeqCst) as u64))
        })
    }
    fn write_schemas(&self, batch: StorageBatch<StorageSchema>) -> StorageFuture<'_, StorageCursor> {
        self.persist(batch, &self.schemas, |value| &value.position, |_| Ok(()))
    }
    fn write_objects(&self, batch: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor> {
        self.persist(
            batch,
            &self.objects,
            |value| &value.position,
            |object| {
                C::validate_value(&object.value, NonZeroUsize::MAX)?;
                let definition = C::definitions()
                    .try_for_each(|definition| if definition.schema().unwrap() == object.value.schema { Err(*definition) } else { Ok(()) })
                    .err()
                    .unwrap();
                let fields = (definition.declaration().fields)().to_vec();
                self.object_schemas.lock().unwrap().insert((object.value.schema.id.to_string(), object.value.schema.version.get()), fields);
                Ok(())
            },
        )
    }
    fn write_records(&self, batch: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor> {
        self.persist(batch, &self.records, |value| &value.position, |_| Ok(()))
    }
    fn write_stream(&self, batch: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor> {
        self.persist(batch, &self.rows, |value| &value.record.position, |_| Ok(()))
    }
    fn write_files(&self, batch: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor> {
        self.persist(
            batch,
            &self.files,
            |value| &value.position,
            |file| {
                if !self.blobs.lock().unwrap().get(file.metadata.id.as_str()).is_some_and(|(finished, _)| finished == file) {
                    return Err(failure(BackendFailure::Corrupt));
                }
                Ok(())
            },
        )
    }
    fn prune_objects<'a>(
        &'a self, through: &'a StorageCursor, max_entries: NonZeroUsize,
        retention: fn(&SchemaKey) -> harmos_domain::Result<ObjectRetention>,
    ) -> StorageFuture<'a, bool> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            assert!(through.sequence <= self.saved.load(Ordering::SeqCst) as u64);
            self.control.prune_calls.lock().unwrap().push((through.sequence, max_entries.get()));
            if self.control.block_prune.load(Ordering::SeqCst) {
                self.control.prune_entered.notify_one();
                self.control.prune_release.notified().await;
            }
            let done = {
                let mut objects = self.objects.lock().unwrap();
                let mut remove = Vec::new();
                let mut done = true;
                // Controlled test storage uses a scan; real adapters use their key index.
                // Only a bounded removal list is allocated, independent of key count.
                for (&position, object) in objects.range(..=through.sequence) {
                    let retention = retention(&object.value.schema)?;
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
                        .range((std::ops::Bound::Excluded(position), std::ops::Bound::Included(through.sequence)))
                        .filter(|(_, other)| other.value.schema.id == object.value.schema.id && other.value.item == object.value.item)
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
                for position in &remove {
                    objects.remove(position);
                }
                // Images Before This Cut May Have Needed A Reclaimed Revision.
                if !remove.is_empty() {
                    self.control.lost_objects_through.fetch_max(through.sequence as usize, Ordering::SeqCst);
                }
                self.control.prune_removed.lock().unwrap().push(remove.len());
                if self.control.prune_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                    return Err(failure(BackendFailure::Transient));
                }
                done
            };
            self.control.pruned.notify_one();
            if self.control.block_prune_result.load(Ordering::SeqCst) {
                self.control.prune_result_entered.notify_one();
                self.control.prune_result_release.notified().await;
            }
            Ok(done)
        })
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.control.close() })
    }
    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        self.fixture_open_snapshot(at)
    }
    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fixture_release_snapshot(snapshot).await })
    }
    fn read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        self.fixture_read_schemas(snapshot, selection, page)
    }
    fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        self.fixture_read_objects(snapshot, selection, page)
    }
    fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        self.fixture_read_records(snapshot, selection, page)
    }
    fn read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        self.fixture_read_stream(snapshot, selection, page)
    }
    fn read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        self.fixture_read_files(snapshot, selection, page)
    }
    fn read_object<'a>(&'a self, snapshot: &'a BackendSnapshot, query: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>> {
        self.fixture_read_object(snapshot, query)
    }
    fn supports_file_sessions(&self) -> bool {
        self.fixture_supports_file_sessions()
    }
    fn supports_mutation_receipts(&self) -> bool {
        self.fixture_supports_mutation_receipts()
    }
    fn save_mutations(&self, receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.fixture_save_mutations(receipts.as_slice()).await })
    }
    fn recover_mutations(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        self.fixture_recover_mutations(limits)
    }
    fn create_production<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move { self.fixture_create_production(metadata).await })
    }
    fn fence_production<'a>(&'a self, file: &'a StagedFile, expected: u64, next: u64) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fixture_fence_production(file, expected, next).await })
    }
    fn save_production(&self, checkpoint: StorageBatch<FileCheckpoint>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            for checkpoint in checkpoint.as_slice() {
                self.fixture_save_production(checkpoint).await?;
            }
            Ok(())
        })
    }
    fn recover_productions(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        self.fixture_recover_productions(limits)
    }
    fn file_progress<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        self.fixture_file_progress(file)
    }
    fn read_production<'a>(&'a self, file: &'a StagedFile, offset: u64, max_bytes: NonZeroUsize) -> StorageFuture<'a, Vec<u8>> {
        self.fixture_read_production(file, offset, max_bytes)
    }

    fn supports_file_deletion(&self) -> bool {
        self.fixture_supports_file_deletion()
    }
    fn delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fixture_delete_file(file).await })
    }

    fn abandon_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fixture_abandon_file(staging).await })
    }
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move { self.fixture_create_file(metadata).await })
    }
    fn write_file<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()> {
        self.fixture_write_file(staging, offset, bytes)
    }
    fn finish_file<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move { self.fixture_finish_file(staging, position, length, sha256).await })
    }
    fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fixture_abort_file(staging).await })
    }
    fn read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        self.fixture_read_file(snapshot, file, offset, max_bytes)
    }
}

impl<C: ObjectCatalog> Data<C> {
    fn persist<'a, T: Clone + PartialEq + Send + Sync + 'static>(
        &'a self, batch: StorageBatch<T>, stored: &'a Mutex<BTreeMap<u64, T>>, position: fn(&T) -> &StorageCursor,
        validate: impl Fn(&T) -> Result<()> + Send + 'a,
    ) -> StorageFuture<'a, StorageCursor> {
        Box::pin(async move {
            let values = batch.as_slice();
            let through = position(values.last().ok_or(Error::InvalidInput {
                field: "controlled write",
                reason: "typed batch is empty",
            })?)
            .clone();
            for value in values {
                self.history.check(&position(value).history)?;
            }
            self.control.write(values.iter().map(|value| position(value).sequence)).await?;

            // Each Typed Call Validates Its Own Replay Identity Before Inserting.
            for value in values {
                validate(value)?;
                if stored.lock().unwrap().get(&position(value).sequence).is_some_and(|prior| prior != value) {
                    return Err(failure(BackendFailure::Conflict));
                }
            }
            for value in values {
                stored.lock().unwrap().insert(position(value).sequence, value.clone());
            }

            // A Later Type Can Fail Without Rolling Back An Earlier Durable Call.
            for sequence in (self.saved.load(Ordering::SeqCst) as u64 + 1)..=through.sequence {
                let count = usize::from(self.schemas.lock().unwrap().contains_key(&sequence))
                    + usize::from(self.objects.lock().unwrap().contains_key(&sequence))
                    + usize::from(self.records.lock().unwrap().contains_key(&sequence))
                    + usize::from(self.rows.lock().unwrap().contains_key(&sequence))
                    + usize::from(self.files.lock().unwrap().contains_key(&sequence));
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
            if self.control.flush_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(self.at(sequence))
        })
    }

    fn image(&self, snapshot: &BackendSnapshot) -> Result<Snapshot> {
        self.snapshots.lock().unwrap().get(&snapshot.token).cloned().ok_or(Error::Closing { domain: "test snapshot" })
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
        RecoveryPosition { history: self.history.clone(), sequence }
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
    fn write_entries(&self, entries: StorageBatch<RecoveryEntry>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if self.control.retain_recovery.load(Ordering::SeqCst) {
                *self.control.retained_recovery.lock().unwrap() = Some(entries.clone());
            }
            self.control.write(entries.as_slice().iter().map(|e| e.position.sequence)).await?;
            let mut stored = self.entries.lock().unwrap();
            for entry in entries.as_slice() {
                stored.insert(entry.position.sequence, entry.clone());
            }
            Ok(())
        })
    }
    fn write_snapshot(&self, snapshot: StorageBatch<RecoverySnapshot>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if self.control.retain_snapshot.load(Ordering::SeqCst) {
                *self.control.retained_snapshot.lock().unwrap() = Some(snapshot.clone());
            }
            let [snapshot] = snapshot.as_slice() else {
                return Err(Error::InvalidInput {
                    field: "state snapshot",
                    reason: "requires exactly one snapshot",
                });
            };
            self.control.write([snapshot.through.sequence]).await?;
            *self.pending_snapshot.lock().unwrap() = Some(snapshot.clone());
            self.control.snapshot_written.notify_one();
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
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits,
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
            let last = entries.last().map_or(after.sequence, |e| e.position.sequence);
            Ok(RecoveryPage {
                entries,
                next: (last < through.sequence).then(|| self.at(last)),
            })
        })
    }
    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition> {
        Box::pin(async move {
            self.control.flush(through.sequence).await?;
            self.saved.store(through.sequence as usize, Ordering::SeqCst);
            let mut pending = self.pending_snapshot.lock().unwrap();
            if pending.as_ref().is_some_and(|snapshot| snapshot.through.sequence <= through.sequence) {
                *self.snapshot.lock().unwrap() = pending.take();
            }
            if self.control.flush_failures.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
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
    Error::backend("test backend", kind, std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected failure"))
}

fn record_matches(row: &StoredRecord, schema: &SchemaKey, scope: &Option<Identifier>, item: &Option<Identifier>) -> bool {
    &row.schema == schema
        && item.as_ref().is_none_or(|id| id == &row.item)
        && scope.as_ref().is_none_or(|scope| Some(scope) == row.scope.as_ref())
}

fn window(start: Option<std::time::SystemTime>, end: Option<std::time::SystemTime>, time: Option<std::time::SystemTime>) -> bool {
    match (start, end) {
        (Some(start), Some(end)) => time.is_some_and(|time| time >= start && time < end),
        _ => true,
    }
}

// Ascending Entries In (page.after, snapshot.at]; At Least One While Any Remain.
fn scan<'a, T: Clone + serde::Serialize + 'a>(
    entries: impl Iterator<Item = &'a T>, snapshot: &BackendSnapshot, page: ScanPage, position: impl Fn(&T) -> &StorageCursor,
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
        if output.len() == page.rows.get() || (!output.is_empty() && bytes + size > page.bytes.get()) {
            break;
        }
        bytes += size;
        output.push(entry.clone());
    }
    Ok(output)
}

fn upsert_release(releases: &mut Vec<Release>, release: Release) {
    if let Some(prior) = releases.iter_mut().find(|prior| prior.schema == release.schema && prior.item == release.item) {
        if prior.through.sequence < release.through.sequence {
            *prior = release;
        }
    } else {
        releases.push(release);
    }
}

/* === Tests === */
