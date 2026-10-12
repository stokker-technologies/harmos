/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "storage/frame.rs"]
mod frame;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

// Local Imports
use harmos::catalog::Projection;
use harmos::storage::recovery::*;
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
struct PageView<'a> {
    view: &'a ReadView,
    pinned_bytes: usize,
}

struct QueryCounter {
    bytes: usize,
    limit: usize,
    started: tokio::time::Instant,
    timeout: std::time::Duration,
}

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
    pub allow_bootstrap: AtomicBool,
    pub fail_bootstrap_begin: AtomicBool,
    pub block_bootstrap_finish: AtomicBool,
    pub bootstrap_finish_entered: Notify,
    pub bootstrap_finish_release: Notify,
    pub uncertain_bootstrap_finish: AtomicBool,
    pub fail_bootstrap_abort: AtomicBool,
    pub bootstrap_begins: AtomicUsize,
    pub bootstrap_aborts: AtomicUsize,
    pub bootstrap_file_writes: AtomicUsize,
    pub bootstrap_file_finishes: AtomicUsize,
    pub bootstrap_file_aborts: AtomicUsize,
    pub refuse_latest_objects: AtomicBool,
    pub allow_empty_browse: AtomicBool,
    pub browse_request: Mutex<Option<ObjectBrowseRequest>>,
    pub browse_rows: Mutex<Option<Vec<ObjectMetadata>>>,
    pub query_views: AtomicUsize,
    pub frame_pages: AtomicUsize,
    pub frame_decodes: AtomicUsize,
    pub frame_sorted: Mutex<Option<frame::SortedScan>>,
    pub frame_capture_only: AtomicBool,
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
    pub accepted_record_read: Mutex<Option<AcceptedRead>>,
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
    pub view_ids: Mutex<Vec<Identifier>>,
    pub open: AtomicBool,
    pub closes: AtomicUsize,
    pub writes: Mutex<Vec<u64>>,
    pub write_sizes: Mutex<Vec<usize>>,
    pub write_bytes: Mutex<Vec<usize>>,
    pub flushes: Mutex<Vec<u64>>,
}

struct Snapshot {
    query_bytes: usize,
    lost_objects_through: u64,
    lost_files_through: u64,
    retention_generation: u64,
    objects: BTreeMap<u64, ObjectEntry>,
    files: BTreeMap<u64, FileReference>,
    schemas: BTreeMap<u64, StorageSchema>,
    records: BTreeMap<u64, StoredRecord>,
    rows: BTreeMap<u64, StoredRow>,
}

struct Import {
    identity: BootstrapId,
    prior: StorageCursor,
    entries: Snapshot,
}

pub struct Data<C = ()> {
    pub schema_encoding: SchemaEncoding,
    catalog: PhantomData<fn() -> C>,
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
    pub retention_generation: AtomicUsize,
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
    import: Mutex<Option<Import>>,
    committed_import: Mutex<Option<BootstrapManifest>>,
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
impl Write for QueryCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.started.elapsed() >= self.timeout {
            return Err(io::Error::other("query time exceeded"));
        }
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|bytes| *bytes <= self.limit)
            .ok_or_else(|| io::Error::other("query allocation bound exceeded"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Control {
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
            catalog: PhantomData,
            schema_encoding: SchemaEncoding::Inline,
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
            retention_generation: AtomicUsize::new(1),
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
            import: Mutex::default(),
            committed_import: Mutex::default(),
        }
    }
    fn at(&self, sequence: u64) -> StorageCursor {
        StorageCursor {
            history: self.history.clone(),
            sequence,
        }
    }
}

impl<C: ObjectCatalog> StorageBackend for Data<C> {
    fn schema_encoding(&self) -> SchemaEncoding {
        self.schema_encoding
    }
    fn supports_scan_order(
        &self,
        _scan: &harmos::storage::frame::Scan,
        order: &harmos::storage::frame::ScanOrder,
    ) -> bool {
        matches!(order, harmos::storage::frame::ScanOrder::Capture)
            || !self.control.frame_capture_only.load(Ordering::SeqCst)
    }
    fn scan_ordered<'a>(
        &'a self,
        view: &'a ReadView,
        scan: &'a harmos::storage::frame::Scan,
        order: &'a harmos::storage::frame::ScanOrder,
        cursor: Option<&'a harmos::storage::frame::ScanCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, harmos::storage::frame::ScanPage> {
        self.frame_scan(view, scan, order, cursor, limits)
    }
    fn validate_object_versions(&self, versions: ObjectVersions) -> Result<()> {
        if versions == ObjectVersions::Latest
            && self.control.refuse_latest_objects.load(Ordering::SeqCst)
        {
            return Err(Error::InvalidInput {
                field: "object query",
                reason: "test backend refuses latest object selection",
            });
        }
        Ok(())
    }
    fn validate_query_limits(&self, limits: QueryLimits) -> Result<()> {
        limits.check()
    }

    fn read_query_view<'a>(
        &'a self,
        saved: &'a StorageCursor,
        limits: QueryLimits,
    ) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            self.control.query_views.fetch_add(1, Ordering::SeqCst);
            self.validate_query_limits(limits)?;
            let started = tokio::time::Instant::now();
            if self.control.block_view.load(Ordering::SeqCst) {
                self.control.view_entered.notify_one();
                self.control.view_release.notified().await;
            }
            // Lock all source maps while measuring and cloning. Refuse before
            // allocation; this fixture has no disk spill or detached work.
            let objects = self.objects.lock().unwrap();
            let files = self.files.lock().unwrap();
            let schemas = self.schemas.lock().unwrap();
            let records = self.records.lock().unwrap();
            let rows = self.rows.lock().unwrap();
            // Preclaim Native Snapshot Maps, Index Nodes And Both Token Buffers.
            let query_bytes = [
                snapshot_map_memory(&objects),
                snapshot_map_memory(&files),
                snapshot_map_memory(&schemas),
                snapshot_map_memory(&records),
                snapshot_map_memory(&rows),
                harmos::btree_map_memory::<Vec<u8>, Snapshot>(1),
                saved.memory_size(),
                2 * std::mem::size_of::<u64>(),
            ]
            .into_iter()
            .try_fold(std::mem::size_of::<Snapshot>(), |bytes, next| {
                bytes
                    .checked_add(next)
                    .filter(|bytes| *bytes <= limits.memory_bytes.get())
                    .ok_or(Error::ResourceExhausted {
                        resource: "test query memory",
                    })
            })?;
            if started.elapsed() >= limits.timeout {
                return Err(Error::ResourceExhausted {
                    resource: "test query time",
                });
            }
            let snapshot = Snapshot {
                lost_files_through: self.lost_files_through.load(Ordering::SeqCst) as u64,
                retention_generation: self.retention_generation.load(Ordering::SeqCst) as u64,
                query_bytes,
                lost_objects_through: self.control.lost_objects_through.load(Ordering::SeqCst)
                    as u64,
                objects: snapshot_map(&objects),
                files: snapshot_map(&files),
                schemas: snapshot_map(&schemas),
                records: snapshot_map(&records),
                rows: snapshot_map(&rows),
            };
            if started.elapsed() >= limits.timeout {
                return Err(Error::ResourceExhausted {
                    resource: "test query time",
                });
            }
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
                saved: saved.clone(),
                token,
            })
        })
    }

    type Catalog = C;
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
    fn supports_retained_snapshots(&self) -> bool {
        self.control.allow_bootstrap.load(Ordering::SeqCst)
    }
    fn read_retained_view<'a>(
        &'a self,
        through: &'a StorageCursor,
        limits: QueryLimits,
    ) -> StorageFuture<'a, RetainedSnapshot> {
        Box::pin(async move {
            if !self.control.allow_bootstrap.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            let snapshot = self.read_query_view(through, limits).await?;
            let snapshots = self.snapshots.lock().unwrap();
            let retained = snapshots.get(&snapshot.token).unwrap();
            let complete_after = retained
                .lost_objects_through
                .max(retained.lost_files_through);
            let generation = NonZeroU64::new(retained.retention_generation).unwrap();
            Ok(RetainedSnapshot {
                snapshot,
                coverage: RetainedCoverage {
                    cut: through.clone(),
                    complete_after,
                    retention_generation: generation,
                },
            })
        })
    }
    fn begin_bootstrap<'a>(
        &'a self,
        identity: &'a BootstrapId,
        saved: &'a StorageCursor,
        _limits: QueryLimits,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.bootstrap_begins.fetch_add(1, Ordering::SeqCst);
            if !self.control.allow_bootstrap.load(Ordering::SeqCst)
                || self.control.fail_bootstrap_begin.load(Ordering::SeqCst)
                || self.at(self.saved.load(Ordering::SeqCst) as u64) != *saved
                || !identity.cut().covers(saved)?
            {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut import = self.import.lock().unwrap();
            if let Some(previous) = import.as_ref() {
                return if previous.identity == *identity && previous.prior == *saved {
                    Ok(())
                } else {
                    Err(failure(BackendFailure::Conflict))
                };
            }
            *import = Some(Import {
                identity: identity.clone(),
                prior: saved.clone(),
                entries: Snapshot {
                    query_bytes: 0,
                    lost_objects_through: 0,
                    lost_files_through: 0,
                    retention_generation: 1,
                    objects: BTreeMap::new(),
                    files: BTreeMap::new(),
                    schemas: BTreeMap::new(),
                    records: BTreeMap::new(),
                    rows: BTreeMap::new(),
                },
            });
            Ok(())
        })
    }
    fn write_bootstrap<'a>(&'a self, batch: RetainedBatch<'a>) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut state = self.import.lock().unwrap();
            let import = state
                .as_mut()
                .ok_or_else(|| failure(BackendFailure::Conflict))?;
            if import.identity != *batch.identity() {
                return Err(failure(BackendFailure::Conflict));
            }
            for entry in batch.part().entries() {
                let position = entry.position().sequence;
                let existing = usize::from(import.entries.objects.contains_key(&position))
                    + usize::from(import.entries.schemas.contains_key(&position))
                    + usize::from(import.entries.records.contains_key(&position))
                    + usize::from(import.entries.rows.contains_key(&position))
                    + usize::from(import.entries.files.contains_key(&position));
                macro_rules! insert {
                    ($map:ident, $entry:expr) => {{
                        if existing > 0 && import.entries.$map.get(&position) != Some($entry) {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        import.entries.$map.insert(position, $entry.clone());
                    }};
                }
                match entry {
                    StorageEntry::Schema(value) => insert!(schemas, value),
                    StorageEntry::Object(value) => insert!(objects, value),
                    StorageEntry::Record(value) => insert!(records, value),
                    StorageEntry::Row(value) => insert!(rows, value),
                    StorageEntry::File(value) => insert!(files, value),
                }
            }
            Ok(())
        })
    }
    fn create_bootstrap_file<'a>(
        &'a self,
        identity: &'a BootstrapId,
        metadata: &'a FileMetadata,
    ) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            if self
                .import
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|import| import.identity != *identity)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.create_file(metadata).await
        })
    }
    fn write_bootstrap_file<'a>(
        &'a self,
        identity: &'a BootstrapId,
        staging: &'a StagedFile,
        offset: u64,
        bytes: &'a [u8],
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self
                .import
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|import| import.identity != *identity)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control
                .bootstrap_file_writes
                .fetch_add(1, Ordering::SeqCst);
            self.write_file(staging, offset, bytes).await
        })
    }
    fn finish_bootstrap_file<'a>(
        &'a self,
        identity: &'a BootstrapId,
        staging: &'a StagedFile,
        position: &'a StorageCursor,
        length: u64,
        sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            if self
                .import
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|import| import.identity != *identity)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control
                .bootstrap_file_finishes
                .fetch_add(1, Ordering::SeqCst);
            self.finish_file(staging, position, length, sha256).await
        })
    }
    fn abort_bootstrap_file<'a>(
        &'a self,
        identity: &'a BootstrapId,
        staging: &'a StagedFile,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if self
                .import
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|import| import.identity != *identity)
            {
                return Err(failure(BackendFailure::Conflict));
            }
            self.control
                .bootstrap_file_aborts
                .fetch_add(1, Ordering::SeqCst);
            self.abort_file(staging).await
        })
    }
    fn finish_bootstrap<'a>(
        &'a self,
        manifest: &'a BootstrapManifest,
    ) -> StorageFuture<'a, StorageCursor> {
        Box::pin(async move {
            if self.control.block_bootstrap_finish.load(Ordering::SeqCst) {
                self.control.bootstrap_finish_entered.notify_one();
                self.control.bootstrap_finish_release.notified().await;
            }
            manifest.check_complete()?;
            let _io = self.file_io.lock().unwrap();
            if let Some(committed) = self.committed_import.lock().unwrap().as_ref()
                && committed.identity() == manifest.identity()
            {
                return if committed == manifest {
                    Ok(manifest.identity().cut().clone())
                } else {
                    Err(failure(BackendFailure::Conflict))
                };
            }
            let mut state = self.import.lock().unwrap();
            let import = state
                .as_ref()
                .ok_or_else(|| failure(BackendFailure::Conflict))?;
            if import.identity != *manifest.identity()
                || self.at(self.saved.load(Ordering::SeqCst) as u64) != import.prior
            {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut actual = BootstrapManifest::new(import.identity.clone());
            macro_rules! verify {
                ($map:ident, $part:ident, $kind:ident) => {{
                    let values: Vec<_> = import.entries.$map.values().cloned().collect();
                    if !values.is_empty() {
                        let batch = RetainedBatch::new(
                            &import.identity,
                            StoragePart::$part(&values),
                            ReadLimits {
                                rows: NonZeroUsize::MAX,
                                bytes: NonZeroUsize::MAX,
                            },
                        )?;
                        actual.record(batch)?;
                    }
                    actual.finish_collection(ReadKind::$kind);
                }};
            }
            verify!(schemas, Schemas, Schemas);
            verify!(objects, Objects, Objects);
            verify!(records, Records, Records);
            verify!(rows, Stream, Stream);
            verify!(files, Files, Files);
            if actual != *manifest {
                return Err(failure(BackendFailure::Corrupt));
            }
            for file in import.entries.files.values() {
                if self
                    .blobs
                    .lock()
                    .unwrap()
                    .get(file.metadata.id.as_str())
                    .is_none_or(|(reference, bytes)| {
                        reference != file
                            || bytes.len() as u64 != file.length
                            || <[u8; 32]>::from(Sha256::digest(bytes)) != file.sha256
                    })
                {
                    return Err(failure(BackendFailure::Corrupt));
                }
            }
            *self.schemas.lock().unwrap() = import.entries.schemas.clone();
            *self.objects.lock().unwrap() = import.entries.objects.clone();
            *self.records.lock().unwrap() = import.entries.records.clone();
            *self.rows.lock().unwrap() = import.entries.rows.clone();
            {
                let mut files = self.files.lock().unwrap();
                *files = import.entries.files.clone();
                // Import Retains The Source's Certified Floor, Including Holes.
                self.control
                    .lost_objects_through
                    .fetch_max(import.identity.complete_after() as usize, Ordering::SeqCst);
                self.lost_files_through
                    .fetch_max(import.identity.complete_after() as usize, Ordering::SeqCst);
                self.retention_generation.fetch_max(
                    import.identity.retention_generation().get() as usize,
                    Ordering::SeqCst,
                );
            }
            self.saved
                .store(import.identity.cut().sequence as usize, Ordering::SeqCst);
            *self.committed_import.lock().unwrap() = Some(manifest.clone());
            *state = None;
            if self
                .control
                .uncertain_bootstrap_finish
                .swap(false, Ordering::SeqCst)
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(manifest.identity().cut().clone())
        })
    }
    fn abort_bootstrap<'a>(&'a self, identity: &'a BootstrapId) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.bootstrap_aborts.fetch_add(1, Ordering::SeqCst);
            if self.control.fail_bootstrap_abort.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Transient));
            }
            let mut import = self.import.lock().unwrap();
            if import
                .as_ref()
                .is_some_and(|import| import.identity == *identity)
            {
                *import = None;
            }
            Ok(())
        })
    }
    fn write<'a>(&'a self, batch: StorageBatch<'a>) -> StorageFuture<'a, StorageCursor> {
        Box::pin(async move {
            if self.import.lock().unwrap().is_some() {
                return Err(failure(BackendFailure::Conflict));
            }
            self.history.check(&batch.first().history)?;
            self.control
                .write_bytes
                .lock()
                .unwrap()
                .push(batch.encoded_bytes());
            self.control
                .write(batch.entries().map(|entry| entry.position().sequence))
                .await?;
            // Physical writes can be partially committed before an uncertain
            // result. Replays reconcile identical content at every position.
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
                            definition.fields().to_vec(),
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
                    let keep = definitions
                        .try_for_each(|definition| {
                            if definition.schema().unwrap().id == object.value.schema.id {
                                Err(*definition)
                            } else {
                                Ok(())
                            }
                        })
                        .err()
                        .unwrap()
                        .retention()
                        .versions()
                        .get();
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
                for position in &remove {
                    objects.remove(position);
                    self.control
                        .lost_objects_through
                        .fetch_max(*position as usize, Ordering::SeqCst);
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
    fn read_view<'a>(&'a self, saved: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            if self.control.block_view.load(Ordering::SeqCst) {
                self.control.view_entered.notify_one();
                self.control.view_release.notified().await;
            }
            let token = self
                .next_snapshot
                .fetch_add(1, Ordering::SeqCst)
                .to_le_bytes()
                .to_vec();
            let (objects, lost_objects_through) = {
                let objects = self.objects.lock().unwrap();
                (
                    objects.clone(),
                    self.control.lost_objects_through.load(Ordering::SeqCst) as u64,
                )
            };
            let files = self.files.lock().unwrap();
            let snapshot = Snapshot {
                lost_files_through: self.lost_files_through.load(Ordering::SeqCst) as u64,
                retention_generation: self.retention_generation.load(Ordering::SeqCst) as u64,
                query_bytes: 0,
                objects,
                lost_objects_through,
                files: files.clone(),
                schemas: self.schemas.lock().unwrap().clone(),
                records: self.records.lock().unwrap().clone(),
                rows: self.rows.lock().unwrap().clone(),
            };
            self.snapshots
                .lock()
                .unwrap()
                .insert(token.clone(), snapshot);
            Ok(BackendSnapshot {
                saved: saved.clone(),
                token,
            })
        })
    }
    fn release_view<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
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
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StorageSchema>> {
        Box::pin(async move {
            view.check_kind(ReadKind::Schemas, selection, cursor)?;
            self.control
                .view_ids
                .lock()
                .unwrap()
                .push(view.id().clone());
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            page(
                snapshot.schemas.values(),
                PageView {
                    view,
                    pinned_bytes: snapshot.query_bytes,
                },
                selection,
                cursor,
                limits,
                |row| &row.position,
                |row| {
                    selection.schema.as_ref().is_none_or(|key| key == &row.key)
                        && selection
                            .item
                            .as_ref()
                            .is_none_or(|item| item == &row.key.id)
                },
            )
        })
    }
    fn validate_object_browse(
        &self,
        request: &ObjectBrowseRequest,
        _limits: QueryLimits,
    ) -> Result<()> {
        *self.control.browse_request.lock().unwrap() = Some(request.clone());
        if self.control.browse_rows.lock().unwrap().is_some()
            || (self.control.allow_empty_browse.load(Ordering::SeqCst)
                && self.objects.lock().unwrap().is_empty())
        {
            Ok(())
        } else {
            Err(Error::InvalidInput {
                field: "object browse",
                reason: "test backend only supports explicitly enabled empty browsing",
            })
        }
    }
    fn read_object_browse<'a>(
        &'a self,
        view: &'a ReadView,
        request: &'a ObjectBrowseRequest,
        cursor: Option<&'a ObjectBrowseCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ObjectBrowsePage> {
        Box::pin(async move {
            let _computation = view.retain_computation()?;
            let scripted = self.control.browse_rows.lock().unwrap();
            let Some(rows) = &*scripted else {
                return Ok(ObjectBrowsePage {
                    entries: Vec::new(),
                    next: None,
                });
            };
            // Scripts Supply Already Selected And Globally Ordered Backend Rows.
            // The Original Reader Still Checks The Plan, Values And Continuation.
            let bound = limits.bytes.get().saturating_mul(4);
            let count = rows.len().min(limits.rows.get());
            let mut retained = count
                .saturating_mul(std::mem::size_of::<ObjectMetadata>())
                .saturating_add(std::mem::size_of::<ObjectBrowsePage>())
                .saturating_add(request.memory_size())
                .saturating_add(view.id().memory_size())
                .saturating_add(std::mem::size_of::<ObjectBrowseCursor>());
            if retained > bound {
                return Err(Error::ResourceExhausted {
                    resource: "test scalar page",
                });
            }
            let mut entries = Vec::with_capacity(count);
            let mut more = false;
            for row in rows {
                if let Some(cursor) = cursor
                    && request.compare(cursor.last(), row)? != std::cmp::Ordering::Less
                {
                    continue;
                }
                if entries.len() == count {
                    more = true;
                    break;
                }
                retained = retained.saturating_add(row.memory_size().saturating_mul(2));
                if retained > bound {
                    return Err(Error::ResourceExhausted {
                        resource: "test scalar page",
                    });
                }
                entries.push(row.clone());
            }
            let next = if more {
                Some(ObjectBrowseCursor::new(
                    view,
                    request.clone(),
                    entries.last().unwrap().clone(),
                )?)
            } else {
                None
            };
            let page = ObjectBrowsePage { entries, next };
            page.check(view, request, cursor, limits)?;
            Ok(page)
        })
    }
    fn read_object<'a>(
        &'a self,
        view: &'a ReadView,
        query: &'a ObjectPoint,
        max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(async move {
            let _computation = view.retain_computation()?;
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            let object = snapshot.objects.values().rev().find(|object| {
                object.position.sequence <= view.saved().sequence
                    && object.value.schema.id == query.schema
                    && object.value.item == query.item
                    && match &query.version {
                        ObjectVersion::Latest => true,
                        ObjectVersion::Exact(version) => version == &object.version,
                    }
            });
            if let Some(object) = object {
                check_query_copy(view, snapshot.query_bytes, object)?;
                let mut counter = QueryCounter {
                    bytes: 0,
                    limit: max_bytes.get(),
                    started: tokio::time::Instant::now(),
                    timeout: view
                        .execution_limits()
                        .map_or(std::time::Duration::MAX, |limits| limits.timeout),
                };
                serde_json::to_writer(&mut counter, object).map_err(|_| Error::InvalidInput {
                    field: "test object",
                    reason: "oversized value",
                })?;
            }
            let mut object = object.cloned();
            if self.control.invalid_page.load(Ordering::SeqCst)
                && let Some(object) = &mut object
            {
                object.value.payload = vec![0xff];
            }
            Ok(object)
        })
    }
    fn read_objects<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<ObjectEntry>> {
        Box::pin(async move {
            view.check_kind(ReadKind::Objects, selection, cursor)?;
            self.validate_object_versions(selection.object_versions)?;
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            let lost = snapshot.lost_objects_through;
            if selection.history == ReadHistory::Complete && selection.after < lost {
                return Err(Error::Gap {
                    requested: selection.after + 1,
                    available: lost + 1,
                });
            }
            // This in-memory fixture builds a borrowed winner index. Charge its
            // nodes before allocation and retain that charge while copying pages.
            let mut latest = BTreeMap::new();
            let mut working = snapshot.query_bytes;
            if selection.object_versions == ObjectVersions::Latest {
                let execution = view
                    .execution_limits()
                    .expect("latest requires a query view");
                let started = tokio::time::Instant::now();
                let through = selection.through.unwrap_or(view.saved().sequence);
                for object in snapshot.objects.values() {
                    if started.elapsed() >= execution.timeout {
                        return Err(Error::ResourceExhausted {
                            resource: "test query time",
                        });
                    }
                    if object.position.sequence > through {
                        break;
                    }
                    let key = (object.value.schema.id.as_str(), object.value.item.as_str());
                    if let Some(position) = latest.get_mut(&key) {
                        *position = object.position.sequence;
                        continue;
                    }
                    working = working
                        .checked_add(1024)
                        .filter(|bytes| *bytes <= execution.memory_bytes.get())
                        .ok_or(Error::ResourceExhausted {
                            resource: "test query memory",
                        })?;
                    latest.insert(key, object.position.sequence);
                }
            }
            let mut page = page(
                snapshot.objects.values(),
                PageView {
                    view,
                    pinned_bytes: working,
                },
                selection,
                cursor,
                limits,
                |object| &object.position,
                |object| {
                    (selection.object_versions == ObjectVersions::Revisions
                        || latest
                            .get(&(object.value.schema.id.as_str(), object.value.item.as_str()))
                            == Some(&object.position.sequence))
                        && selection
                            .schema
                            .as_ref()
                            .is_none_or(|schema| schema == &object.value.schema)
                        && selection
                            .object_key
                            .as_ref()
                            .is_none_or(|item| item == &object.value.item)
                },
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !page.entries.is_empty() {
                page.entries[0].value.payload = vec![0xff];
            }
            Ok(page)
        })
    }
    fn read_records<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StoredRecord>> {
        Box::pin(async move {
            view.check_kind(ReadKind::Records, selection, cursor)?;
            let computation = view.retain_computation()?;
            let accepted = self.control.accepted_record_read.lock().unwrap().take();
            let _computation = if let Some((started, release)) = accepted {
                tokio::spawn(async move {
                    started.send(()).unwrap();
                    release.await.unwrap();
                    computation
                })
                .await
                .map_err(|_| failure(BackendFailure::Permanent))?
            } else {
                computation
            };
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            let mut page = page(
                snapshot.records.values(),
                PageView {
                    view,
                    pinned_bytes: snapshot.query_bytes,
                },
                selection,
                cursor,
                limits,
                |row| &row.position,
                |row| matches_record(row, selection),
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !page.entries.is_empty() {
                page.entries[0].item = Identifier::new("wrong-item").unwrap();
            }
            Ok(page)
        })
    }
    fn read_stream<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StoredRow>> {
        Box::pin(async move {
            view.check_kind(ReadKind::Stream, selection, cursor)?;
            let _computation = view.retain_computation()?;
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            page(
                snapshot.rows.values(),
                PageView {
                    view,
                    pinned_bytes: snapshot.query_bytes,
                },
                selection,
                cursor,
                limits,
                |row| &row.record.position,
                |row| {
                    matches_record(&row.record, selection)
                        && selection.stream.as_ref().is_none_or(|id| id == &row.stream)
                },
            )
        })
    }
    fn read_files<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<FileReference>> {
        Box::pin(async move {
            view.check_kind(ReadKind::Files, selection, cursor)?;
            let held = self.snapshots.lock().unwrap();
            let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
            if selection.history == ReadHistory::Complete
                && selection.after < snapshot.lost_files_through
            {
                return Err(Error::Gap {
                    requested: selection.after + 1,
                    available: snapshot.lost_files_through + 1,
                });
            }
            page(
                snapshot.files.values(),
                PageView {
                    view,
                    pinned_bytes: snapshot.query_bytes,
                },
                selection,
                cursor,
                limits,
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
            let bytes = saved.values().fold(
                std::mem::size_of::<Vec<MutationReceipt>>(),
                |bytes, receipt| bytes.saturating_add(receipt.memory_size()),
            );
            if saved.len() > limits.rows.get() || bytes > limits.bytes.get() {
                return Err(failure(BackendFailure::Permanent));
            }
            Ok(saved.values().cloned().collect())
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
                self.retention_generation.fetch_add(1, Ordering::SeqCst);
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
        view: &'a ReadView,
        file: &'a FileReference,
        offset: u64,
        max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            if !self
                .snapshots
                .lock()
                .unwrap()
                .contains_key(&view.snapshot().token)
            {
                return Err(Error::ViewExpired);
            }
            if !view.accepted().covers(&file.position)? {
                return Err(failure(BackendFailure::Conflict));
            }
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
                if let Some(limits) = view.execution_limits() {
                    let held = self.snapshots.lock().unwrap();
                    let snapshot = held.get(&view.snapshot().token).ok_or(Error::ViewExpired)?;
                    if end.saturating_sub(offset)
                        > limits
                            .memory_bytes
                            .get()
                            .saturating_sub(snapshot.query_bytes)
                    {
                        return Err(Error::ResourceExhausted {
                            resource: "test query memory",
                        });
                    }
                }
                bytes[offset..end].to_vec()
            };
            if self.control.corrupt_file.load(Ordering::SeqCst) && !result.is_empty() {
                result[0] ^= 1;
            }
            let accepted = self.control.accepted_file_read.lock().unwrap().take();
            if let Some((started, release)) = accepted {
                let active = view.retain_computation()?.unwrap();
                let work = tokio::spawn(async move {
                    started.send(()).unwrap();
                    release.await.unwrap();
                    // The Actual Payload And Original Guard Share Mailbox Disposal.
                    (result, active)
                });
                let (result, _active) =
                    work.await.map_err(|_| failure(BackendFailure::Permanent))?;
                return Ok(result);
            }
            Ok(result)
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

impl RecoveryBackend for Recovery {
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

fn matches_record(row: &StoredRecord, selection: &ReadSelection) -> bool {
    selection
        .source_time
        .as_ref()
        .is_none_or(|range| range.contains(row.source_time))
        && selection
            .schema
            .as_ref()
            .is_none_or(|key| key == &row.schema)
        && selection.item.as_ref().is_none_or(|id| id == &row.item)
        && selection
            .scope
            .as_ref()
            .is_none_or(|scope| Some(scope) == row.scope.as_ref())
}

fn check_query_copy<T: serde::Serialize>(
    view: &ReadView,
    pinned_bytes: usize,
    value: &T,
) -> Result<()> {
    let Some(limits) = view.execution_limits() else {
        return Ok(());
    };
    let mut counter = QueryCounter {
        bytes: 0,
        limit: limits
            .memory_bytes
            .get()
            .saturating_sub(pinned_bytes)
            .saturating_sub(1024)
            / 8,
        started: tokio::time::Instant::now(),
        timeout: limits.timeout,
    };
    serde_json::to_writer(&mut counter, value).map_err(|_| Error::ResourceExhausted {
        resource: "test query memory/time",
    })?;
    Ok(())
}

fn snapshot_map_memory<T: MemorySize>(map: &BTreeMap<u64, T>) -> usize {
    if map.is_empty() {
        std::mem::size_of::<BTreeMap<u64, T>>()
    } else {
        map.memory_size()
    }
}

fn snapshot_map<T: Clone>(map: &BTreeMap<u64, T>) -> BTreeMap<u64, T> {
    if map.is_empty() {
        BTreeMap::new()
    } else {
        map.clone()
    }
}

fn page<'a, T: Clone + MemorySize + serde::Serialize + 'a>(
    entries: impl DoubleEndedIterator<Item = &'a T> + Clone,
    held: PageView<'_>,
    selection: &ReadSelection,
    cursor: Option<&ReadCursor>,
    limits: ReadLimits,
    position: impl Fn(&T) -> &StorageCursor,
    matches: impl Fn(&T) -> bool,
) -> Result<ReadPage<T>> {
    let PageView { view, pinned_bytes } = held;
    let _computation = view.retain_computation()?;
    let execution = view.execution_limits();
    let started = tokio::time::Instant::now();
    if execution.is_some_and(|execution| pinned_bytes > execution.memory_bytes.get()) {
        return Err(Error::ResourceExhausted {
            resource: "test query memory",
        });
    }
    let selected = |entry: &T| {
        let at = position(entry).sequence;
        at > selection.after
            && at <= selection.through.unwrap_or(view.accepted().sequence)
            && at <= view.saved().sequence
            && matches(entry)
            && cursor.is_none_or(|cursor| match selection.order {
                ReadOrder::Ascending => at > cursor.position().sequence,
                ReadOrder::Descending => at < cursor.position().sequence,
            })
    };
    let next = |entries: &mut dyn DoubleEndedIterator<Item = &'a T>| match selection.order {
        ReadOrder::Ascending => entries.next(),
        ReadOrder::Descending => entries.next_back(),
    };
    let mut inspecting = entries.clone();
    let mut count = 0usize;
    let mut bytes = 0usize;
    let mut resident = std::mem::size_of::<Vec<T>>();
    let mut more = false;
    while let Some(entry) = next(&mut inspecting) {
        if execution.is_some_and(|limits| started.elapsed() >= limits.timeout) {
            return Err(Error::ResourceExhausted {
                resource: "test query time",
            });
        }
        if !selected(entry) {
            continue;
        }
        if count == limits.rows.get() {
            more = true;
            break;
        }
        let mut counter = QueryCounter {
            bytes: 0,
            limit: limits.bytes.get().saturating_sub(bytes),
            started,
            timeout: execution.map_or(std::time::Duration::MAX, |limits| limits.timeout),
        };
        let encoded = serde_json::to_writer(&mut counter, entry).is_ok();
        let native = resident
            .checked_add(entry.memory_size())
            .filter(|bytes| *bytes <= limits.bytes.get());
        if !encoded || native.is_none() {
            if execution.is_some_and(|limits| started.elapsed() >= limits.timeout) {
                return Err(Error::ResourceExhausted {
                    resource: "test query time",
                });
            }
            if count == 0 {
                return Err(Error::InvalidInput {
                    field: "test page",
                    reason: "oversized entry",
                });
            }
            more = true;
            break;
        }
        resident = native.expect("preflighted native output");
        bytes += counter.bytes;
        count += 1;
    }
    // Page Output Has Its Own Original Reservation, Separate From Pinned Working Memory.
    let mut result =
        harmos::__private::decoding_vector(count).map_err(|_| Error::ResourceExhausted {
            resource: "test page layout",
        })?;
    let mut copying = entries;
    while result.len() < count {
        if execution.is_some_and(|limits| started.elapsed() >= limits.timeout) {
            return Err(Error::ResourceExhausted {
                resource: "test query time",
            });
        }
        let entry =
            next(&mut copying).expect("same immutable snapshot retains preflighted entries");
        if selected(entry) {
            result.push(entry.clone());
        }
    }
    let next = if more {
        Some(ReadCursor::new(
            view,
            selection.clone(),
            position(result.last().unwrap()).clone(),
        )?)
    } else {
        None
    };
    Ok(ReadPage {
        entries: result,
        next,
    })
}

/* === Tests === */
