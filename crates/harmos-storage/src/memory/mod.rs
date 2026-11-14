/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "record_store.rs"]
mod record_store;

#[path = "file_backend.rs"]
mod file_backend;
#[path = "read_backend.rs"]
mod read_backend;
/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use crate::query::{FileSelection, ObjectSelection, ObjectVersions, RecordSelection, SchemaSelection, StreamSelection};
use crate::state::*;
use crate::*;
use crate::{BackendFailure, Error, Identifier, MemorySize, Result};
use record_store::{RecordStore, RowStore};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type AcceptedRead = (tokio::sync::oneshot::Sender<()>, tokio::sync::oneshot::Receiver<()>);

/* === Structs === */

struct IoAttempt<'a>(&'a Notify);

#[derive(Default)]
pub struct Control {
    pub record_rows_examined: AtomicUsize,
    pub record_page_rows: AtomicUsize,
    pub record_rows_copied: AtomicUsize,
    pub snapshot_rows_copied: AtomicUsize,
    pub date: Mutex<Option<std::time::SystemTime>>,
    pub fail_projection: AtomicBool,
    pub unsupported_conditional: AtomicBool,
    pub unsupported_sessions: AtomicBool,
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
    pub block_receipt: AtomicBool,
    pub receipt_entered: Notify,
    pub receipt_release: Notify,
    pub definition_lost_replies: AtomicUsize,
    pub block_definition: AtomicBool,
    pub definition_entered: Notify,
    pub definition_release: Notify,
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
    pub production_read_bytes: AtomicUsize,
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
    pub abort_settled: Notify,
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
    pub write_settled: Notify,
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
    pub committed_bytes: Mutex<Vec<usize>>,
    pub flushes: Mutex<Vec<u64>>,
}

struct Snapshot {
    at: StorageCursor,
    pruned_objects: BTreeMap<u64, (SchemaKey, ObjectKey)>,
    blobs: BTreeMap<String, (FileReference, Vec<u8>)>,
    lost_objects_through: u64,
    objects: BTreeMap<u64, ObjectEntry>,
    files: BTreeMap<u64, FileReference>,
    schemas: BTreeMap<u64, StorageSchema>,
    records: RecordStore,
    rows: BTreeMap<u64, StoredRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub schema: SchemaKey,
    pub item: ObjectKey,
    pub through: StorageCursor,
}

#[derive(Default)]
struct Conditional {
    floor: u64,
    receipts: BTreeMap<u64, ([u8; 32], CompareExchange)>,
}

pub struct Backend {
    clock: crate::execution::Execution,
    conditional: Mutex<Conditional>,
    projections: Mutex<crate::query::projection::ProjectionCatalog>,
    indexes: Mutex<crate::query::index::IndexCatalog>,
    pruned_objects: Mutex<BTreeMap<u64, (SchemaKey, ObjectKey)>>,
    pub object_releases: Mutex<Vec<Release>>,
    catalog: ObjectCatalog,
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
    snapshots: Mutex<BTreeMap<Vec<u8>, Arc<Snapshot>>>,
    next_snapshot: AtomicUsize,
    snapshot_scope: usize,
    pub history: StoreHistory,
    pub control: Control,
    pub records: Mutex<RecordStore>,
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

impl Drop for IoAttempt<'_> {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

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
        let _settled = IoAttempt(&self.write_settled);
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

impl Backend {
    pub fn new(history: StoreHistory) -> Self {
        Self::with_catalog(ObjectCatalog::new(), history)
    }
}

impl Backend {
    pub fn with_catalog(catalog: ObjectCatalog, history: StoreHistory) -> Self {
        static NEXT_SCOPE: AtomicUsize = AtomicUsize::new(1);
        Self {
            clock: crate::execution::Execution::default(),
            indexes: Mutex::default(),
            projections: Mutex::default(),
            conditional: Mutex::default(),
            pruned_objects: Mutex::default(),
            object_releases: Mutex::new(Vec::new()),
            catalog,
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
            snapshot_scope: NEXT_SCOPE.fetch_add(1, Ordering::SeqCst),
            history,
            control: Control::default(),
            records: Mutex::default(),
            rows: Mutex::new(BTreeMap::new()),
            schemas: Mutex::new(BTreeMap::new()),
            saved: AtomicUsize::new(0),
        }
    }
    pub fn with_execution(catalog: ObjectCatalog, history: StoreHistory, clock: crate::execution::Execution) -> Self {
        let mut backend = Self::with_catalog(catalog, history);
        backend.clock = clock;
        backend
    }

    fn date(&self) -> std::time::SystemTime {
        self.control.date.lock().unwrap().unwrap_or_else(|| self.clock.read().date)
    }

    fn at(&self, sequence: u64) -> StorageCursor {
        StorageCursor { history: self.history.clone(), sequence }
    }
}

impl StorageBackend for Backend {
    fn set_object<'a>(&'a self, exchange: &'a ObjectExchange) -> StorageFuture<'a, CompareExchange> {
        Box::pin(async move {
            exchange.validate()?;
            self.history.check(&exchange.value.position.history)?;
            self.control.write([exchange.schema.position.sequence, exchange.value.position.sequence]).await?;
            let fingerprint = exchange.fingerprint()?;
            let mut conditional = self.conditional.lock().unwrap();
            let position = exchange.value.position.sequence;
            if let Some((prior, outcome)) = conditional.receipts.get(&position) {
                if prior != &fingerprint {
                    return Err(failure(BackendFailure::Conflict));
                }
                return Ok(outcome.clone());
            }
            if position <= conditional.floor {
                return Err(Error::Gap {
                    requested: position,
                    available: conditional.floor.saturating_add(1),
                });
            }
            if exchange.schema.position.sequence <= self.saved.load(Ordering::SeqCst) as u64 {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut objects = self.objects.lock().unwrap();
            let current = objects
                .values()
                .rev()
                .find(|object| object.value.schema.id == exchange.value.value.schema.id && object.value.item == exchange.value.value.item)
                .map(|object| object.version.clone());
            let outcome = if current == exchange.expected {
                self.catalog.validate_value(&exchange.value.value, NonZeroUsize::MAX)?;
                let mut schemas = self.schemas.lock().unwrap();
                if schemas.values().any(|schema| schema.key == exchange.schema.key && schema.schema != exchange.schema.schema) {
                    return Err(failure(BackendFailure::Conflict));
                }
                schemas.insert(exchange.schema.position.sequence, exchange.schema.clone());
                objects.insert(position, exchange.value.clone());
                CompareExchange::Applied(ObjectReceipt {
                    position: exchange.value.position.clone(),
                    version: exchange.value.version.clone(),
                })
            } else {
                CompareExchange::Conflict { current }
            };
            conditional.receipts.insert(position, (fingerprint, outcome.clone()));
            self.saved.fetch_max(position as usize, Ordering::SeqCst);
            Ok(outcome)
        })
    }
    fn delete_object_receipts<'a>(&'a self, through: &'a StorageCursor) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            if through.sequence > self.saved.load(Ordering::SeqCst) as u64 {
                return Err(failure(BackendFailure::Conflict));
            }
            let mut conditional = self.conditional.lock().unwrap();
            conditional.receipts.retain(|position, _| *position > through.sequence);
            conditional.floor = conditional.floor.max(through.sequence);
            Ok(())
        })
    }

    fn capabilities(&self) -> crate::query::columnar::StorageCapabilities {
        crate::query::columnar::StorageCapabilities {
            file_sessions: !self.control.unsupported_sessions.load(Ordering::Acquire),
            file_deletion: true,
            mutation_receipts: true,
            durable_projections: true,
            conditional_objects: !self.control.unsupported_conditional.load(Ordering::Acquire),
            ..Default::default()
        }
    }
    fn set_projection_definition<'a>(
        &'a self, definition: &'a crate::query::projection::ProjectionDefinition, expected: Option<u64>, rebuild: bool,
        mutation: Option<&'a crate::MutationHeader>,
    ) -> StorageFuture<'a, crate::DefinitionOutcome> {
        Box::pin(async move {
            if self.control.block_definition.load(Ordering::Acquire) {
                self.control.definition_entered.notify_one();
                self.control.definition_release.notified().await;
            }
            let mut ledger = self.mutation_receipts.lock().unwrap();
            let mut definitions = self.projections.lock().unwrap();
            if let Some(header) = mutation
                && let Some(prior) = ledger.get(&header.mutation)
            {
                return header.replay_definition(prior, crate::DefinitionKind::Projection, &definition.name);
            }

            let outcome = crate::DefinitionOutcome::capture(definitions.register(definition, expected, rebuild))?;
            if let Some(header) = mutation {
                ledger.insert(
                    header.mutation.clone(),
                    header.definition(crate::DefinitionKind::Projection, definition.name.clone(), outcome.clone()),
                );
            }
            drop(definitions);
            drop(ledger);
            if mutation.is_some()
                && self
                    .control
                    .definition_lost_replies
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |remaining| remaining.checked_sub(1))
                    .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(outcome)
        })
    }
    fn get_projection_definition<'a>(
        &'a self, name: &'a Identifier, revision: Option<u64>,
    ) -> StorageFuture<'a, (crate::query::projection::ProjectionDefinition, u64)> {
        Box::pin(async move {
            let projections = self.projections.lock().unwrap();
            match revision {
                Some(revision) => Ok((projections.definition_at(name, revision)?, revision)),
                None => projections.definition(name),
            }
        })
    }

    fn get_projection_status<'a>(
        &'a self, name: &'a Identifier, revision: u64, frontier: Option<&'a StorageCursor>,
    ) -> StorageFuture<'a, Option<crate::query::projection::ProjectionStatus>> {
        Box::pin(async move {
            let projections = self.projections.lock().unwrap();
            let frontier = match frontier {
                Some(frontier) => frontier.clone(),
                None => match projections.frontier(name, revision)? {
                    Some(frontier) => frontier,
                    None => return Ok(None),
                },
            };
            self.history.check(&frontier.history)?;
            projections.status(name, revision, &frontier)
        })
    }

    fn set_projection_output(&self, publication: crate::query::projection::ProjectionPublication) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.image(publication.snapshot())?;
            if self.control.fail_projection.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Transient));
            }
            self.projections.lock().unwrap().publish(&publication)
        })
    }
    fn list_projection_rows<'a>(
        &'a self, read: &'a crate::query::projection::ProjectionRead,
    ) -> StorageFuture<'a, crate::query::projection::ProjectionPage> {
        Box::pin(async move {
            self.history.check(&read.frontier.history)?;
            self.projections.lock().unwrap().read(read)
        })
    }
    fn delete_projection_history<'a>(&'a self, name: &'a Identifier, revision: u64, through: &'a StorageCursor) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            if through.sequence > self.saved.load(Ordering::SeqCst) as u64 {
                return Err(failure(BackendFailure::Conflict));
            }
            self.projections.lock().unwrap().retire(name, revision, through)
        })
    }

    fn set_index_definition<'a>(
        &'a self, definition: &'a harmos_query::index::IndexDefinition, expected: Option<u64>, rebuild: bool, _work: ReadLimits,
        cancel: &'a harmos_query::columnar::Cancellation, mutation: Option<&'a crate::MutationHeader>,
    ) -> StorageFuture<'a, crate::DefinitionOutcome> {
        Box::pin(async move {
            if self.control.block_definition.load(Ordering::Acquire) {
                self.control.definition_entered.notify_one();
                self.control.definition_release.notified().await;
            }
            let mut ledger = self.mutation_receipts.lock().unwrap();
            let mut definitions = self.indexes.lock().unwrap();
            if let Some(header) = mutation
                && let Some(prior) = ledger.get(&header.mutation)
            {
                return header.replay_definition(prior, crate::DefinitionKind::Index, &definition.name);
            }
            cancel.check()?;
            let outcome = crate::DefinitionOutcome::capture(definitions.register(definition, expected, rebuild))?;
            if let Some(header) = mutation {
                ledger.insert(
                    header.mutation.clone(),
                    header.definition(crate::DefinitionKind::Index, definition.name.clone(), outcome.clone()),
                );
            }
            drop(definitions);
            drop(ledger);
            if mutation.is_some()
                && self
                    .control
                    .definition_lost_replies
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |remaining| remaining.checked_sub(1))
                    .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(outcome)
        })
    }
    fn get_index_definition<'a>(&'a self, name: &'a Identifier) -> StorageFuture<'a, (harmos_query::index::IndexDefinition, u64)> {
        Box::pin(async move { self.indexes.lock().unwrap().definition(name) })
    }

    fn delete_object_history<'a>(
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
    fn list_record_views<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Option<Vec<RecordView>>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let limit = self.control.record_page_rows.load(Ordering::Relaxed);
            let rows = if limit == 0 { page.rows.get() } else { page.rows.get().min(limit) };
            let mut entries = Vec::new();
            let mut bytes = 0usize;
            for view in image.records.selected_views(&selection.schema, page.after, snapshot.at.sequence) {
                self.control.record_rows_examined.fetch_add(1, Ordering::Relaxed);
                let row = view.record();
                if !record_matches(row, &selection.schema, &selection.scope, &selection.item)
                    || !window(selection.source_start, selection.source_end, row.source_time)
                {
                    continue;
                }
                if entries.len() == rows || (!entries.is_empty() && bytes + view.encoded_bytes() > page.bytes.get()) {
                    break;
                }
                bytes += view.encoded_bytes();
                entries.push(view.clone());
            }
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                let mut record = entries[0].record().clone();
                record.item = Identifier::new("wrong-item").unwrap();
                entries[0] = RecordView::new(record)?;
            }
            Ok(Some(entries))
        })
    }

    fn get_record_position<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, offset: u64,
    ) -> StorageFuture<'a, Option<u64>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            if selection.scope.is_some() || selection.item.is_some() || selection.source_start.is_some() || selection.source_end.is_some() {
                return Ok(None);
            }
            Ok(Some(image.records.seek(&selection.schema, offset, snapshot.at.sequence)))
        })
    }

    fn get_history(&self) -> &StoreHistory {
        &self.history
    }
    fn create_session(&self) -> StorageFuture<'_, StorageCursor> {
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
    fn get_saved_position(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            if self.control.regressed_saved.load(Ordering::SeqCst) {
                return Ok(self.at(0));
            }
            Ok(self.at(self.saved.load(Ordering::SeqCst) as u64))
        })
    }
    fn append_schemas(&self, write: crate::SchemaWrite) -> StorageFuture<'_, crate::SchemaWriteReceipt> {
        Box::pin(async move {
            match write {
                crate::SchemaWrite::Append(batch) => {
                    let saved = self.persist(batch, &self.schemas, |value| &value.position, |_| Ok(())).await?;
                    Ok(crate::SchemaWriteReceipt { saved, mutation: None })
                }
                crate::SchemaWrite::Set(exchange) => {
                    self.history.check(&exchange.value.position.history)?;
                    self.control.write([exchange.value.position.sequence]).await?;
                    let mut ledger = self.mutation_receipts.lock().unwrap();
                    if let Some(receipt) = ledger.get(&exchange.mutation().mutation) {
                        exchange.replay(receipt)?;
                        return Ok(crate::SchemaWriteReceipt {
                            saved: self.at(self.saved.load(Ordering::SeqCst) as u64),
                            mutation: Some(receipt.clone()),
                        });
                    }
                    let mut schemas = self.schemas.lock().unwrap();
                    let prior = schemas.values().rev().find(|value| value.key == exchange.value.key);
                    let unchanged = prior.is_none_or(|value| value.schema == exchange.value.schema);
                    let outcome = if prior.map(|value| &value.position) == exchange.expected.as_ref() && unchanged {
                        exchange.value.schema.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| {
                            Error::InvalidInput {
                                field: "SetSchema",
                                reason: "invalid finite schema graph",
                            }
                        })?;
                        schemas.insert(exchange.value.position.sequence, exchange.value.clone());
                        crate::DefinitionOutcome::Applied {
                            revision: exchange.value.position.sequence,
                        }
                    } else {
                        crate::DefinitionOutcome::Refused {
                            kind: crate::DefinitionRefusal::Conflict,
                            reason: "schema revision changed or graph is immutable; get again and retry".to_owned(),
                        }
                    };
                    let receipt = exchange.receipt(outcome);
                    ledger.insert(receipt.mutation.clone(), receipt.clone());
                    self.saved.store(exchange.value.position.sequence as usize, Ordering::SeqCst);
                    Ok(crate::SchemaWriteReceipt {
                        saved: exchange.value.position.clone(),
                        mutation: Some(receipt),
                    })
                }
            }
        })
    }
    fn append_object_revisions(&self, batch: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor> {
        self.persist(
            batch,
            &self.objects,
            |value| &value.position,
            |object| {
                self.catalog.validate_value(&object.value, NonZeroUsize::MAX)?;
                let definition = self
                    .catalog
                    .definitions()
                    .iter()
                    .try_for_each(|definition| if definition.schema().unwrap() == object.value.schema { Err(*definition) } else { Ok(()) })
                    .err()
                    .unwrap();
                let fields = (definition.declaration().fields)().to_vec();
                self.object_schemas.lock().unwrap().insert((object.value.schema.id.to_string(), object.value.schema.version.get()), fields);
                Ok(())
            },
        )
    }
    fn append_records(&self, batch: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor> {
        let bytes = batch.as_slice().iter().map(|value| crate::encoded_size(value).unwrap()).sum();
        let committing = self.persist(batch, &self.records, |value| &value.position, |_| Ok(()));
        Box::pin(async move {
            let saved = committing.await?;
            self.control.committed_bytes.lock().unwrap().push(bytes);
            Ok(saved)
        })
    }
    fn append_samples(&self, batch: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor> {
        let bytes = batch.as_slice().iter().map(|value| crate::encoded_size(value).unwrap()).sum();
        let committing = self.persist(batch, &self.rows, |value| &value.record.position, |_| Ok(()));
        Box::pin(async move {
            let saved = committing.await?;
            self.control.committed_bytes.lock().unwrap().push(bytes);
            Ok(saved)
        })
    }
    fn append_file_references(&self, batch: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor> {
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
    fn delete_expired_object_revisions<'a>(
        &'a self, through: &'a StorageCursor, max_entries: NonZeroUsize, retention: &'a ObjectCatalog,
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
                    let retention = retention.retention(&object.value.schema)?;
                    if retention == ObjectRetention::All {
                        continue;
                    }
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
                    let object = objects.remove(position).unwrap();
                    self.pruned_objects.lock().unwrap().insert(*position, (object.value.schema, object.value.item));
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
    fn delete_session(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.control.close()?;
            self.snapshots.lock().unwrap().clear();
            Ok(())
        })
    }
    fn create_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        self.open_snapshot(at)
    }
    fn delete_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.release_snapshot(snapshot).await })
    }
    fn list_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        self.read_schemas(snapshot, selection, page)
    }
    fn list_object_revisions<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        self.read_objects(snapshot, selection, page)
    }
    fn list_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        self.read_records(snapshot, selection, page)
    }
    fn list_samples<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        self.read_stream(snapshot, selection, page)
    }
    fn list_identities<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a IdentitySelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredIdentity>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            let mut seen = BTreeSet::new();
            let mut identities = Vec::new();
            for row in image.rows.values().filter(|row| row.record.position.sequence <= snapshot.at.sequence) {
                if seen.insert(serde_json::to_vec(&(&row.record.schema, &row.record.scope, &row.record.item, &row.stream)).unwrap()) {
                    identities.push(StoredIdentity::Stream {
                        schema: row.record.schema.clone(),
                        scope: row.record.scope.clone(),
                        item: row.record.item.clone(),
                        stream: row.stream.clone(),
                        first: row.record.position.clone(),
                    });
                }
            }
            let mut seen = BTreeSet::new();
            for object in image.objects.values().filter(|object| object.position.sequence <= snapshot.at.sequence) {
                if seen.insert(serde_json::to_vec(&(&object.value.schema, &object.value.item)).unwrap()) {
                    identities.push(StoredIdentity::Object {
                        schema: object.value.schema.clone(),
                        key: object.value.item.clone(),
                        first: object.position.clone(),
                    });
                }
            }
            identities.retain(|identity| selection.matches(identity) && identity.first().sequence > page.after);
            identities.sort_by_key(|identity| identity.first().sequence);
            identities.truncate(page.rows.get());
            let required: usize = identities.iter().map(MemorySize::memory_size).sum();
            if required > page.bytes.get() {
                return Err(failure(BackendFailure::Permanent));
            }
            Ok(identities)
        })
    }
    fn list_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        self.read_files(snapshot, selection, page)
    }
    fn get_object<'a>(&'a self, snapshot: &'a BackendSnapshot, query: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>> {
        self.read_object(snapshot, query)
    }

    fn append_mutation_receipts(&self, receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.save_mutations(receipts.as_slice()).await })
    }
    fn list_mutation_receipts(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        self.recover_mutations(limits)
    }
    fn create_file_production<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move { self.create_production(metadata).await })
    }
    fn set_file_writer_generation<'a>(&'a self, file: &'a StagedFile, expected: u64, next: u64) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.fence_production(file, expected, next).await })
    }
    fn set_file_production_checkpoints(&self, checkpoint: StorageBatch<FileCheckpoint>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            for checkpoint in checkpoint.as_slice() {
                self.save_production(checkpoint).await?;
            }
            Ok(())
        })
    }
    fn list_file_production_checkpoints(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        self.recover_productions(limits)
    }
    fn get_file_production<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        self.file_progress(file)
    }
    fn get_file_production_bytes<'a>(&'a self, file: &'a StagedFile, offset: u64, max_bytes: NonZeroUsize) -> StorageFuture<'a, Vec<u8>> {
        self.read_production(file, offset, max_bytes)
    }

    fn delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.delete_file(file).await })
    }

    fn delete_file_production<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.abandon_file(staging).await })
    }
    fn create_file_staging<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move { self.create_file(metadata).await })
    }
    fn append_file_bytes<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()> {
        self.write_file(staging, offset, bytes)
    }
    fn publish_file_production<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move { self.finish_file(staging, position, length, sha256).await })
    }
    fn delete_file_staging<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.abort_file(staging).await })
    }
    fn get_file_bytes<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        self.read_file(snapshot, file, offset, max_bytes)
    }
}

impl Backend {
    fn persist<'a, T: Clone + PartialEq + Send + Sync + 'static, M: RowStore<T> + Send + 'static>(
        &'a self, batch: StorageBatch<T>, stored: &'a Mutex<M>, position: fn(&T) -> &StorageCursor,
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
                stored.lock().unwrap().insert(position(value).sequence, value.clone())?;
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
            self.history.check(&through.history)?;
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

    fn image(&self, snapshot: &BackendSnapshot) -> Result<Arc<Snapshot>> {
        self.snapshots
            .lock()
            .unwrap()
            .get(&snapshot.token)
            .filter(|image| image.at == snapshot.at)
            .cloned()
            .ok_or(Error::NotReady { domain: "expired test snapshot" })
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
    fn get_history(&self) -> &StoreHistory {
        &self.history
    }
    fn create_recovery_session(&self) -> StorageFuture<'_, RecoveryPosition> {
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
    fn append_recovery_entries(&self, entries: StorageBatch<RecoveryEntry>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if self.control.retain_recovery.load(Ordering::SeqCst) {
                *self.control.retained_recovery.lock().unwrap() = Some(entries.clone());
            }
            self.control.write(entries.as_slice().iter().map(|e| e.position.sequence)).await?;
            let mut stored = self.entries.lock().unwrap();
            for entry in entries.as_slice() {
                self.history.check(&entry.position.history)?;
                if stored.get(&entry.position.sequence).is_some_and(|prior| prior != entry) {
                    return Err(Error::backend("append in-memory recovery entries", BackendFailure::Conflict, std::io::Error::other("position contains different recovery bytes")));
                }
            }
            for entry in entries.as_slice() { stored.insert(entry.position.sequence, entry.clone()); }
            Ok(())
        })
    }
    fn set_recovery_snapshot(&self, snapshot: StorageBatch<RecoverySnapshot>) -> StorageFuture<'_, ()> {
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
            self.history.check(&snapshot.through.history)?;
            snapshot.payload.verify(snapshot.payload.segment_bytes()).map_err(|source| Error::backend("write in-memory recovery snapshot", BackendFailure::Corrupt, source))?;
            self.control.write([snapshot.through.sequence]).await?;
            *self.pending_snapshot.lock().unwrap() = Some(snapshot.clone());
            self.control.snapshot_written.notify_one();
            Ok(())
        })
    }
    fn get_recovery_snapshot(&self, maximum: NonZeroUsize) -> StorageFuture<'_, Option<RecoverySnapshot>> {
        Box::pin(async move {
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }
            let snapshot = self.snapshot.lock().unwrap().clone();
            if snapshot.as_ref().is_some_and(|snapshot| snapshot.payload.len() > maximum.get()) {
                return Err(Error::ResourceExhausted { resource: "read in-memory recovery snapshot" });
            }
            Ok(snapshot)
        })
    }
    fn list_recovery_entries<'a>(
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits,
    ) -> StorageFuture<'a, RecoveryPage> {
        Box::pin(async move {
            self.history.check(&after.history)?;
            self.history.check(&through.history)?;
            if after.sequence > through.sequence { return Err(Error::InvalidInput { field: "read in-memory recovery entries", reason: "start exceeds frontier" }); }
            if after.sequence == through.sequence { return Ok(RecoveryPage { entries: Vec::new(), next: None }); }
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
            if entries.is_empty() { return Err(Error::ResourceExhausted { resource: "read in-memory recovery entries" }); }
            let last = entries.last().map_or(after.sequence, |e| e.position.sequence);
            Ok(RecoveryPage {
                entries,
                next: (last < through.sequence).then(|| self.at(last)),
            })
        })
    }
    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition> {
        Box::pin(async move {
            self.history.check(&through.history)?;
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
    fn delete_recovery_session(&self) -> StorageFuture<'_, ()> {
        Box::pin(async { self.control.close() })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn failure(kind: BackendFailure) -> Error {
    Error::backend("in-memory storage", kind, std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected failure"))
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
