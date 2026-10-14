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
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
// Third Party Imports
use serde::Serialize;
use tokio::sync::watch;
// Local Imports
use super::admissions::Mutation;
use super::queue::PendingQueue;
use super::receipts::{MutationValue, ReceiptState};
use super::recovery::{RecoveryBackend, RecoveryEntry, RecoveryPage, RecoverySnapshot};
use super::worker::Worker;
use super::{
    ReadLimits, RecoveryPosition, SchemaKey, StorageBackends, StorageCursor, StorageFuture,
    StorageSchema, StorageSettings, StoredRecord, StoredRow,
};
use crate::operations::{OperationManager, resident::Lease};
use crate::services::{Once, ServiceDefinition};
use crate::supervision::executor;
use crate::{Application, Error, Identifier, Result, state::Timestamp};

/* === Constants === */
const WORKER_NAME: &str = "harmos-storage";

// Includes owned envelopes, queue nodes, Arc headers, and cloned metadata.
pub(super) const ENTRY_OVERHEAD: usize = 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RecordWrite<'a> {
    pub schema: &'a SchemaKey,
    pub scope: Option<&'a Identifier>,
    pub item: &'a Identifier,
    pub source: Option<&'a Identifier>,
    pub source_time: Option<Timestamp>,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StreamWrite<'a> {
    pub stream: &'a Identifier,
    pub sequence: u64,
    pub record: RecordWrite<'a>,
}

#[derive(Clone, Debug)]
pub struct StorageStatus {
    pub accepted: StorageCursor,
    pub saved: StorageCursor,
    pub recovery_accepted: RecoveryPosition,
    pub recovery_saved: RecoveryPosition,
    pub pending_bytes: usize,
    pub read_bytes: usize,
    pub mutation_bytes: usize,
    pub pending_batches: usize,
    pub running: bool,
    pub closing: bool,
    pub closed: bool,
    pub failure: Option<Arc<Error>>,
}

pub struct StorageManager<C = ()> {
    pub(super) inner: Arc<Inner>,
    catalog: PhantomData<fn() -> C>,
}

pub(super) struct Inner {
    pub(super) settings: StorageSettings,
    pub(super) primary: Arc<dyn super::backend::ErasedBackend>,
    pub(super) recovery: Arc<dyn RecoveryBackend>,
    pub(super) has_objects: bool,
    pub(super) object_definition:
        fn(&SchemaKey) -> Result<crate::storage::catalog::ObjectDefinition>,
    pub(super) validate_object: fn(&super::ObjectValue, std::num::NonZeroUsize) -> Result<()>,
    pub(super) replicas: Vec<Arc<super::replication::Replica>>,
    pub(super) receipts: super::replication::Receipts,
    pub(super) state: Mutex<State>,
    pub(super) changed: watch::Sender<u64>,
    pub(super) lifecycle: tokio::sync::Mutex<Lifecycle>,
}

#[derive(Default)]
pub(super) struct Lifecycle {
    startup_complete: bool,
    primary_touched: bool,
    recovery_touched: bool,
    primary_open: bool,
    files_recovered: bool,
    receipts_recovered: bool,
    recovery_open: bool,
    primary_closed: bool,
    recovery_closed: bool,
    owner: Option<Box<dyn WorkerOwner>>,
}

struct Owner {
    leases: Mutex<Vec<Lease>>,
}

pub(super) struct State {
    pub(super) reserved: usize,
    pub(super) reserved_batches: usize,
    pub(super) journal_limit: usize,
    pub(super) journal_saved: Option<watch::Sender<crate::state::Position>>,
    pub(super) status: StorageStatus,
    pub(super) ready: bool,
    service_owner: Option<Arc<()>>,
    pub(super) next: u64,
    pub(super) queue: PendingQueue,
    pub(super) scratch_bytes: usize,
    pub(super) read_bytes: usize,
    pub(super) replica_base_bytes: usize,
    pub(super) replication_bytes: usize,
    pub(super) data_attempted: u64,
    pub(super) objects_pruned: u64,
    pub(super) objects_prune_owed: bool,
    pub(super) pruning_objects: bool,
    pub(super) recovery_written: Target,
    pub(super) data_accepted: Target,
    pub(super) recovery_accepted: Target,
    pub(super) data_flushed: u64,
    pub(super) recovery_flushed: u64,
    pub(super) data_request: Option<Target>,
    pub(super) recovery_request: Option<Target>,
    pub(super) retry: u64,
    pub(super) views: BTreeMap<u64, super::views::ViewState>,
    pub(super) followers: BTreeMap<u64, super::follow::Following>,
    pub(super) files: BTreeMap<u64, super::staging::FileState>,
    pub(super) receipts: super::receipts::Receipts,
    pub(super) saves: BTreeMap<super::MutationId, super::saving::SaveState>,
    pub(super) deletions: BTreeMap<super::MutationId, super::deletion::Deletion>,
    pub(super) file_cursor: u64,
    pub(super) finishing: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Target {
    pub(super) index: u64,
    pub(super) sequence: u64,
}

pub(super) struct Pending {
    pub(super) journal: Option<crate::state::Position>,
    pub(super) index: u64,
    pub(super) bytes: usize,
    pub(super) operation: Operation,
}

struct Counter(usize);

struct Admission {
    inner: Arc<Inner>,
    active: bool,
}

pub(super) struct MemoryReservation {
    pub(super) inner: Arc<Inner>,
    pub(super) bytes: usize,
    pub(super) kind: ReservationKind,
}

/* === Enums === */
pub enum ErasedCatalog {}

#[derive(Clone, Copy)]
pub(super) enum ReservationKind {
    Read,
    Write,
    Replication,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Domain {
    Data,
    Recovery,
}

pub(super) enum Operation {
    Schema(StorageSchema),
    File(super::FileReference),
    Records(Vec<StoredRecord>),
    ObjectGroup {
        schemas: Vec<StorageSchema>,
        objects: Vec<super::ObjectEntry>,
        bytes: usize,
    },
    Stream(Vec<StoredRow>),
    Recovery(Vec<RecoveryEntry>),
    Snapshot(RecoverySnapshot),
}

/* === Traits === */
trait WorkerOwner: Send + Sync {
    fn stop(&self) -> StorageFuture<'_, ()>;
}

/* === Implementations === */
impl<C> Clone for StorageManager<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            catalog: PhantomData,
        }
    }
}

impl<C> StorageManager<C> {
    pub(crate) fn erased(&self) -> StorageManager<ErasedCatalog> {
        StorageManager {
            inner: self.inner.clone(),
            catalog: PhantomData,
        }
    }
}

impl<C: super::ObjectCatalog> StorageManager<C> {
    pub fn new(backends: StorageBackends<C>, settings: StorageSettings) -> Result<Self> {
        C::validate()?;
        settings.check()?;
        let StorageBackends {
            primary,
            recovery,
            replicas: targets,
        } = backends;
        primary.history().check(recovery.history())?;
        let (replicas, replica_base_bytes) =
            super::replication::configure(&settings, &primary, &targets)?;
        let history = primary.history().clone();
        let position = StorageCursor {
            history: history.clone(),
            sequence: 0,
        };
        let recovery_position = RecoveryPosition {
            history,
            sequence: 0,
        };
        let receipts = super::receipts::Receipts::new(settings.mutation_retention)?;
        Ok(Self {
            catalog: PhantomData,
            inner: Arc::new(Inner {
                settings,
                primary: super::backend::erase(primary),
                recovery,
                validate_object: crate::storage::catalog::validate_transfer::<C>,
                has_objects: C::definition_count() != 0,
                object_definition: crate::storage::catalog::definition::<C>,
                replicas,
                receipts: super::replication::Receipts::default(),
                state: Mutex::new(State {
                    reserved: 0,
                    reserved_batches: 0,
                    journal_limit: 0,
                    journal_saved: None,
                    status: StorageStatus {
                        accepted: position.clone(),
                        saved: position,
                        recovery_accepted: recovery_position.clone(),
                        recovery_saved: recovery_position,
                        pending_bytes: replica_base_bytes,
                        read_bytes: 0,
                        mutation_bytes: 0,
                        pending_batches: 0,
                        running: false,
                        closing: false,
                        closed: false,
                        failure: None,
                    },
                    ready: false,
                    service_owner: None,
                    next: 0,
                    queue: PendingQueue::default(),
                    scratch_bytes: 0,
                    read_bytes: 0,
                    replica_base_bytes,
                    replication_bytes: 0,
                    data_attempted: 0,
                    objects_pruned: 0,
                    objects_prune_owed: false,
                    pruning_objects: false,
                    recovery_written: Target::default(),
                    data_accepted: Target::default(),
                    recovery_accepted: Target::default(),
                    data_flushed: 0,
                    recovery_flushed: 0,
                    data_request: None,
                    recovery_request: None,
                    retry: 0,
                    views: BTreeMap::new(),
                    followers: BTreeMap::new(),
                    files: BTreeMap::new(),
                    saves: BTreeMap::new(),
                    deletions: BTreeMap::new(),
                    receipts,
                    file_cursor: 0,
                    finishing: None,
                }),
                changed: watch::channel(0).0,
                lifecycle: tokio::sync::Mutex::new(Lifecycle::default()),
            }),
        })
    }
}

impl<C> StorageManager<C> {
    // Start the storage workers as infrastructure services of this runtime.
    pub async fn start<A: Application>(&self, runtime: &crate::Runtime<A>) -> Result<()> {
        self.start_on::<A>(&runtime.operations).await
    }

    pub(crate) async fn start_on<A: Application>(
        &self,
        operations: &OperationManager,
    ) -> Result<()> {
        self.claim_owner(operations.resident_identity())?;
        let mut lifecycle = self.inner.lifecycle.lock().await;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if state.ready && lifecycle.startup_complete {
                return Ok(());
            }
            if state.ready {
                return Err(Error::AlreadyRegistered {
                    domain: "storage",
                    name: WORKER_NAME.to_owned(),
                });
            }
        }
        self.acquire_locked(&mut lifecycle).await?;
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        let primary = operations.start_service::<A>(
            Once::new(ServiceDefinition::new(WORKER_NAME), Worker(self.erased())),
            true,
            false,
        )?;
        state.ready = true;
        state.status.running = true;
        drop(state);
        let mut owner = Owner {
            leases: Mutex::new(vec![primary]),
        };
        for replica in &self.inner.replicas {
            let name = format!("harmos-storage-replica-{}", replica.config.id);
            replica.started();
            let worker = super::replication::Worker {
                manager: self.erased(),
                replica: replica.clone(),
                reservation: None,
            };
            match operations.start_service::<A>(
                Once::new(ServiceDefinition::new(name.clone()), worker),
                true,
                false,
            ) {
                Ok(lease) => {
                    owner.leases.get_mut().unwrap().push(lease);
                }
                Err(error) => {
                    replica.state.lock().unwrap().status.running = false;
                    lifecycle.owner = Some(Box::new(owner));
                    self.inner.notify();
                    return Err(error);
                }
            }
        }
        lifecycle.owner = Some(Box::new(owner));
        lifecycle.startup_complete = true;
        self.inner.notify();
        Ok(())
    }

    pub(crate) async fn acquire(&self) -> Result<()> {
        let mut lifecycle = self.inner.lifecycle.lock().await;
        self.acquire_locked(&mut lifecycle).await
    }

    async fn acquire_locked(&self, lifecycle: &mut Lifecycle) -> Result<()> {
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if !lifecycle.primary_open {
            lifecycle.primary_touched = true;
            let saved = self.inner.bounded(self.inner.primary.open()).await?;
            self.inner.primary.history().check(&saved.history)?;
            let mut state = self.inner.state.lock().unwrap();
            state.status.accepted = saved.clone();
            state.status.saved = saved;
            state.data_attempted = state.status.accepted.sequence;
            state.data_accepted.sequence = state.status.accepted.sequence;
            lifecycle.primary_open = true;
        }
        if !lifecycle.files_recovered {
            self.recover_files().await?;
            lifecycle.files_recovered = true;
        }
        if !lifecycle.receipts_recovered {
            self.recover_receipts().await?;
            lifecycle.receipts_recovered = true;
        }
        if !lifecycle.recovery_open {
            lifecycle.recovery_touched = true;
            let saved = self.inner.bounded(self.inner.recovery.open()).await?;
            self.inner.recovery.history().check(&saved.history)?;
            let mut state = self.inner.state.lock().unwrap();
            state.status.recovery_accepted = saved.clone();
            state.status.recovery_saved = saved;
            state.recovery_written.sequence = state.status.recovery_accepted.sequence;
            state.recovery_accepted.sequence = state.status.recovery_accepted.sequence;
            lifecycle.recovery_open = true;
        }
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        Ok(())
    }

    pub(crate) fn same_instance<D>(&self, other: &StorageManager<D>) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn claim_owner(&self, owner: Arc<()>) -> Result<()> {
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if let Some(current) = &state.service_owner {
            if !Arc::ptr_eq(current, &owner) {
                return Err(Error::AlreadyRegistered {
                    domain: "storage owner",
                    name: WORKER_NAME.to_owned(),
                });
            }
        } else {
            state.service_owner = Some(owner);
        }
        Ok(())
    }

    pub fn cursor_accepted(&self) -> StorageCursor {
        self.inner.state.lock().unwrap().status.accepted.clone()
    }

    pub fn cursor_saved(&self) -> StorageCursor {
        self.inner.state.lock().unwrap().status.saved.clone()
    }

    pub fn status(&self) -> StorageStatus {
        self.inner.state.lock().unwrap().status.clone()
    }

    pub async fn write_schema(
        &self,
        key: &SchemaKey,
        schema: &crate::schema::graph::Schema,
    ) -> Result<StorageCursor> {
        self.admit_schema(key, schema, None).await
    }

    pub(super) async fn admit_schema(
        &self,
        key: &SchemaKey,
        schema: &crate::schema::graph::Schema,
        mutation: Option<&crate::storage::MutationId>,
    ) -> Result<StorageCursor> {
        let scratch = schema
            .validation_memory_bytes(&harmos_contract::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput {
                field: "storage schema",
                reason: "invalid message field budget",
            })?;
        let mutation = mutation
            .map(|id| super::admissions::schema_mutation(id, key, schema))
            .transpose()?;
        // Replay Completed Admission Before Scratch Pressure Or Later I/O Errors.
        // Admission Rechecks The Same Ledger After Every Suspension.
        if let Some(mutation) = mutation {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if !state.ready || !state.status.running {
                return Err(Error::NotReady { domain: "storage" });
            }
            if let ReceiptState::Complete(value) =
                state.receipts.lookup(mutation.id, &mutation.fingerprint)?
            {
                return value.cursor();
            }
        }
        let scratch = self.reserve_scratch(scratch).await?;
        schema
            .prepare(&harmos_contract::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput {
                field: "storage schema",
                reason: "invalid message fields",
            })?;
        drop(scratch);
        let bytes = StorageSchema::resident_bytes(key, schema, self.inner.primary.history())?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, 1, mutation, |sequence| {
                Ok(Operation::Schema(StorageSchema::new(
                    self.position(sequence),
                    key.clone(),
                    schema.clone(),
                )))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub async fn write_records(&self, records: &[RecordWrite<'_>]) -> Result<StorageCursor> {
        self.admit_records(records.iter().copied(), None).await
    }

    pub(super) async fn admit_records<'a>(
        &self,
        records: impl ExactSizeIterator<Item = RecordWrite<'a>> + Clone,
        mutation: Option<Mutation<'_>>,
    ) -> Result<StorageCursor> {
        if records.len() == 0 {
            return Ok(self.status().accepted);
        }
        let bytes = records.clone().try_fold(0usize, |bytes, record| {
            bytes
                .checked_add(record.resident_bytes(self.inner.primary.history())?)
                .ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "size overflow",
                })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, records.len(), mutation, |first| {
                Ok(Operation::Records(
                    records
                        .enumerate()
                        .map(|(index, record)| record.stored(self.position(first + index as u64)))
                        .collect(),
                ))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub async fn write_stream(&self, rows: &[StreamWrite<'_>]) -> Result<StorageCursor> {
        self.admit_stream(rows.iter().copied(), None).await
    }

    pub(super) async fn admit_stream<'a>(
        &self,
        rows: impl ExactSizeIterator<Item = StreamWrite<'a>> + Clone,
        mutation: Option<Mutation<'_>>,
    ) -> Result<StorageCursor> {
        if rows.len() == 0 {
            return Ok(self.status().accepted);
        }
        let bytes = rows.clone().try_fold(0usize, |bytes, row| {
            bytes
                .checked_add(row.record.resident_bytes(self.inner.primary.history())?)
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<StoredRow>() - std::mem::size_of::<StoredRecord>(),
                    )
                })
                .and_then(|bytes| bytes.checked_add(row.stream.as_str().len()))
                .ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "size overflow",
                })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, rows.len(), mutation, |first| {
                Ok(Operation::Stream(
                    rows.enumerate()
                        .map(|(index, row)| StoredRow {
                            record: row.record.stored(self.position(first + index as u64)),
                            stream: row.stream.clone(),
                            sequence: row.sequence,
                        })
                        .collect(),
                ))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub async fn write_recovery(&self, payloads: &[&[u8]]) -> Result<RecoveryPosition> {
        self.check_raw_recovery()?;
        if payloads.is_empty() {
            return Ok(self.status().recovery_accepted);
        }
        let bytes = payloads.iter().try_fold(0usize, |bytes, payload| {
            bytes
                .checked_add(recovery_resident_bytes(
                    payload.len(),
                    self.inner.recovery.history(),
                )?)
                .ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "size overflow",
                })
        })?;
        let sequence = self
            .admit(Domain::Recovery, bytes, payloads.len(), |first| {
                Ok(Operation::Recovery(
                    payloads
                        .iter()
                        .enumerate()
                        .map(|(index, payload)| RecoveryEntry {
                            position: self.recovery_position(first + index as u64),
                            payload: payload.to_vec(),
                        })
                        .collect(),
                ))
            })
            .await?;
        Ok(self.recovery_position(sequence))
    }

    pub async fn write_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<()> {
        self.check_raw_recovery()?;
        self.inner
            .recovery
            .history()
            .check(&snapshot.through.history)?;
        let bytes = recovery_resident_bytes(snapshot.payload.len(), &snapshot.through.history)?;
        self.admit(Domain::Recovery, bytes, 0, |_| {
            Ok(Operation::Snapshot(snapshot.clone()))
        })
        .await?;
        Ok(())
    }

    pub async fn flush(&self) -> Result<StorageCursor> {
        let through = self.cursor_accepted();
        self.flush_until(&through).await
    }

    pub async fn flush_until(&self, through: &StorageCursor) -> Result<StorageCursor> {
        let files = self.capture_files(Some(through))?;
        self.flush_prepared(through, &files).await
    }

    pub(crate) async fn flush_prepared(
        &self,
        through: &StorageCursor,
        files: &Option<super::FileSave>,
    ) -> Result<StorageCursor> {
        self.flush_domain(Domain::Data, Some(through)).await?;
        if files.is_some() {
            let _lifecycle = self.inner.lifecycle.lock().await;
            if self.inner.state.lock().unwrap().status.closed {
                return Err(Error::Closing { domain: "storage" });
            }
            self.persist_files(files).await?;
        }
        Ok(self.cursor_saved())
    }

    pub async fn flush_recovery(&self) -> Result<RecoveryPosition> {
        self.flush_domain(Domain::Recovery, None).await?;
        Ok(self.status().recovery_saved)
    }

    pub async fn read_recovery(
        &self,
        after: &RecoveryPosition,
        through: &RecoveryPosition,
        limits: ReadLimits,
    ) -> Result<RecoveryPage> {
        let _lifecycle = self.inner.lifecycle.lock().await;
        let _reservation = self
            .reserve_read(limits.bytes.get(), limits.rows.get())
            .await?;
        let status = self.status();
        if !status.recovery_saved.covers(through)? {
            return Err(Error::InvalidInput {
                field: "recovery range",
                reason: "frontier is not durable",
            });
        }
        self.check_read_limit(limits.bytes.get())?;
        let page = self
            .inner
            .bounded(self.inner.recovery.read_entries(after, through, limits))
            .await?;
        page.check(after, through, limits, |entry| {
            encoded_size(entry).unwrap_or(usize::MAX)
        })?;
        Ok(page)
    }

    pub async fn read_snapshot(
        &self,
        max_bytes: std::num::NonZeroUsize,
    ) -> Result<Option<RecoverySnapshot>> {
        let _lifecycle = self.inner.lifecycle.lock().await;
        let _reservation = self.reserve_read(max_bytes.get(), 1).await?;
        self.check_read_limit(max_bytes.get())?;
        let snapshot = self
            .inner
            .bounded(self.inner.recovery.read_snapshot(max_bytes))
            .await?;
        if let Some(snapshot) = &snapshot
            && (!self.status().recovery_saved.covers(&snapshot.through)?
                || encoded_size(snapshot)? > max_bytes.get())
        {
            return Err(Error::InvalidInput {
                field: "recovery snapshot",
                reason: "invalid saved boundary or byte size",
            });
        }
        Ok(snapshot)
    }

    pub async fn close(&self) -> Result<()> {
        self.inner.state.lock().unwrap().status.closing = true;
        self.inner.notify();

        let mut lifecycle = self.inner.lifecycle.lock().await;
        let ready = {
            let state = self.inner.state.lock().unwrap();
            state.ready && !state.status.closed
        };
        if ready {
            self.settle_transition().await;
            self.settle_files().await?;
            self.settle_saves().await?;
            self.settle_deletions().await?;
            let files = self.capture_files(None)?;
            self.flush_domain(Domain::Data, None).await?;
            self.persist_files(&files).await?;
            self.retire_files();
            self.flush_recovery().await?;
        }
        self.settle_replicas(ready && lifecycle.startup_complete)
            .await?;
        if ready {
            self.release_views().await?;
        }
        // Closing prevents new maintenance; let an in-flight backend call settle
        // before releasing its physical owner, including uncertain failed calls.
        let mut changed = self.inner.changed.subscribe();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if !state.pruning_objects {
                    break;
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
            }
            changed
                .changed()
                .await
                .map_err(|_| Error::NotReady { domain: "storage" })?;
        }
        if lifecycle.primary_touched && !lifecycle.primary_closed {
            self.inner.primary.close().await?;
            lifecycle.primary_closed = true;
        }
        if lifecycle.recovery_touched && !lifecycle.recovery_closed {
            self.inner.recovery.close().await?;
            lifecycle.recovery_closed = true;
        }
        // Release Recovered Files Even When Startup Failed Before Readiness.
        self.retire_files();
        {
            let mut state = self.inner.state.lock().unwrap();
            state.receipts.clear();
            state.status.closed = true;
            state.collect();
        }
        self.inner.notify();
        if let Some(owner) = &lifecycle.owner {
            owner.stop().await?;
        }
        lifecycle.owner = None;
        // A supervisor stopped before its first poll never enters the worker's
        // running guard. The completed join is authoritative in either case.
        self.inner.state.lock().unwrap().status.running = false;
        self.inner.notify();
        Ok(())
    }

    fn check_raw_recovery(&self) -> Result<()> {
        if self.inner.state.lock().unwrap().journal_saved.is_some() {
            return Err(Error::InvalidInput {
                field: "recovery ownership",
                reason: "managed journal owns recovery writes",
            });
        }
        Ok(())
    }

    pub(super) fn position(&self, sequence: u64) -> StorageCursor {
        StorageCursor {
            history: self.inner.primary.history().clone(),
            sequence,
        }
    }

    fn recovery_position(&self, sequence: u64) -> RecoveryPosition {
        RecoveryPosition {
            history: self.inner.recovery.history().clone(),
            sequence,
        }
    }

    pub(super) fn charge<T: Serialize + ?Sized>(&self, value: &T, entries: usize) -> Result<usize> {
        encoded_size(value)?
            .checked_add(encoded_size(self.inner.primary.history())?.saturating_mul(entries))
            .and_then(|bytes| bytes.checked_add(entries.checked_mul(ENTRY_OVERHEAD)?))
            .ok_or(Error::InvalidInput {
                field: "storage admission",
                reason: "size overflow",
            })
    }

    fn check_read_limit(&self, bytes: usize) -> Result<()> {
        if bytes > self.inner.settings.read_memory_bytes.get() {
            return Err(Error::InvalidInput {
                field: "storage read",
                reason: "page exceeds read memory budget",
            });
        }
        Ok(())
    }

    async fn reserve_scratch(&self, bytes: usize) -> Result<MemoryReservation> {
        if bytes > self.inner.settings.queue_capacity_bytes.get() {
            return Err(Error::InvalidInput {
                field: "storage schema",
                reason: "validation scratch exceeds write memory budget",
            });
        }
        let mut changed = self.inner.changed.subscribe();
        loop {
            {
                let mut state = self.inner.state.lock().unwrap();
                if state.status.closing {
                    return Err(Error::Closing { domain: "storage" });
                }
                if !state.ready || !state.status.running {
                    return Err(Error::NotReady { domain: "storage" });
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                state.collect();
                if state.status.pending_bytes
                    <= self.inner.settings.queue_capacity_bytes.get() - bytes
                {
                    state.scratch_bytes += bytes;
                    state.collect();
                    return Ok(MemoryReservation {
                        inner: self.inner.clone(),
                        bytes,
                        kind: ReservationKind::Write,
                    });
                }
                self.inner.relieve_pressure(&mut state, bytes, 0);
            }
            changed.changed().await.map_err(|_| Error::NotReady {
                domain: "storage worker",
            })?;
        }
    }

    pub(super) async fn reserve_read(
        &self,
        bytes: usize,
        entries: usize,
    ) -> Result<MemoryReservation> {
        self.reserve_read_now(bytes, entries)
    }

    pub(super) fn reserve_read_now(
        &self,
        bytes: usize,
        entries: usize,
    ) -> Result<MemoryReservation> {
        let bytes = entries
            .checked_mul(ENTRY_OVERHEAD)
            .and_then(|overhead| bytes.checked_add(overhead))
            .ok_or(Error::InvalidInput {
                field: "storage read",
                reason: "size overflow",
            })?;
        self.check_read_limit(bytes)?;
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if !state.ready || !state.status.running {
            return Err(Error::NotReady { domain: "storage" });
        }
        state.collect();
        if state.status.read_bytes > self.inner.settings.read_memory_bytes.get() - bytes {
            return Err(Error::ResourceExhausted {
                resource: "storage read memory",
            });
        }
        state.read_bytes += bytes;
        state.collect();
        Ok(MemoryReservation {
            inner: self.inner.clone(),
            bytes,
            kind: ReservationKind::Read,
        })
    }

    pub(super) async fn admit(
        &self,
        domain: Domain,
        bytes: usize,
        count: usize,
        build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        self.admit_mutation(domain, bytes, count, None, build).await
    }

    pub(super) async fn admit_mutation(
        &self,
        domain: Domain,
        bytes: usize,
        count: usize,
        mutation: Option<Mutation<'_>>,
        build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        if mutation.is_some() && !self.inner.primary.supports_mutation_receipts() {
            return Err(Error::InvalidInput {
                field: "storage mutation",
                reason: "backend does not support mutation receipts",
            });
        }
        let bytes = bytes.max(1);
        let mut changed = self.inner.changed.subscribe();
        let (sequence, last, index) = loop {
            let replica_ceiling = self.inner.replica_ceiling();
            {
                let mut state = self.inner.state.lock().unwrap();
                if state.status.closing {
                    return Err(Error::Closing { domain: "storage" });
                }
                if !state.ready || !state.status.running {
                    return Err(Error::NotReady { domain: "storage" });
                }
                if let Some(mutation) = mutation {
                    match state.receipts.lookup(mutation.id, &mutation.fingerprint)? {
                        ReceiptState::Complete(value) => return Ok(value.cursor()?.sequence),
                        ReceiptState::Pending => {
                            return Err(Error::InvalidInput {
                                field: "storage mutation",
                                reason: "mutation already owns different work",
                            });
                        }
                        ReceiptState::Missing => {}
                    }
                }
                if bytes > self.inner.settings.queue_capacity_bytes.get() {
                    return Err(Error::InvalidInput {
                        field: "storage admission",
                        reason: "batch exceeds memory budget",
                    });
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                state.collect();
                if state.reserved_batches == 0
                    && state.finishing.is_none()
                    && state.status.pending_bytes
                        <= self.inner.settings.queue_capacity_bytes.get() - bytes
                    && state.status.pending_batches < self.inner.settings.queue_capacity.get()
                {
                    let sequence = match domain {
                        Domain::Data => state.status.accepted.sequence,
                        Domain::Recovery => state.status.recovery_accepted.sequence,
                    };
                    let last = sequence
                        .checked_add(count as u64)
                        .ok_or(Error::InvalidInput {
                            field: "storage admission",
                            reason: "position overflow",
                        })?;
                    if domain == Domain::Data && last > replica_ceiling {
                        return Err(Error::ResourceExhausted {
                            resource: "replica retained history",
                        });
                    }
                    let index = state.next.checked_add(1).ok_or(Error::InvalidInput {
                        field: "storage admission",
                        reason: "queue order overflow",
                    })?;
                    state.reserved = bytes;
                    state.reserved_batches = 1;
                    state.collect();
                    break (sequence, last, index);
                }
                self.inner.relieve_pressure(&mut state, bytes, 1);
            }
            changed
                .changed()
                .await
                .map_err(|_| Error::NotReady { domain: "storage" })?;
        };
        // The original exclusive reservation owns capacity and capture order.
        // Producer code runs without the state mutex; unwind releases the permit.
        let mut admission = Admission {
            inner: self.inner.clone(),
            active: true,
        };
        let operation = build(if count == 0 { sequence } else { sequence + 1 })?;
        operation.check_entry_bytes(self.inner.settings.max_batch_bytes)?;
        if let Operation::Snapshot(snapshot) = &operation
            && snapshot.through.sequence > sequence
        {
            return Err(Error::InvalidInput {
                field: "recovery snapshot",
                reason: "snapshot is ahead of admitted transitions",
            });
        }
        let bytes = match &operation {
            Operation::ObjectGroup {
                bytes: retained, ..
            } => *retained,
            _ => bytes,
        };
        let pending = Arc::new(Pending {
            journal: None,
            index,
            bytes,
            operation,
        });
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if let Some(source) = &state.status.failure {
                return Err(Error::Persistence {
                    source: source.clone(),
                });
            }
            if let Some(mutation) = mutation {
                let outcome = MutationValue::Cursor(self.position(last));
                state.receipts.insert(
                    mutation.id,
                    mutation.fingerprint,
                    crate::MemorySize::memory_size(&outcome),
                    &self.inner.settings,
                )?;
                state.receipts.complete(mutation.id, outcome);
            }
            let target = Target {
                index,
                sequence: last,
            };
            match domain {
                Domain::Data => {
                    state.status.accepted.sequence = last;
                    state.data_accepted = target;
                }
                Domain::Recovery => {
                    state.status.recovery_accepted.sequence = last;
                    state.recovery_accepted = target;
                }
            }
            state.next = index;
            state.reserved = 0;
            state.reserved_batches = 0;
            state.queue.push_back(pending);
            state.collect();
            admission.active = false;
        }
        self.inner.notify();
        Ok(last)
    }

    async fn flush_domain(&self, domain: Domain, through: Option<&StorageCursor>) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        let target = {
            let mut state = self.inner.state.lock().unwrap();
            if !state.ready {
                return Err(Error::NotReady { domain: "storage" });
            }
            let target = if let Some(through) = through {
                state.data_target(through)?
            } else {
                match domain {
                    Domain::Data => state.data_accepted,
                    Domain::Recovery => state.recovery_accepted,
                }
            };
            // Explicit retry permits another bounded attempt after a failure.
            if state.status.failure.is_some() {
                state.status.failure = None;
                state.retry = state.retry.wrapping_add(1);
            }
            target
        };
        self.inner.notify();
        loop {
            let notify;
            {
                let mut state = self.inner.state.lock().unwrap();
                let flushed = match domain {
                    Domain::Data => state.data_flushed,
                    Domain::Recovery => state.recovery_flushed,
                };
                if flushed >= target.index {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
                let request = match domain {
                    Domain::Data => &mut state.data_request,
                    Domain::Recovery => &mut state.recovery_request,
                };
                notify = request.is_none_or(|current| target.index < current.index);
                if notify {
                    *request = Some(target);
                }
            }
            if notify {
                self.inner.notify();
            }
            changed
                .changed()
                .await
                .map_err(|_| Error::NotReady { domain: "storage" })?;
        }
    }
}

impl State {
    pub(super) fn snapshot_released(&mut self) {
        self.objects_pruned = 0;
        self.objects_prune_owed = true;
    }

    pub(super) fn needs_write(&self, entry: &Pending) -> bool {
        let written = match entry.operation.domain() {
            Domain::Data => self.data_flushed,
            Domain::Recovery => self.recovery_written.index,
        };
        entry.index > written
    }

    pub(super) fn write_boundary(&self) -> Option<u64> {
        [self.data_request, self.recovery_request]
            .into_iter()
            .flatten()
            .map(|target| target.index)
            .min()
    }

    pub(super) fn request_flush(&mut self) -> bool {
        let mut requested = false;
        if self.data_request.is_none() && self.data_accepted.index > self.data_flushed {
            self.data_request = Some(self.data_accepted);
            requested = true;
        }
        if self.recovery_request.is_none() && self.recovery_accepted.index > self.recovery_flushed {
            self.recovery_request = Some(self.recovery_accepted);
            requested = true;
        }
        requested
    }
}

impl Inner {
    pub(super) fn replica_floor(&self) -> u64 {
        self.replicas
            .iter()
            .map(|replica| {
                replica
                    .state
                    .lock()
                    .unwrap()
                    .status
                    .saved
                    .as_ref()
                    .map_or(0, |saved| saved.sequence)
            })
            .min()
            .unwrap_or(u64::MAX)
    }

    pub(super) fn retention_floor(&self) -> u64 {
        self.replica_floor().min(self.receipts.floor())
    }

    pub(super) fn replica_ceiling(&self) -> u64 {
        self.replica_floor()
            .saturating_add(self.settings.replica_lag_positions.get())
    }

    pub(super) fn notify(&self) {
        self.changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    pub(super) fn relieve_pressure(&self, state: &mut State, bytes: usize, batches: usize) {
        let full = state.status.pending_bytes.saturating_add(bytes)
            > self.settings.queue_capacity_bytes.get()
            || state.status.pending_batches.saturating_add(batches)
                > self.settings.queue_capacity.get();
        if full {
            let lagged = state.lag_followers();
            let requested = state.request_flush();
            if lagged || requested {
                self.notify();
            }
        }
    }

    pub(super) async fn bounded<T>(&self, future: StorageFuture<'_, T>) -> Result<T> {
        executor::timeout(self.settings.io_timeout, future)
            .await
            .map_err(|_| {
                Error::backend(
                    "storage I/O",
                    crate::BackendFailure::Transient,
                    io::Error::new(io::ErrorKind::TimedOut, "backend deadline exceeded"),
                )
            })?
    }
}

impl WorkerOwner for Owner {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let receipts: Vec<_> = self
                .leases
                .lock()
                .unwrap()
                .iter_mut()
                .map(Lease::retire)
                .collect();
            let mut failure = None;
            for receipt in receipts {
                if let Err(error) = receipt.await {
                    failure.get_or_insert(error);
                }
            }
            failure.map_or(Ok(()), Err)
        })
    }
}

impl RecordWrite<'_> {
    pub(super) fn resident_bytes(&self, history: &super::StoreHistory) -> Result<usize> {
        [
            self.payload.len(),
            self.schema.id.as_str().len(),
            self.scope.map_or(0, |value| value.as_str().len()),
            self.item.as_str().len(),
            self.source.map_or(0, |value| value.as_str().len()),
            history.store.as_str().len(),
            history.epoch.as_str().len(),
            std::mem::size_of::<StoredRecord>(),
            ENTRY_OVERHEAD,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, next| bytes.checked_add(next))
        .ok_or(Error::InvalidInput {
            field: "storage admission",
            reason: "size overflow",
        })
    }

    pub(super) fn stored(&self, position: StorageCursor) -> StoredRecord {
        StoredRecord {
            position,
            schema: self.schema.clone(),
            scope: self.scope.cloned(),
            item: self.item.clone(),
            source: self.source.cloned(),
            source_time: self.source_time,
            payload: self.payload.to_vec(),
        }
    }
}

impl MemoryReservation {
    pub(super) fn projection(&self, bytes: std::num::NonZeroUsize) -> Result<Self> {
        assert!(matches!(self.kind, ReservationKind::Read));
        let mut output = Self {
            inner: Arc::clone(&self.inner),
            bytes: 0,
            kind: ReservationKind::Read,
        };
        output.reserve_projection(bytes)?;
        Ok(output)
    }

    pub(super) fn reserve_projection(&mut self, bytes: std::num::NonZeroUsize) -> Result<()> {
        assert!(matches!(self.kind, ReservationKind::Read));
        if bytes > self.inner.settings.query_decoded_bytes_ceiling {
            return Err(super::query::decoded_limit());
        }
        let additional = bytes
            .get()
            .checked_add(ENTRY_OVERHEAD)
            .ok_or_else(super::query::decoded_limit)?;
        let retained = self
            .bytes
            .checked_add(additional)
            .ok_or_else(super::query::decoded_limit)?;
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if !state.ready || !state.status.running {
            return Err(Error::NotReady { domain: "storage" });
        }
        let total = state
            .read_bytes
            .checked_add(additional)
            .filter(|bytes| *bytes <= self.inner.settings.read_memory_bytes.get())
            .ok_or(Error::ResourceExhausted {
                resource: "storage read memory",
            })?;
        state.read_bytes = total;
        self.bytes = retained;
        state.collect();
        Ok(())
    }

    pub(super) fn retain_read(&mut self, bytes: usize) {
        assert!(matches!(self.kind, ReservationKind::Read));
        assert!(bytes <= self.bytes);
        let mut state = self.inner.state.lock().unwrap();
        state.read_bytes -= self.bytes - bytes;
        self.bytes = bytes;
        state.collect();
        drop(state);
        self.inner.notify();
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        if self.active {
            {
                let mut state = self.inner.state.lock().unwrap();
                state.reserved = 0;
                state.reserved_batches = 0;
                state.collect();
            }
            self.inner.notify();
        }
    }
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        match self.kind {
            ReservationKind::Read => state.read_bytes -= self.bytes,
            ReservationKind::Write => state.scratch_bytes -= self.bytes,
            ReservationKind::Replication => {
                state.scratch_bytes -= self.bytes;
                state.replication_bytes -= self.bytes;
            }
        }
        state.collect();
        drop(state);
        self.inner.notify();
    }
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("encoded size overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */
pub(super) fn encoded_size<T: Serialize + ?Sized>(value: &T) -> Result<usize> {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|_| Error::InvalidInput {
        field: "storage payload",
        reason: "cannot measure encoded size",
    })?;
    Ok(counter.0)
}

pub(super) fn recovery_preparation_bytes(
    payload: usize,
    history: &super::StoreHistory,
) -> Result<usize> {
    let capacity = payload
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or(Error::InvalidInput {
            field: "managed recovery",
            reason: "size overflow",
        })?;
    recovery_resident_bytes(capacity, history)
}

pub(super) fn recovery_resident_bytes(
    payload: usize,
    history: &super::StoreHistory,
) -> Result<usize> {
    [
        payload,
        history.store.as_str().len(),
        history.epoch.as_str().len(),
        std::mem::size_of::<RecoveryEntry>(),
        ENTRY_OVERHEAD,
    ]
    .into_iter()
    .try_fold(0usize, |bytes, next| bytes.checked_add(next))
    .ok_or(Error::InvalidInput {
        field: "managed recovery",
        reason: "size overflow",
    })
}

/* === Private Functions === */

/* === Tests === */
