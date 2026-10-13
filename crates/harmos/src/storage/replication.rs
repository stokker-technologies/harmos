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
use std::collections::BTreeSet;
use std::fmt;
use std::future::{Future, poll_fn};
use std::io;
use std::sync::{Arc, Mutex};
use std::task::Poll;
// Third Party Imports

// Local Imports
use super::manager::{ENTRY_OVERHEAD, MemoryReservation, ReservationKind, encoded_size};
use super::*;
use crate::supervision::executor;
use crate::{
    Application, BackendFailure, Error, Identifier, Result, services::Fault, services::Resident,
    services::Service,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicaHolder {
    pub owner: String,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicaReceipt {
    pub replica: Identifier,
    pub holder: ReplicaHolder,
    pub cursor: StorageCursor,
}

#[derive(Default)]
pub(super) struct Receipts {
    entries: Mutex<std::collections::BTreeMap<Identifier, ReplicaReceipt>>,
}

pub struct ReplicaTarget<C: ObjectCatalog = ()> {
    pub id: Identifier,
    pub backend: Arc<dyn StorageBackend<Catalog = C>>,
}

#[derive(Clone, Debug)]
pub struct ReplicaStatus {
    pub id: Identifier,
    pub saved: Option<StorageCursor>,
    pub running: bool,
    pub closed: bool,
    pub failure: Option<Arc<Error>>,
}

pub(super) struct Replica {
    pub(super) config: ErasedReplica,
    pub(super) state: Mutex<State>,
    base_bytes: usize,
}

pub(super) struct State {
    pub(super) status: ReplicaStatus,
    pub(super) touched: bool,
    pub(super) snapshot: Option<BackendSnapshot>,
    pub(super) bootstrap: Option<BootstrapId>,
    pub(super) staging: Option<FileStaging>,
    pub(super) extra_staging: Option<FileStaging>,
    pub(super) stopping: bool,
    retry: u64,
    objects_pruned: u64,
}

#[derive(Clone)]
pub(super) struct FileStaging {
    pub(super) file: StagedFile,
    pub(super) bootstrap: Option<BootstrapId>,
}

pub(super) struct Worker {
    pub(super) manager: StorageManager<super::manager::ErasedCatalog>,
    pub(super) replica: Arc<Replica>,
    pub(super) reservation: Option<MemoryReservation>,
}

struct Running {
    manager: StorageManager<super::manager::ErasedCatalog>,
    replica: Arc<Replica>,
}

pub(super) struct ErasedReplica {
    pub(super) id: Identifier,
    pub(super) backend: Arc<dyn super::backend::ErasedBackend>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Receipts {
    pub(super) fn floor(&self) -> u64 {
        self.entries
            .lock()
            .unwrap()
            .values()
            .map(|receipt| receipt.cursor.sequence)
            .min()
            .unwrap_or(u64::MAX)
    }
}

impl<C: ObjectCatalog> Clone for ReplicaTarget<C> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            backend: self.backend.clone(),
        }
    }
}

impl<C: ObjectCatalog> fmt::Debug for ReplicaTarget<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplicaTarget")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Replica {
    pub(super) fn started(&self) {
        self.state.lock().unwrap().status.running = true;
    }

    pub(super) fn check_copying(&self) -> Result<()> {
        if self.state.lock().unwrap().stopping {
            return Err(Error::Closing { domain: "replica" });
        }
        Ok(())
    }

    pub(super) fn retry(&self) {
        let mut state = self.state.lock().unwrap();
        if state.status.failure.take().is_some() {
            state.retry = state.retry.wrapping_add(1);
        }
    }
}

impl<C> StorageManager<C> {
    pub fn replicas(&self) -> Vec<ReplicaStatus> {
        self.inner
            .replicas
            .iter()
            .map(|replica| replica.state.lock().unwrap().status.clone())
            .collect()
    }

    pub async fn wait_replicas(
        &self,
        through: &StorageCursor,
        targets: &[Identifier],
    ) -> Result<Vec<ReplicaStatus>> {
        if !self.status().saved.covers(through)? {
            return Err(invalid("boundary is not saved on the primary"));
        }
        let selected = targets
            .iter()
            .map(|id| {
                self.inner
                    .replicas
                    .iter()
                    .find(|replica| replica.config.id == *id)
                    .cloned()
                    .ok_or_else(|| invalid("unknown replica identity"))
            })
            .collect::<Result<Vec<_>>>()?;
        for replica in &selected {
            replica.retry();
        }
        self.inner.notify();
        let mut changed = self.inner.changed.subscribe();
        loop {
            let mut done = true;
            let mut statuses = Vec::with_capacity(selected.len());
            for replica in &selected {
                let status = replica.state.lock().unwrap().status.clone();
                if !status
                    .saved
                    .as_ref()
                    .is_some_and(|saved| saved.sequence >= through.sequence)
                {
                    if let Some(source) = &status.failure {
                        return Err(Error::Persistence {
                            source: source.clone(),
                        });
                    }
                    if status.closed || !status.running {
                        return Err(Error::NotReady {
                            domain: "replica worker",
                        });
                    }
                    done = false;
                }
                statuses.push(status);
            }
            if done {
                return Ok(statuses);
            }
            changed
                .changed()
                .await
                .map_err(|_| invalid("replica worker stopped"))?;
        }
    }

    pub(super) async fn settle_replicas(&self, ready: bool) -> Result<()> {
        if ready && !self.inner.settings.replication_shutdown.is_zero() {
            let through = self.status().saved;
            let targets: Vec<_> = self
                .inner
                .replicas
                .iter()
                .map(|r| r.config.id.clone())
                .collect();
            executor::timeout(
                self.inner.settings.replication_shutdown,
                self.wait_replicas(&through, &targets),
            )
            .await
            .map_err(|_| {
                Error::backend(
                    "replica shutdown",
                    BackendFailure::Transient,
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "replica shutdown boundary was not saved",
                    ),
                )
            })??;
        }
        for replica in &self.inner.replicas {
            replica.state.lock().unwrap().stopping = true;
            replica.retry();
        }
        self.inner.notify();
        let mut changed = self.inner.changed.subscribe();
        loop {
            let mut done = true;
            for replica in &self.inner.replicas {
                let status = replica.state.lock().unwrap().status.clone();
                if status.closed {
                    continue;
                }
                if let Some(source) = status.failure {
                    return Err(Error::Persistence { source });
                }
                if !status.running {
                    close(&self.erased(), replica).await?;
                } else {
                    done = false;
                }
            }
            if done {
                return Ok(());
            }
            changed
                .changed()
                .await
                .map_err(|_| invalid("replica worker stopped"))?;
        }
    }

    pub(super) async fn reserve_replication(&self, replica: &Replica) -> Result<MemoryReservation> {
        let bytes = transfer_bytes(&self.inner.settings, self.inner.has_objects)?;
        let mut changed = self.inner.changed.subscribe();
        loop {
            replica.check_copying()?;
            {
                let mut state = self.inner.state.lock().unwrap();
                if !state.ready || !state.status.running || state.status.closed {
                    return Err(Error::NotReady { domain: "storage" });
                }
                state.collect();
                if state.replication_bytes
                    <= self.inner.settings.queue_capacity_bytes.get() / 2 - bytes
                    && state.status.pending_bytes
                        <= self.inner.settings.queue_capacity_bytes.get() - bytes
                {
                    state.scratch_bytes += bytes;
                    state.replication_bytes += bytes;
                    state.status.pending_bytes += bytes;
                    return Ok(MemoryReservation {
                        inner: self.inner.clone(),
                        bytes,
                        kind: ReservationKind::Replication,
                    });
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                self.inner.relieve_pressure(&mut state, bytes, 0);
            }
            changed
                .changed()
                .await
                .map_err(|_| invalid("storage worker stopped"))?;
        }
    }
}

impl<A: Application> Resident<A> for Worker {
    type Error = Error;

    async fn stop(&mut self) -> Result<()> {
        close(&self.manager, &self.replica).await
    }

    async fn finalize(&mut self) -> Result<()> {
        self.reservation = None;
        Ok(())
    }

    async fn attempt(&mut self, mut context: Service<A>) -> std::result::Result<(), Fault<Error>> {
        let _running = Running {
            manager: self.manager.clone(),
            replica: self.replica.clone(),
        };
        let mut changed = self.manager.inner.changed.subscribe();
        let mut generation = 0;
        let mut failures = 0usize;
        let mut retry_at = None;
        // Physical I/O can outlive an uncertain result. Retain its budget until
        // reconciliation or successful owner close, including retry backoff.
        context.ready();
        loop {
            changed.borrow_and_update();
            let primary = self.manager.status().saved;
            let work = {
                let mut state = self.replica.state.lock().unwrap();
                if state.status.closed {
                    return Ok(());
                }
                if state.retry != generation {
                    generation = state.retry;
                    failures = 0;
                    retry_at = None;
                    state.status.failure = None;
                }
                if retry_at.is_some_and(|at| executor::now() >= at) {
                    retry_at = None;
                    state.status.failure = None;
                }
                state.status.failure.is_none()
                    && (state.stopping
                        || (self.manager.inner.has_objects
                            && state
                                .status
                                .saved
                                .as_ref()
                                .is_some_and(|saved| saved.sequence > state.objects_pruned))
                        || state.bootstrap.is_some()
                        || state.snapshot.is_some()
                        || state.staging.is_some()
                        || state
                            .status
                            .saved
                            .as_ref()
                            .is_none_or(|saved| saved.sequence < primary.sequence))
            };
            if work {
                let Some(result) = context
                    .until_stopping(run(
                        &self.manager,
                        &self.replica,
                        primary,
                        &mut self.reservation,
                    ))
                    .await
                else {
                    return Ok(());
                };
                match result {
                    Ok(()) => {
                        failures = 0;
                    }
                    Err(Error::Closing { domain: "replica" }) => {}
                    Err(error) => {
                        // A failed target must not pin obsolete primary versions
                        // throughout an outage. Failed cleanup remains owned.
                        let error = cleanup(&self.manager, &self.replica)
                            .await
                            .err()
                            .unwrap_or(error);
                        failures = failures.saturating_add(1);
                        retry_at = if transient(&error)
                            && failures <= self.manager.inner.settings.max_retries
                        {
                            let multiplier = 1u32
                                .checked_shl((failures - 1).min(31) as u32)
                                .unwrap_or(u32::MAX);
                            Some(
                                executor::now()
                                    + self
                                        .manager
                                        .inner
                                        .settings
                                        .retry_base
                                        .saturating_mul(multiplier)
                                        .min(self.manager.inner.settings.retry_cap),
                            )
                        } else {
                            None
                        };
                        self.replica.state.lock().unwrap().status.failure = Some(Arc::new(error));
                    }
                }
                self.manager.inner.notify();
                continue;
            }
            let waiting = async {
                let wake = changed.changed();
                let sleep = async {
                    if let Some(at) = retry_at {
                        executor::sleep(at.saturating_duration_since(executor::now())).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                };
                let mut wake = std::pin::pin!(wake);
                let mut sleep = std::pin::pin!(sleep);
                std::future::poll_fn(|cx| {
                    if wake.as_mut().poll(cx).is_ready() || sleep.as_mut().poll(cx).is_ready() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
                .await;
            };
            if context.until_stopping(waiting).await.is_none() {
                return Ok(());
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.replica.state.lock().unwrap().status.running = false;
        self.manager.inner.notify();
    }
}

impl<C> StorageManager<C> {
    pub fn acknowledge(
        &self,
        replica: &Identifier,
        holder: &ReplicaHolder,
        cursor: &StorageCursor,
    ) -> Result<StorageCursor> {
        let accepted = self.status().accepted;
        if cursor.history != accepted.history || cursor.sequence > accepted.sequence {
            return Err(Error::InvalidInput {
                field: "replica receipt",
                reason: "the cursor must name an accepted position of this store",
            });
        }
        let mut entries = self.inner.receipts.entries.lock().unwrap();
        if let Some(bound) = entries.get(replica) {
            if bound.holder.owner != holder.owner {
                return Err(Error::OperationUnauthorized {
                    caller: holder.owner.clone(),
                    owner: bound.holder.owner.clone(),
                    operation: format!("replica `{replica}`"),
                });
            }
            if holder.generation < bound.holder.generation {
                return Err(Error::Closing {
                    domain: "fenced replica generation",
                });
            }
            if holder.generation == bound.holder.generation
                && cursor.sequence < bound.cursor.sequence
            {
                return Err(Error::InvalidInput {
                    field: "replica receipt",
                    reason: "a receipt never regresses within one generation",
                });
            }
        }
        entries.insert(
            replica.clone(),
            ReplicaReceipt {
                replica: replica.clone(),
                holder: holder.clone(),
                cursor: cursor.clone(),
            },
        );
        drop(entries);
        self.inner.notify();
        Ok(cursor.clone())
    }

    pub fn release_replica(&self, replica: &Identifier, holder: &ReplicaHolder) -> Result<()> {
        let mut entries = self.inner.receipts.entries.lock().unwrap();
        let Some(bound) = entries.get(replica) else {
            return Ok(());
        };
        if bound.holder.owner != holder.owner || holder.generation < bound.holder.generation {
            return Err(Error::OperationUnauthorized {
                caller: holder.owner.clone(),
                owner: bound.holder.owner.clone(),
                operation: format!("replica `{replica}`"),
            });
        }
        entries.remove(replica);
        drop(entries);
        self.inner.notify();
        Ok(())
    }

    pub fn receipts(&self) -> Vec<ReplicaReceipt> {
        self.inner
            .receipts
            .entries
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }
}

/* === Public Functions === */
pub(super) fn configure<C: ObjectCatalog>(
    settings: &StorageSettings,
    primary: &Arc<dyn StorageBackend<Catalog = C>>,
    targets: &[ReplicaTarget<C>],
) -> Result<(Vec<Arc<Replica>>, usize)> {
    if targets.is_empty() {
        return Ok((Vec::new(), 0));
    }
    if transfer_bytes(settings, C::definition_count() != 0)?
        .checked_mul(targets.len())
        .is_none_or(|bytes| bytes > settings.queue_capacity_bytes.get() / 2)
    {
        return Err(invalid(
            "all replica transfer bounds must fit half the shared memory budget",
        ));
    }
    let mut names = BTreeSet::new();
    let mut replicas: Vec<Arc<Replica>> = Vec::new();
    let mut total = 0usize;
    for (index, target) in targets.iter().enumerate() {
        primary.history().check(target.backend.history())?;
        if !names.insert(&target.id)
            || Arc::ptr_eq(primary, &target.backend)
            || targets[..index]
                .iter()
                .any(|other| Arc::ptr_eq(&other.backend, &target.backend))
        {
            return Err(invalid(
                "replica identities and physical owners must be distinct",
            ));
        }
        let base_bytes = encoded_size(&(&target.id, primary.history()))?
            .checked_mul(16)
            .and_then(|n| n.checked_add(settings.snapshot_bytes.get().checked_mul(4)?))
            .and_then(|n| n.checked_add(settings.replication_limits.bytes.get().checked_mul(5)?))
            .and_then(|n| n.checked_add(ENTRY_OVERHEAD * 8))
            .ok_or(invalid("replica metadata size overflow"))?;
        total = total
            .checked_add(base_bytes)
            .ok_or(invalid("replica metadata size overflow"))?;
        if total > settings.queue_capacity_bytes.get() / 4 {
            return Err(invalid(
                "replica metadata exceeds a quarter of shared memory",
            ));
        }
        replicas.push(Arc::new(Replica {
            config: ErasedReplica {
                id: target.id.clone(),
                backend: super::backend::erase(target.backend.clone()),
            },
            base_bytes,
            state: Mutex::new(State {
                status: ReplicaStatus {
                    id: target.id.clone(),
                    saved: None,
                    running: false,
                    closed: false,
                    failure: None,
                },
                touched: false,
                snapshot: None,
                bootstrap: None,
                staging: None,
                extra_staging: None,
                stopping: false,
                retry: 0,
                objects_pruned: 0,
            }),
        }));
    }
    Ok((replicas, total))
}

pub(super) async fn cleanup(
    manager: &StorageManager<super::manager::ErasedCatalog>,
    replica: &Replica,
) -> Result<()> {
    let target = async {
        let mut failure = None;
        let extra = replica.state.lock().unwrap().extra_staging.clone();
        if let Some(extra) = extra {
            let abort = match &extra.bootstrap {
                Some(identity) => replica
                    .config
                    .backend
                    .abort_bootstrap_file(identity, &extra.file),
                None => replica.config.backend.abort_file(&extra.file),
            };
            match abort.await {
                Ok(()) => replica.state.lock().unwrap().extra_staging = None,
                Err(error) => {
                    failure = Some(error);
                }
            }
        }
        let staging = replica.state.lock().unwrap().staging.clone();
        if let Some(staging) = staging {
            let abort = match &staging.bootstrap {
                Some(identity) => replica
                    .config
                    .backend
                    .abort_bootstrap_file(identity, &staging.file),
                None => replica.config.backend.abort_file(&staging.file),
            };
            match abort.await {
                Ok(()) => replica.state.lock().unwrap().staging = None,
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        let bootstrap = replica.state.lock().unwrap().bootstrap.clone();
        if let Some(identity) = bootstrap.filter(|_| failure.is_none()) {
            match replica.config.backend.abort_bootstrap(&identity).await {
                Ok(()) => replica.state.lock().unwrap().bootstrap = None,
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    };
    // Release source pins independently: a stalled target cleanup must not
    // retain obsolete primary versions. Neither cleanup uses the I/O deadline.
    let source = async {
        let snapshot = replica.state.lock().unwrap().snapshot.clone();
        if let Some(snapshot) = snapshot {
            manager.inner.primary.release_view(&snapshot).await?;
            replica.state.lock().unwrap().snapshot = None;
            manager.inner.state.lock().unwrap().snapshot_released();
            manager.inner.notify();
        }
        Ok(())
    };
    let mut target = std::pin::pin!(target);
    let mut source = std::pin::pin!(source);
    let mut target_result = None;
    let mut source_result = None;
    poll_fn(|context| {
        if target_result.is_none()
            && let Poll::Ready(result) = target.as_mut().poll(context)
        {
            target_result = Some(result);
        }
        if source_result.is_none()
            && let Poll::Ready(result) = source.as_mut().poll(context)
        {
            source_result = Some(result);
        }
        if target_result.is_some() && source_result.is_some() {
            Poll::Ready(
                target_result
                    .take()
                    .unwrap()
                    .and(source_result.take().unwrap()),
            )
        } else {
            Poll::Pending
        }
    })
    .await
}

/* === Private Functions === */
fn transfer_bytes(settings: &StorageSettings, objects: bool) -> Result<usize> {
    // Five Pages, One Temporary Native Verification, And File Copy State.
    settings
        .replication_limits
        .bytes
        .get()
        .checked_mul(20)
        .and_then(|n| {
            n.checked_add(
                settings
                    .replication_limits
                    .rows
                    .get()
                    .checked_mul(ENTRY_OVERHEAD * 5)?,
            )
        })
        .and_then(|n| {
            if objects {
                n.checked_add(
                    crate::storage::catalog::verification_bytes(settings.replication_limits.bytes)
                        .ok()?,
                )
            } else {
                Some(n)
            }
        })
        .and_then(|n| n.checked_add(settings.file_chunk_bytes.get()))
        .and_then(|n| {
            n.checked_add(StorageSchema::decoding_memory_bytes(
                settings.replication_limits.bytes.get(),
            )?)
        })
        .ok_or(invalid("transfer bounds overflow"))
}

async fn run(
    manager: &StorageManager<super::manager::ErasedCatalog>,
    replica: &Replica,
    through: StorageCursor,
    reservation: &mut Option<MemoryReservation>,
) -> Result<()> {
    if replica.state.lock().unwrap().stopping {
        close(manager, replica).await?;
        *reservation = None;
        return Ok(());
    }
    cleanup(manager, replica).await?;
    replica.check_copying()?;
    let opened = replica.state.lock().unwrap().status.saved.is_some();
    replica.state.lock().unwrap().touched = true;
    let saved = if opened {
        manager
            .inner
            .bounded(replica.config.backend.cursor_saved())
            .await?
    } else {
        manager.inner.bounded(replica.config.backend.open()).await?
    };
    if !through.covers(&saved)? {
        return Err(invalid("replica is ahead of the primary"));
    }
    {
        let mut state = replica.state.lock().unwrap();
        if let Some(previous) = &state.status.saved
            && !saved.covers(previous)?
        {
            return Err(invalid("replica durable receipt regressed"));
        }
        state.status.saved = Some(saved.clone());
    }
    manager.inner.notify();
    if saved != through {
        if reservation.is_none() {
            *reservation = Some(manager.reserve_replication(replica).await?);
        }
        match super::transfer::copy(manager, replica, &saved, &through).await {
            Ok(()) => {}
            Err(Error::Gap { .. }) if manager.inner.primary.supports_retained_snapshots() => {
                cleanup(manager, replica).await?;
                super::bootstrap::copy(manager, replica, &through).await?;
            }
            Err(error) => return Err(error),
        }
        replica.check_copying()?;
        cleanup(manager, replica).await?;
    }
    if manager.inner.has_objects && through.sequence > replica.state.lock().unwrap().objects_pruned
    {
        let done = manager
            .inner
            .bounded(
                replica
                    .config
                    .backend
                    .prune_objects(&through, manager.inner.settings.object_prune_entries),
            )
            .await?;
        if done {
            replica.state.lock().unwrap().objects_pruned = through.sequence;
        }
    }
    *reservation = None;
    Ok(())
}

async fn close(
    manager: &StorageManager<super::manager::ErasedCatalog>,
    replica: &Replica,
) -> Result<()> {
    if replica.state.lock().unwrap().status.closed {
        return Ok(());
    }
    cleanup(manager, replica).await?;
    let touched = replica.state.lock().unwrap().touched;
    if touched {
        replica.config.backend.close().await?;
    }
    let mut state = replica.state.lock().unwrap();
    if !state.status.closed {
        state.status.closed = true;
        state.status.failure = None;
        drop(state);
        let mut storage = manager.inner.state.lock().unwrap();
        storage.replica_base_bytes -= replica.base_bytes;
        storage.collect();
    }
    manager.inner.notify();
    Ok(())
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "replication",
        reason,
    }
}

fn transient(error: &Error) -> bool {
    match error {
        Error::Backend {
            kind: BackendFailure::Transient,
            ..
        } => true,
        Error::Persistence { source } => transient(source),
        _ => false,
    }
}

/* === Tests === */
