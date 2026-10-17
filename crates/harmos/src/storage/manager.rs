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
use harmos_domain::model::storage::{RecordWrite, StreamWrite};
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
use super::arrow::RetainedMemory;
use super::capacity::{Attempt, Held, Pool, Refusal, Reservation};
use super::config::{IO_TIMEOUT, MUTATION_RETENTION};
use super::queue::PendingQueue;
use super::receipts::{MutationValue, ReceiptState};
use super::state::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StateBackend};
use super::worker::Worker;
use super::{
    ReadLimits, RecoveryPosition, SchemaKey, StorageBackend, StorageBackends, StorageCursor,
    StorageFuture, StorageSchema, StorageSettings, StoredRecord, StoredRow,
};
use crate::operations::{OperationManager, resident::Lease};
use crate::services::{Once, ServiceDefinition};
use crate::supervision::executor;
use crate::{Application, Error, Identifier, Result};

/* === Constants === */
const WORKER_NAME: &str = "harmos-storage";

// Includes owned envelopes, queue nodes, Arc headers, and cloned metadata.
pub(super) const ENTRY_OVERHEAD: usize = 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct StorageMetrics {
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
    pub(super) measurement_admission: tokio::sync::Mutex<()>,
    pub(super) control_admission: tokio::sync::Mutex<()>,
    measurement_waiting: std::sync::atomic::AtomicUsize,
    control_waiting: std::sync::atomic::AtomicUsize,
    pub(super) object_writes: tokio::sync::Mutex<()>,
    pub(super) query_active: Arc<tokio::sync::Semaphore>,
    pub(super) query_pending: Arc<tokio::sync::Semaphore>,
    pub(super) settings: StorageSettings,
    pub(super) primary: Arc<dyn StorageBackend>,
    pub(super) recovery: Arc<dyn StateBackend>,
    pub(super) has_objects: bool,
    pub(super) object_definition: fn(
        &SchemaKey,
    ) -> harmos_domain::Result<
        harmos_domain::model::storage::catalog::ObjectDefinition,
    >,
    pub(super) state: Mutex<State>,
    pub(super) changed: watch::Sender<u64>,
    pub(super) lifecycle: tokio::sync::RwLock<Lifecycle>,
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
    prefer_measurement: bool,
    pub(super) journal_required_data: StorageCursor,
    pub(super) reserved: usize,
    pub(super) reserved_batches: usize,
    pub(super) journal_limit: usize,
    pub(super) journal_saved: Option<watch::Sender<crate::state::Position>>,
    pub(super) status: StorageMetrics,
    pub(super) ready: bool,
    service_owner: Option<Arc<()>>,
    pub(super) next: u64,
    pub(super) queue: PendingQueue,
    pub(super) scratch_bytes: usize,
    pub(super) read_bytes: usize,
    /// Per pool: owners that hold bytes and are waiting to grow.
    pub(super) growers: [super::capacity::Growers; 2],
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
    pub(super) snapshots: BTreeMap<u64, super::snapshot::SnapshotState>,
    pub(super) followers: BTreeMap<u64, super::follow::Following>,
    pub(super) files: BTreeMap<u64, super::staging::FileState>,
    pub(super) receipts: super::receipts::Receipts,
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

enum Admitted {
    Replayed(u64),
    Reserved(u64, u64, u64),
}

struct WriteWaiter {
    inner: Arc<Inner>,
    measurement: bool,
}

/* === Enums === */
pub enum ErasedCatalog {}

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
pub(crate) trait RecordWriteBytes {
    fn resident_bytes(&self, history: &super::StoreHistory) -> Result<usize>;
}
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
    pub fn new(backends: StorageBackends, settings: StorageSettings) -> Result<Self> {
        C::validate()?;
        settings.validate()?;
        let StorageBackends {
            primary,
            state: state_backend,
        } = backends;
        primary.history().check(state_backend.history())?;
        let history = primary.history().clone();
        let position = StorageCursor {
            history: history.clone(),
            sequence: 0,
        };
        let recovery_position = RecoveryPosition {
            history,
            sequence: 0,
        };
        let receipts =
            super::receipts::Receipts::new(MUTATION_RETENTION, settings.mutation_bytes())?;
        Ok(Self {
            catalog: PhantomData,
            inner: Arc::new(Inner {
                measurement_admission: tokio::sync::Mutex::new(()),
                control_admission: tokio::sync::Mutex::new(()),
                measurement_waiting: std::sync::atomic::AtomicUsize::new(0),
                control_waiting: std::sync::atomic::AtomicUsize::new(0),
                object_writes: tokio::sync::Mutex::new(()),
                query_active: Arc::new(tokio::sync::Semaphore::new(
                    settings.request.max_active_queries.get(),
                )),
                query_pending: Arc::new(tokio::sync::Semaphore::new(
                    settings.request.max_pending_queries.get(),
                )),
                settings,
                primary,
                recovery: state_backend,
                has_objects: C::definition_count() != 0,
                object_definition: harmos_domain::model::storage::catalog::definition::<C>,
                state: Mutex::new(State {
                    prefer_measurement: false,
                    journal_required_data: position.clone(),
                    reserved: 0,
                    reserved_batches: 0,
                    journal_limit: 0,
                    journal_saved: None,
                    status: StorageMetrics {
                        accepted: position.clone(),
                        saved: position,
                        recovery_accepted: recovery_position.clone(),
                        recovery_saved: recovery_position,
                        pending_bytes: 0,
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
                    growers: Default::default(),
                    read_bytes: 0,
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
                    snapshots: BTreeMap::new(),
                    followers: BTreeMap::new(),
                    files: BTreeMap::new(),
                    deletions: BTreeMap::new(),
                    receipts,
                    file_cursor: 0,
                    finishing: None,
                }),
                changed: watch::channel(0).0,
                lifecycle: tokio::sync::RwLock::new(Lifecycle::default()),
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
        let mut lifecycle = self.inner.lifecycle.write().await;
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
        let owner = Owner {
            leases: Mutex::new(vec![primary]),
        };
        lifecycle.owner = Some(Box::new(owner));
        lifecycle.startup_complete = true;
        self.inner.notify();
        Ok(())
    }

    pub(crate) async fn acquire(&self) -> Result<()> {
        let mut lifecycle = self.inner.lifecycle.write().await;
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

    pub fn status(&self) -> StorageMetrics {
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
            .validation_memory_bytes(&harmos_domain::config::SchemaSettings::default())
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
            .prepare(&harmos_domain::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput {
                field: "storage schema",
                reason: "invalid message fields",
            })?;
        drop(scratch);
        let bytes = StorageSchema::resident_bytes(key, schema, self.inner.primary.history())?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, 1, mutation, false, |sequence| {
                Ok(Operation::Schema(StorageSchema::new(
                    self.position(sequence),
                    key.clone(),
                    schema.clone(),
                )))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub fn record_batch_len(&self, records: &[RecordWrite<'_>]) -> Result<usize> {
        self.batch_prefix(records.iter().map(|record| {
            let value = StoredRecord {
                position: self.position(u64::MAX),
                schema: record.schema.clone(),
                scope: record.scope.cloned(),
                item: record.item.clone(),
                source: record.source.cloned(),
                source_time: record.source_time,
                payload: record.payload,
            };
            Ok((
                encoded_size(&value)?,
                record.resident_bytes(self.inner.primary.history())?,
            ))
        }))
    }

    pub fn stream_batch_len(&self, rows: &[StreamWrite<'_>]) -> Result<usize> {
        self.batch_prefix(rows.iter().map(|row| {
            let value = StoredRow {
                record: StoredRecord {
                    position: self.position(u64::MAX),
                    schema: row.record.schema.clone(),
                    scope: row.record.scope.cloned(),
                    item: row.record.item.clone(),
                    source: row.record.source.cloned(),
                    source_time: row.record.source_time,
                    payload: row.record.payload,
                },
                stream: row.stream.clone(),
                sequence: row.sequence,
            };
            let resident = row
                .record
                .resident_bytes(self.inner.primary.history())?
                .checked_add(std::mem::size_of::<StoredRow>() - std::mem::size_of::<StoredRecord>())
                .and_then(|bytes| bytes.checked_add(row.stream.as_str().len()))
                .ok_or(Error::ResourceExhausted {
                    resource: "stream batch memory",
                })?;
            Ok((encoded_size(&value)?, resident))
        }))
    }

    fn batch_prefix(&self, sizes: impl Iterator<Item = Result<(usize, usize)>>) -> Result<usize> {
        let mut bytes = 0usize;
        let mut rows = 0usize;
        let mut resident = 0usize;
        let memory = self.inner.settings.request.max_pending_write_bytes.get();
        let maximum = self.inner.settings.max_batch_bytes().get();
        for size in sizes {
            let (size, held) = size?;
            if size > maximum || held > memory {
                return if rows == 0 {
                    Err(Error::ResourceExhausted {
                        resource: "oversized atomic write row",
                    })
                } else {
                    Ok(rows)
                };
            }
            if size > maximum - bytes || held > memory - resident {
                break;
            }
            bytes += size;
            resident += held;
            rows += 1;
        }
        Ok(rows)
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
            .admit_mutation(
                Domain::Data,
                bytes,
                records.len(),
                mutation,
                false,
                |first| {
                    Ok(Operation::Records(
                        records
                            .enumerate()
                            .map(|(index, record)| {
                                record.stored(self.position(first + index as u64))
                            })
                            .collect(),
                    ))
                },
            )
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
            .admit_mutation(Domain::Data, bytes, rows.len(), mutation, true, |first| {
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
        let required_data = self.cursor_saved();
        let bytes = payloads.iter().try_fold(0usize, |bytes, payload| {
            bytes
                .checked_add(recovery_resident_bytes(
                    payload.len(),
                    self.inner.recovery.history(),
                    self.inner.primary.history(),
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
                            required_data: required_data.clone(),
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
        self.check_state_dependency(&snapshot.required_data)?;
        let bytes = recovery_resident_bytes(
            snapshot.payload.len(),
            &snapshot.through.history,
            &snapshot.required_data.history,
        )?;
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
            let _lifecycle = self.inner.lifecycle.write().await;
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
        let _lifecycle = self.inner.lifecycle.read().await;
        let charge = limits
            .rows
            .get()
            .saturating_mul(ENTRY_OVERHEAD)
            .saturating_add(limits.bytes.get());
        let _reservation =
            Reservation::wait(&self.inner, Pool::Read, &Held::default(), charge, None)
                .await
                .map_err(Refusal::into_error)?;
        let status = self.status();
        if !status.recovery_saved.covers(through)? {
            return Err(Error::InvalidInput {
                field: "recovery range",
                reason: "frontier is not durable",
            });
        }
        let page = self
            .inner
            .bounded(self.inner.recovery.read_entries(after, through, limits))
            .await?;
        page.check(after, through, limits, |entry| {
            encoded_size(entry).unwrap_or(usize::MAX)
        })?;
        for entry in &page.entries {
            self.check_state_dependency(&entry.required_data)?;
        }
        Ok(page)
    }

    pub async fn read_snapshot(
        &self,
        max_bytes: std::num::NonZeroUsize,
    ) -> Result<Option<RecoverySnapshot>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        let charge = max_bytes.get().saturating_add(ENTRY_OVERHEAD);
        let _reservation =
            Reservation::wait(&self.inner, Pool::Read, &Held::default(), charge, None)
                .await
                .map_err(Refusal::into_error)?;
        let snapshot = self
            .inner
            .bounded(self.inner.recovery.read_snapshot(max_bytes))
            .await?;
        if let Some(snapshot) = &snapshot
            && (!self.status().recovery_saved.covers(&snapshot.through)?
                || !self.status().saved.covers(&snapshot.required_data)?
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
        {
            let mut state = self.inner.state.lock().unwrap();
            state.status.closing = true;
            state.close_followers();
        }
        self.inner.notify();

        let mut lifecycle = self.inner.lifecycle.write().await;
        let ready = {
            let state = self.inner.state.lock().unwrap();
            state.ready && !state.status.closed
        };
        if ready {
            self.settle_transition().await;
            self.settle_files().await?;
            self.settle_deletions().await?;
            let files = self.capture_files(None)?;
            self.flush_domain(Domain::Data, None).await?;
            self.persist_files(&files).await?;
            self.retire_files();
            self.flush_recovery().await?;
        }
        if ready {
            self.release_snapshots().await?;
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

    async fn reserve_scratch(&self, bytes: usize) -> Result<Reservation> {
        Reservation::wait(&self.inner, Pool::Write, &Held::default(), bytes, None)
            .await
            .map_err(Refusal::into_error)
    }

    /// The schema registered under `key` in the current saved image.
    pub(crate) async fn lookup_schema(
        &self,
        key: &SchemaKey,
    ) -> Result<(StorageSchema, RetainedMemory)> {
        let held = Held::default();
        let snapshot = self.snapshot(&self.cursor_saved()).await?;
        super::query::schema_at(&self.inner, &held, &snapshot, key).await
    }

    pub(crate) async fn admit_arrow_records(
        &self,
        scope: Option<&Identifier>,
        batch: &super::writes::DecodedBatch,
    ) -> Result<StorageCursor> {
        let scratch = self.reserve_arrow_write(batch).await?;
        let (schema, _schema_memory) = self.lookup_schema(&batch.schema).await?;
        let mut records = super::writes::convert::records(scope, batch, &schema.schema)?;
        for record in &mut records {
            if record
                .source
                .as_ref()
                .is_some_and(|source| source != &batch.destination)
            {
                return Err(Error::InvalidInput {
                    field: "record destination",
                    reason: "record source differs from authorized destination",
                });
            }
            record.source = Some(batch.destination.clone());
        }
        let result = self
            .admit_records(records.iter().map(super::RecordInput::borrowed), None)
            .await;
        drop(scratch);
        result
    }

    pub(crate) async fn admit_arrow_streams(
        &self,
        scope: Option<&Identifier>,
        batch: &super::writes::DecodedBatch,
    ) -> Result<StorageCursor> {
        let scratch = self.reserve_arrow_write(batch).await?;
        let (schema, _schema_memory) = self.lookup_schema(&batch.schema).await?;
        let rows = super::writes::convert::streams(scope, batch, &schema.schema)?;
        let result = self
            .admit_stream(rows.iter().map(super::StreamInput::borrowed), None)
            .await;
        drop(scratch);
        result
    }

    pub(crate) async fn admit_arrow_objects(
        &self,
        _scope: Option<&Identifier>,
        batch: &super::writes::DecodedBatch,
    ) -> Result<StorageCursor> {
        use crate::MemorySize;
        if batch.destination != batch.schema.id {
            return Err(Error::InvalidInput {
                field: "object destination",
                reason: "destination differs from object schema",
            });
        }
        let scratch = self.reserve_arrow_write(batch).await?;
        let _writer = self.inner.object_writes.lock().await;
        let definition = (self.inner.object_definition)(&batch.schema)?;
        let graph = definition.graph(&harmos_domain::config::SchemaSettings::default())?;
        let values =
            super::writes::convert::objects(batch, &graph, self.inner.settings.max_batch_bytes())?;
        let count = std::num::NonZeroUsize::new(values.len()).ok_or(Error::InvalidInput {
            field: "object batch",
            reason: "empty atomic batch",
        })?;
        let metadata = graph
            .memory_size()
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD * 8))
            .and_then(|bytes| bytes.checked_mul(values.len()))
            .and_then(|bytes| {
                values
                    .memory_size()
                    .checked_mul(4)
                    .and_then(|payload| bytes.checked_add(payload))
            })
            .ok_or(Error::ResourceExhausted {
                resource: "object atomic batch",
            })?;
        let allowance = std::num::NonZeroUsize::new(metadata).ok_or(Error::ResourceExhausted {
            resource: "object atomic batch",
        })?;
        if scratch
            .bytes()
            .checked_add(allowance.get())
            .is_none_or(|bytes| bytes > self.inner.settings.request.max_pending_write_bytes.get())
            || values.memory_size() > allowance.get()
        {
            return Err(Error::ResourceExhausted {
                resource: "object atomic batch",
            });
        }
        self.check_object_preconditions(batch, &values).await?;
        let receipts = self
            .admit_object_group_locked(allowance, count, None, |group| {
                for value in &values {
                    group.write_value(value)?;
                }
                Ok(())
            })
            .await?;
        drop(scratch);
        receipts
            .last()
            .map(|receipt| receipt.position.clone())
            .ok_or(Error::InvalidInput {
                field: "object batch",
                reason: "accepted batch has no receipt",
            })
    }

    async fn check_object_preconditions(
        &self,
        batch: &super::writes::DecodedBatch,
        values: &[super::ObjectValue],
    ) -> Result<()> {
        use arrow_array::Array;
        let Some(metadata) = batch
            .data
            .as_record_batch()
            .column_by_name("_storage")
            .and_then(|array| array.as_any().downcast_ref::<arrow_array::StructArray>())
        else {
            return Err(Error::InvalidInput {
                field: "object batch",
                reason: "missing object metadata",
            });
        };
        let Some(expected) = metadata.column_by_name("expected_revision") else {
            return Ok(());
        };
        let expected = expected
            .as_any()
            .downcast_ref::<arrow_array::UInt64Array>()
            .ok_or(Error::InvalidInput {
                field: "object revision",
                reason: "expected_revision must be nullable UInt64",
            })?;
        let snapshot = self.snapshot(&self.cursor_saved()).await?;
        let held = Held::default();
        let page_bytes = self.inner.settings.scan_bytes();
        let store = metadata
            .column_by_name("store")
            .and_then(|array| array.as_any().downcast_ref::<arrow_array::StringArray>());
        let epoch = metadata
            .column_by_name("epoch")
            .and_then(|array| array.as_any().downcast_ref::<arrow_array::StringArray>());
        for (row, value) in values.iter().enumerate() {
            if expected.is_null(row) {
                continue;
            }
            if expected.value(row) != 0
                && (store.is_none_or(|store| {
                    store.is_null(row)
                        || store.value(row) != self.inner.primary.history().store.as_str()
                }) || epoch.is_none_or(|epoch| {
                    epoch.is_null(row)
                        || epoch.value(row) != self.inner.primary.history().epoch.as_str()
                }))
            {
                return Err(Error::InvalidInput {
                    field: "object revision",
                    reason: "expected revision belongs to a different storage history",
                });
            }
            let selection = super::query::ObjectSelection {
                schema: Some(value.schema.clone()),
                key: Some(value.item.clone()),
                versions: super::query::ObjectVersions::Latest,
            };
            let page = super::ScanPage {
                after: 0,
                rows: std::num::NonZeroUsize::MIN,
                bytes: page_bytes,
            };
            let _page = Reservation::wait(
                &self.inner,
                Pool::Read,
                &held,
                page_bytes.get().saturating_add(ENTRY_OVERHEAD),
                None,
            )
            .await
            .map_err(Refusal::into_error)?;
            let (latest, _) = snapshot
                .scan::<super::ObjectEntry>(&selection, page)
                .await?;
            let revision = latest
                .first()
                .map_or(0, |entry| entry.version.revision().get());
            if revision != expected.value(row) {
                return Err(Error::InvalidInput {
                    field: "object revision",
                    reason: "atomic batch revision precondition failed",
                });
            }
        }
        Ok(())
    }

    async fn reserve_arrow_write(
        &self,
        batch: &super::writes::DecodedBatch,
    ) -> Result<Reservation> {
        use crate::MemorySize;
        if batch.data.as_record_batch().num_rows() == 0 {
            return Err(Error::InvalidInput {
                field: "storage write",
                reason: "empty atomic batch",
            });
        }
        let bytes = batch
            .memory_size()
            .checked_mul(8)
            .ok_or(Error::ResourceExhausted {
                resource: "storage write conversion",
            })?;
        // The original arrays, conversion scratch and admitted native copy coexist.
        // Refuse an impossible working set before allocating or waiting on ourselves.
        if bytes
            .checked_mul(2)
            .is_none_or(|bytes| bytes > self.inner.settings.request.max_pending_write_bytes.get())
        {
            return Err(Error::ResourceExhausted {
                resource: "storage write conversion",
            });
        }
        self.reserve_conversion(bytes).await
    }

    // Conversion Scratch Draws On At Most Half The Write Budget So The Admitted
    // Native Copy Always Has Coexistence Space.
    async fn reserve_conversion(&self, bytes: usize) -> Result<Reservation> {
        Reservation::wait(&self.inner, Pool::Write, &Held::default(), bytes, None)
            .await
            .map_err(Refusal::into_error)
    }

    pub(crate) fn check_state_dependency(&self, required: &StorageCursor) -> Result<()> {
        if !self.cursor_saved().covers(required)? {
            return Err(Error::InvalidInput {
                field: "state evidence dependency",
                reason: "required primary evidence is not durable",
            });
        }
        Ok(())
    }

    pub fn retry_transient_persistence(&self) -> bool {
        let mut state = self.inner.state.lock().unwrap();
        let Some(failure) = &state.status.failure else {
            return false;
        };
        let mut cause = failure.as_ref();
        while let Error::Persistence { source } = cause {
            cause = source.as_ref();
        }
        if !matches!(
            cause,
            Error::Backend {
                kind: crate::BackendFailure::Transient,
                ..
            }
        ) {
            return false;
        }
        state.status.failure = None;
        state.retry = state.retry.wrapping_add(1);
        drop(state);
        self.inner.notify();
        true
    }

    /// Charge consumer-retained Arrow data to the shared read budget without waiting.
    pub fn reserve_arrow_memory(&self, bytes: usize) -> Result<RetainedMemory> {
        Reservation::try_new(&self.inner, Pool::Read, &Held::default(), bytes)
            .map(Reservation::retain)
            .map_err(Refusal::into_error)
    }

    pub(crate) fn write_receipt_capacity(&self) -> usize {
        self.inner.settings.request.max_pending_write_batches.get()
    }

    pub async fn await_durable(&self, cursor: &StorageCursor) -> Result<()> {
        self.inner.primary.history().check(&cursor.history)?;
        self.flush_domain(Domain::Data, Some(cursor)).await
    }

    pub async fn release_object_history(
        &self,
        schema: &SchemaKey,
        item: &super::ObjectKey,
        through: &StorageCursor,
    ) -> Result<()> {
        self.inner.primary.history().check(&through.history)?;
        if !self.status().saved.covers(through)? {
            return Err(Error::InvalidInput {
                field: "object retention",
                reason: "release exceeds durable frontier",
            });
        }
        self.inner
            .bounded(
                self.inner
                    .primary
                    .release_object_history(schema, item, through),
            )
            .await?;
        self.inner.state.lock().unwrap().objects_pruned = 0;
        self.inner.notify();
        Ok(())
    }

    pub(super) async fn admit(
        &self,
        domain: Domain,
        bytes: usize,
        count: usize,
        build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        self.admit_mutation(domain, bytes, count, None, false, build)
            .await
    }

    pub(super) async fn admit_mutation(
        &self,
        domain: Domain,
        bytes: usize,
        count: usize,
        mutation: Option<Mutation<'_>>,
        measurement: bool,
        build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        if mutation.is_some() && !self.inner.primary.supports_mutation_receipts() {
            return Err(Error::InvalidInput {
                field: "storage mutation",
                reason: "backend does not support mutation receipts",
            });
        }
        // FIFO Within Each Class; Alternate Classes When Both Are Waiting.
        let _class = if measurement {
            self.inner.measurement_admission.lock().await
        } else {
            self.inner.control_admission.lock().await
        };
        let counter = if measurement {
            &self.inner.measurement_waiting
        } else {
            &self.inner.control_waiting
        };
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _waiting = WriteWaiter {
            inner: self.inner.clone(),
            measurement,
        };
        let bytes = bytes.max(1);
        let byte_capacity = self.inner.settings.request.max_pending_write_bytes.get();
        let batch_capacity = self.inner.settings.request.max_pending_write_batches.get();
        let admitted = self
            .inner
            .wait(None, |state| {
                if let Err(error) = super::capacity::admitting(state, Pool::Write) {
                    return Attempt::Refuse(Refusal::Failed(error));
                }
                if let Some(mutation) = mutation {
                    match state.receipts.lookup(mutation.id, &mutation.fingerprint) {
                        Ok(ReceiptState::Complete(value)) => {
                            return match value.cursor() {
                                Ok(cursor) => Attempt::Ready(Admitted::Replayed(cursor.sequence)),
                                Err(error) => Attempt::Refuse(Refusal::Failed(error)),
                            };
                        }
                        Ok(ReceiptState::Pending) => {
                            return Attempt::Refuse(Refusal::Failed(Error::InvalidInput {
                                field: "storage mutation",
                                reason: "mutation already owns different work",
                            }));
                        }
                        Ok(ReceiptState::Missing) => {}
                        Err(error) => return Attempt::Refuse(Refusal::Failed(error)),
                    }
                }
                if bytes > byte_capacity {
                    return Attempt::Refuse(Refusal::Exceeded(Pool::Write));
                }
                state.collect();

                // FIFO Within Each Class; Alternate Classes When Both Are Waiting.
                let opposite = if measurement {
                    &self.inner.control_waiting
                } else {
                    &self.inner.measurement_waiting
                };
                let turn = state.prefer_measurement == measurement
                    || opposite.load(std::sync::atomic::Ordering::SeqCst) == 0;
                if !(turn
                    && state.reserved_batches == 0
                    && state.finishing.is_none()
                    && state.status.pending_bytes <= byte_capacity - bytes
                    && state.status.pending_batches < batch_capacity)
                {
                    self.inner.relieve_pressure(state, bytes, 1);
                    return Attempt::Wait;
                }
                let sequence = match domain {
                    Domain::Data => state.status.accepted.sequence,
                    Domain::Recovery => state.status.recovery_accepted.sequence,
                };
                let (Some(last), Some(index)) = (
                    sequence.checked_add(count as u64),
                    state.next.checked_add(1),
                ) else {
                    return Attempt::Refuse(Refusal::Failed(Error::InvalidInput {
                        field: "storage admission",
                        reason: "position overflow",
                    }));
                };
                state.reserved = bytes;
                state.reserved_batches = 1;
                state.collect();
                Attempt::Ready(Admitted::Reserved(sequence, last, index))
            })
            .await
            .map_err(Refusal::into_error)?;
        let (sequence, last, index) = match admitted {
            Admitted::Replayed(sequence) => return Ok(sequence),
            Admitted::Reserved(sequence, last, index) => (sequence, last, index),
        };
        // The original exclusive reservation owns capacity and capture order.
        // Producer code runs without the state mutex; unwind releases the permit.
        let mut admission = Admission {
            inner: self.inner.clone(),
            active: true,
        };
        let operation = build(if count == 0 { sequence } else { sequence + 1 })?;
        operation.check_entry_bytes(self.inner.settings.max_batch_bytes())?;
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
            state.prefer_measurement = !measurement;
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
    pub(super) fn data_target(&self, through: &StorageCursor) -> Result<Target> {
        if !self.status.accepted.covers(through)? {
            return Err(Error::InvalidInput {
                field: "storage cursor",
                reason: "cursor is ahead of admission",
            });
        }
        if self.status.saved.covers(through)? {
            return Ok(Target {
                index: self.data_flushed,
                sequence: self.status.saved.sequence,
            });
        }
        let entry = self
            .queue
            .iter()
            .find(|entry| {
                entry.operation.domain() == Domain::Data
                    && entry.operation.sequence() >= through.sequence
            })
            .ok_or(Error::InvalidInput {
                field: "storage cursor",
                reason: "admitted cursor has no pending owner",
            })?;
        Ok(Target {
            index: entry.index,
            sequence: entry.operation.sequence(),
        })
    }

    /// Recompute the published pending and read totals from their owners.
    pub(super) fn collect(&mut self) {
        let now = executor::now();
        self.receipts.purge(now);
        self.status.mutation_bytes = self.receipts.snapshot_bytes();
        self.status.pending_batches = self.reserved_batches
            + self.queue.len()
            + self
                .files
                .values()
                .filter(|file| file.pending_batch())
                .count();
        self.status.pending_bytes = self.reserved
            + self.scratch_bytes
            + self.queue.bytes()
            + self.files.values().map(|file| file.bytes()).sum::<usize>();
        self.status.read_bytes = self.read_bytes
            + self
                .deletions
                .values()
                .map(|deletion| deletion.bytes)
                .sum::<usize>();
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
    pub(super) fn notify(&self) {
        self.changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    pub(super) fn relieve_pressure(&self, state: &mut State, bytes: usize, batches: usize) {
        let full = state.status.pending_bytes.saturating_add(bytes)
            > self.settings.request.max_pending_write_bytes.get()
            || state.status.pending_batches.saturating_add(batches)
                > self.settings.request.max_pending_write_batches.get();
        if full {
            let lagged = state.lag_followers();
            let requested = state.request_flush();
            if lagged || requested {
                self.notify();
            }
        }
    }

    pub(super) async fn bounded<T>(&self, future: StorageFuture<'_, T>) -> Result<T> {
        executor::timeout(IO_TIMEOUT, future).await.map_err(|_| {
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

impl RecordWriteBytes for RecordWrite<'_> {
    fn resident_bytes(&self, history: &super::StoreHistory) -> Result<usize> {
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
}

impl Drop for WriteWaiter {
    fn drop(&mut self) {
        let counter = if self.measurement {
            &self.inner.measurement_waiting
        } else {
            &self.inner.control_waiting
        };
        counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
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
    data_history: &super::StoreHistory,
) -> Result<usize> {
    let capacity = payload
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or(Error::InvalidInput {
            field: "managed recovery",
            reason: "size overflow",
        })?;
    recovery_resident_bytes(capacity, history, data_history)
}

pub(super) fn recovery_resident_bytes(
    payload: usize,
    history: &super::StoreHistory,
    data_history: &super::StoreHistory,
) -> Result<usize> {
    [
        payload,
        history.store.as_str().len(),
        history.epoch.as_str().len(),
        data_history.store.as_str().len(),
        data_history.epoch.as_str().len(),
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
