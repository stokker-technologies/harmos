/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod accounting;
mod admission;
mod arrow;
mod encoding;
mod lifecycle;
mod recovery;
mod shutdown;
mod state;
mod status;
mod writes;

/* === Re-Exports === */

pub use encoding::encoded_size;
pub(crate) use encoding::{recovery_preparation_bytes, recovery_resident_bytes};

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
use super::arrow::RetainedMemory;
use super::capacity::{Attempt, Held, Pool, Refusal, Reservation};
use super::config::{IO_TIMEOUT, MUTATION_RETENTION};
use super::queue::PendingQueue;
use super::receipts::{MutationValue, ReceiptState};
use super::state::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StateBackend};
use super::worker::Worker;
use super::{
    ReadLimits, RecoveryPosition, SchemaKey, StorageBackend, StorageBackends, StorageBatch, StorageCursor, StorageFuture, StorageSchema,
    StorageSettings, StoredRecord, StoredRow,
};
use crate::execution::Execution;
use crate::{Error, Identifier, Result};
use harmos_domain::model::storage::{RecordInput, RecordWrite, StreamInput};

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
    pub(super) execution: Execution,
    pub(super) retained_write_bytes: Arc<std::sync::atomic::AtomicUsize>,
    pub(super) retained_read_bytes: Arc<std::sync::atomic::AtomicUsize>,
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
    pub(super) object_definition: fn(&SchemaKey) -> harmos_domain::Result<harmos_domain::model::storage::catalog::ObjectDefinition>,
    pub(super) object_retention: fn(&SchemaKey) -> harmos_domain::Result<super::ObjectRetention>,
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

pub(super) struct State {
    pub(super) execution: Execution,
    pub(super) retained_write_bytes: Arc<std::sync::atomic::AtomicUsize>,
    pub(super) retained_read_bytes: Arc<std::sync::atomic::AtomicUsize>,
    prefer_measurement: bool,
    pub(super) journal_required_data: StorageCursor,
    pub(super) reserved: usize,
    pub(super) reserved_batches: usize,
    pub(super) journal_limit: usize,
    pub(super) journal_saved: Option<watch::Sender<harmos_domain::state::Position>>,
    pub(super) status: StorageMetrics,
    pub(super) ready: bool,
    service_owner: Option<Arc<()>>,
    pub(super) next: u64,
    pub(super) queue: PendingQueue,
    pub(super) scratch_bytes: usize,
    pub(super) read_bytes: usize,

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
    pub(super) journal: Option<harmos_domain::state::Position>,
    pub(super) index: u64,
    pub(super) bytes: usize,
    pub(super) operation: Operation,
    charge: Option<Arc<crate::batch::WriteCharge>>,
}

struct Counter(usize);

struct Admission {
    inner: Arc<Inner>,
    active: bool,
}

struct WriteWaiter {
    inner: Arc<Inner>,
    measurement: bool,
}

/* === Enums === */

enum Admitted {
    Replayed(u64),
    Reserved(u64, u64, u64),
}

pub enum ErasedCatalog {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Domain {
    Data,
    Recovery,
}

pub(super) enum Operation {
    Schema(StorageBatch<StorageSchema>),
    File(StorageBatch<super::FileReference>),
    Records(StorageBatch<StoredRecord>),
    ObjectGroup { schemas: StorageBatch<StorageSchema>, objects: StorageBatch<super::ObjectEntry>, bytes: usize },
    Stream(StorageBatch<StoredRow>),
    Recovery(StorageBatch<RecoveryEntry>),
    Snapshot(StorageBatch<RecoverySnapshot>),
}

/* === Traits === */

pub(crate) trait RecordWriteBytes {
    fn resident_bytes(&self, history: &super::StoreHistory) -> Result<usize>;
}

pub trait WorkerOwner: Send + Sync {
    fn stop(&self) -> StorageFuture<'_, ()>;
}

/* === Implementations === */

impl Pending {
    pub(super) fn retire(&self) {
        if let Some(charge) = &self.charge {
            charge.retire();
        }
    }

    pub(super) fn new(
        inner: &Arc<Inner>, journal: Option<harmos_domain::state::Position>, index: u64, bytes: usize, mut operation: Operation,
    ) -> Self {
        let charge = match &mut operation {
            Operation::Schema(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::File(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::Records(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::Stream(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::ObjectGroup { schemas, objects, .. } => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *schemas = schemas.clone().retained(Arc::clone(&charge));
                *objects = objects.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::Recovery(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
            Operation::Snapshot(values) => Some({
                let charge = crate::batch::WriteCharge::new(inner, bytes);
                *values = values.clone().retained(Arc::clone(&charge));
                charge
            }),
        };
        Self { journal, index, bytes, operation, charge }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.retire();
    }
}

impl<C> Clone for StorageManager<C> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), catalog: PhantomData }
    }
}

impl<C> StorageManager<C> {
    pub fn erased(&self) -> StorageManager<ErasedCatalog> {
        StorageManager { inner: self.inner.clone(), catalog: PhantomData }
    }
}

impl<C: super::ObjectCatalog> StorageManager<C> {
    pub fn new(backends: StorageBackends, settings: StorageSettings) -> Result<Self> {
        Self::with_execution(backends, settings, Execution::default())
    }

    pub fn with_execution(backends: StorageBackends, settings: StorageSettings, execution: Execution) -> Result<Self> {
        C::validate()?;
        settings.validate()?;
        let StorageBackends { primary, state: state_backend } = backends;
        primary.history().check(state_backend.history())?;
        let history = primary.history().clone();
        let position = StorageCursor { history: history.clone(), sequence: 0 };
        let recovery_position = RecoveryPosition { history, sequence: 0 };
        let receipts = super::receipts::Receipts::new(MUTATION_RETENTION, settings.mutation_bytes(), execution)?;
        let retained_write_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let retained_read_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        Ok(Self {
            catalog: PhantomData,
            inner: Arc::new(Inner {
                execution,
                retained_write_bytes: Arc::clone(&retained_write_bytes),
                retained_read_bytes: Arc::clone(&retained_read_bytes),
                measurement_admission: tokio::sync::Mutex::new(()),
                control_admission: tokio::sync::Mutex::new(()),
                measurement_waiting: std::sync::atomic::AtomicUsize::new(0),
                control_waiting: std::sync::atomic::AtomicUsize::new(0),
                object_writes: tokio::sync::Mutex::new(()),
                query_active: Arc::new(tokio::sync::Semaphore::new(settings.request.max_active_queries.get())),
                query_pending: Arc::new(tokio::sync::Semaphore::new(settings.request.max_pending_queries.get())),
                settings,
                primary,
                recovery: state_backend,
                has_objects: C::definition_count() != 0,
                object_definition: harmos_domain::model::storage::catalog::definition::<C>,
                object_retention: |key| Ok(harmos_domain::model::storage::catalog::definition::<C>(key)?.retention()),
                state: Mutex::new(State {
                    execution,
                    retained_write_bytes,
                    retained_read_bytes,
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
