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
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;

// Third Party Imports

// Local Imports
use super::batching::{Batch, Progress, write_data};
use super::manager::{Domain, Inner, Operation, Pending, Target};
use super::staging::{FileAction, FileCompleted};
use super::views::ViewAction;
use super::{RecoveryPosition, StorageManager, StoragePart};
use crate::supervision::executor;
use crate::{
    Application, BackendFailure, Error, Result, services::Fault, services::Resident,
    services::Service,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct Worker(pub(super) StorageManager<crate::storage::ErasedCatalog>);

struct Running(Arc<Inner>);

/* === Enums === */
enum Action {
    Write(Arc<Pending>),
    View(ViewAction),
    File(FileAction),
    Save(Arc<super::saving::SaveWork>),
    Delete(Arc<super::deletion::DeleteWork>),
    Recovery(Target),
    Prune(super::StorageCursor),
}

enum Completed {
    Write(Progress),
    Opened(u64, Result<super::BackendSnapshot>),
    Released(u64),
    File(FileCompleted),
    Saved,
    Deleted,
    Recovery(RecoveryPosition),
    Pruned(bool),
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> Resident<A> for Worker {
    type Error = Error;

    async fn cleanup(&mut self) -> Result<()> {
        // StorageManager owns physical backend teardown before this worker's
        // lease retires. An early stop cannot claim that teardown completed.
        if !self.0.inner.state.lock().unwrap().status.closed {
            return Err(Error::NotReady {
                domain: "storage cleanup",
            });
        }
        Ok(())
    }

    async fn run(&mut self, mut context: Service<A>) -> std::result::Result<(), Fault<Error>> {
        let inner = &self.0.inner;
        let _running = Running(inner.clone());
        let mut changed = inner.changed.subscribe();
        let mut next_flush = executor::now() + inner.settings.flush_interval;
        let mut retry_at = None;
        let mut failures = 0usize;
        let mut retry_generation = 0;
        let mut prefer_views = true;
        let mut prefer_files = true;
        let mut prefer_pruning = true;
        context.ready();
        loop {
            changed.borrow_and_update();
            let now = executor::now();
            let replica_floor = inner.replica_floor();
            let retention_floor = inner.retention_floor();
            let deletion_replicas_ready = inner.replicas.iter().all(|replica| {
                let state = replica.state.lock().unwrap();
                state.status.saved.is_some()
                    && !state.status.closed
                    && state.status.failure.is_none()
            });
            let replica_ceiling =
                replica_floor.saturating_add(inner.settings.replica_lag_positions.get());
            let action = {
                let mut state = inner.state.lock().unwrap();
                if state.status.closed {
                    return Ok(());
                }
                if state.retry != retry_generation {
                    retry_generation = state.retry;
                    failures = 0;
                    retry_at = None;
                    state.status.failure = None;
                }
                if retry_at.is_some_and(|deadline| now >= deadline) {
                    retry_at = None;
                    state.status.failure = None;
                }
                if now >= next_flush {
                    state.request_flush();
                    next_flush = now + inner.settings.flush_interval;
                }
                state.expire_files();
                state.request_saves();
                state.request_deletions();
                state.collect();
                if state.queue.bytes() >= inner.settings.target_batch_bytes.get() {
                    state.request_flush();
                }
                // Already registered snapshots acquire their pins before maintenance.
                let prune_through = if state
                    .views
                    .values()
                    .any(|view| !view.expired && view.result.is_none())
                {
                    0
                } else {
                    state.status.saved.sequence.min(retention_floor)
                };
                let mut prune = (inner.has_objects
                    && state.ready
                    && !state.status.closing
                    && state.status.failure.is_none()
                    && prune_through > state.objects_pruned)
                    .then(|| Action::Prune(self.0.position(prune_through)));
                let data = if prefer_pruning && prune.is_some() {
                    prune.take()
                } else if !state.ready || state.status.failure.is_some() {
                    None
                } else if let Some(save) = state.save_action() {
                    Some(Action::Save(save))
                } else if let Some(deletion) = deletion_replicas_ready
                    .then(|| state.deletion_action(replica_floor))
                    .flatten()
                {
                    Some(Action::Delete(deletion))
                } else if let Some(target) = state
                    .recovery_request
                    .filter(|target| target.index <= state.recovery_written.index)
                {
                    Some(Action::Recovery(target))
                } else if let Some(boundary) = state.write_boundary() {
                    // A primary barrier does not require recovery I/O. Keep
                    // both domains in the same queue, but prefer its data while
                    // preserving the earliest explicitly requested boundary.
                    let mut pending = state
                        .queue
                        .iter()
                        .filter(|entry| entry.index <= boundary && state.needs_write(entry));
                    let first = pending.next();
                    let selected = if state.data_request.is_some()
                        && first.is_some_and(|entry| entry.operation.domain() == Domain::Recovery)
                    {
                        pending
                            .find(|entry| entry.operation.domain() == Domain::Data)
                            .or(first)
                    } else {
                        first
                    };
                    selected.cloned().map(Action::Write)
                } else {
                    None
                };
                let data = data.or(prune);
                // Alternate persistence with bounded control work; rotate files
                // and views so neither producer churn nor reads can starve a peer.
                let selected = if prefer_views || data.is_none() {
                    let healthy = state.status.failure.is_none();
                    let file = if prefer_files && healthy {
                        state.file_action(replica_ceiling).map(Action::File)
                    } else {
                        None
                    };
                    let control = file
                        .or_else(|| state.view_action(healthy).map(Action::View))
                        .or_else(|| {
                            if healthy {
                                state.file_action(replica_ceiling).map(Action::File)
                            } else {
                                None
                            }
                        });
                    if let Some(action) = &control {
                        prefer_files = !matches!(action, Action::File(_));
                    }
                    control.or(data)
                } else {
                    data
                };
                state.pruning_objects = matches!(selected, Some(Action::Prune(_)));
                if state.pruning_objects {
                    state.objects_prune_owed = false;
                }
                selected
            };
            if let Some(action) = action {
                prefer_pruning = !matches!(action, Action::Prune(_));
                prefer_views = !matches!(action, Action::View(_) | Action::File(_));
                let Some(result) = context.until_stopping(perform(inner, &action)).await else {
                    return Ok(());
                };
                if matches!(action, Action::Prune(_)) {
                    inner.state.lock().unwrap().pruning_objects = false;
                }
                if let Ok(Completed::Opened(id, snapshot)) = result {
                    inner.state.lock().unwrap().opened(
                        id,
                        Arc::downgrade(inner),
                        snapshot,
                        inner.settings.snapshot_bytes.get(),
                        inner.settings.view_idle_timeout,
                    );
                    inner.notify();
                    continue;
                }
                match result {
                    Ok(completed) => {
                        failures = 0;
                        let mut state = inner.state.lock().unwrap();
                        state.status.failure = None;
                        match (action, completed) {
                            (Action::Delete(work), Completed::Deleted) => {
                                state
                                    .receipts
                                    .complete(&work.receipt.mutation, work.receipt.value.clone());
                                state.deletions.remove(&work.receipt.mutation);
                            }
                            (Action::Save(work), Completed::Saved) => {
                                state
                                    .receipts
                                    .complete(&work.receipt.mutation, work.receipt.value.clone());
                                state.saves.remove(&work.receipt.mutation);
                            }
                            (Action::Prune(through), Completed::Pruned(done)) => {
                                if done && !state.objects_prune_owed {
                                    state.objects_pruned = through.sequence;
                                }
                            }
                            (Action::File(action), Completed::File(result)) => state
                                .file_completed(
                                    action,
                                    result,
                                    inner.settings.file_session_timeout,
                                ),
                            (Action::View(ViewAction::Release(..)), Completed::Released(id)) => {
                                state.remove_view(id);
                                state.snapshot_released();
                            }
                            (Action::Write(_), Completed::Write(progress)) => {
                                if let Some(target) = progress.data {
                                    let saved =
                                        progress.saved.expect("data write returns durability");
                                    let through = if saved.sequence == target.sequence {
                                        target.index
                                    } else {
                                        state
                                            .queue
                                            .iter()
                                            .filter(|entry| {
                                                entry.operation.domain() == Domain::Data
                                                    && entry.operation.sequence() <= saved.sequence
                                            })
                                            .map(|entry| entry.index)
                                            .max()
                                            .unwrap_or(state.data_flushed)
                                    };
                                    state.status.saved = saved;
                                    state.data_flushed = through;
                                    state.retire_data();
                                    if state
                                        .data_request
                                        .is_some_and(|request| request.index <= through)
                                    {
                                        state.data_request = None;
                                    }
                                }
                                if let Some(target) = progress.recovery {
                                    state.recovery_written.index = target.index;
                                    state.recovery_written.sequence =
                                        state.recovery_written.sequence.max(target.sequence);
                                }
                            }
                            (Action::Recovery(target), Completed::Recovery(saved)) => {
                                if let Some(progress) = &state.journal_saved
                                    && let Some(frontier) = state
                                        .queue
                                        .iter()
                                        .filter(|entry| entry.index <= target.index)
                                        .filter_map(|entry| entry.journal)
                                        .max()
                                {
                                    progress.send_replace(frontier);
                                }
                                state.status.recovery_saved = saved;
                                state.recovery_flushed = target.index;
                                state.queue.retire(Domain::Recovery, target.index);
                                state.recovery_request = None;
                            }
                            _ => unreachable!("completion matches its storage action"),
                        }
                        state.collect();
                    }
                    Err(error) => {
                        failures = failures.saturating_add(1);
                        // Receipt-Addressed Calls Use The Shared Operation Retry Policy.
                        // Do Not Multiply Its Three Retries By Another Physical Budget.
                        let retries = match &action {
                            Action::Save(_) | Action::Delete(_) => 0,
                            Action::Write(pending)
                                if pending.operation.domain() == Domain::Data && {
                                    let state = inner.state.lock().unwrap();
                                    state.saving_data(pending.index)
                                        || state.deleting_data(pending.index)
                                } =>
                            {
                                0
                            }
                            Action::File(file)
                                if file.caller_retries(&inner.state.lock().unwrap()) =>
                            {
                                0
                            }
                            _ => inner.settings.max_retries,
                        };
                        retry_at = if transient(&error) && failures <= retries {
                            let multiplier = 1u32
                                .checked_shl((failures - 1).min(31) as u32)
                                .unwrap_or(u32::MAX);
                            Some(
                                executor::now()
                                    + inner
                                        .settings
                                        .retry_base
                                        .saturating_mul(multiplier)
                                        .min(inner.settings.retry_cap),
                            )
                        } else {
                            None
                        };
                        let source = Arc::new(error);
                        let mut state = inner.state.lock().unwrap();
                        if let Action::File(action) = &action {
                            state.file_failed(action, source.clone(), retry_at.is_some());
                        }
                        match &action {
                            Action::Delete(work) => {
                                state.deletion_failed(&work.receipt.mutation, source.clone())
                            }
                            Action::Save(work) => {
                                state.save_failed(&work.receipt.mutation, source.clone())
                            }
                            Action::Write(pending)
                                if pending.operation.domain() == Domain::Data =>
                            {
                                state.save_data_failed(pending.index, source.clone());
                                state.deletion_data_failed(pending.index, source.clone())
                            }
                            _ => {}
                        }
                        state.status.failure = Some(source);
                    }
                }
                inner.notify();
            } else {
                let deadline = retry_at.map_or(next_flush, |retry| retry.min(next_flush));
                let deadline = inner
                    .state
                    .lock()
                    .unwrap()
                    .view_deadline()
                    .map_or(deadline, |view| view.min(deadline));
                let deadline = inner
                    .state
                    .lock()
                    .unwrap()
                    .file_deadline()
                    .map_or(deadline, |file| file.min(deadline));
                let mut sleep = Box::pin(executor::sleep(
                    deadline.saturating_duration_since(executor::now()),
                ));
                let mut wake = Box::pin(changed.changed());
                let wait = std::future::poll_fn(|cx| {
                    if wake.as_mut().poll(cx).is_ready() || sleep.as_mut().poll(cx).is_ready() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                });
                if context.until_stopping(wait).await.is_none() {
                    return Ok(());
                }
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.status.running = false;
        state.pruning_objects = false;
        if !state.status.closed {
            state.status.failure = Some(Arc::new(Error::NotReady {
                domain: "storage worker",
            }));
        }
        self.0.notify();
    }
}

impl Operation {
    pub(super) fn check_entry_bytes(&self, maximum: std::num::NonZeroUsize) -> Result<()> {
        let part = match self {
            Self::Schema(value) => StoragePart::Schemas(std::slice::from_ref(value)),
            Self::ObjectGroup {
                schemas, objects, ..
            } => {
                for (schema, object) in schemas.iter().zip(objects) {
                    for part in [
                        StoragePart::Schemas(std::slice::from_ref(schema)),
                        StoragePart::Objects(std::slice::from_ref(object)),
                    ] {
                        for entry in part.entries() {
                            if entry.encoded_bytes()? > maximum.get() {
                                return Err(Error::InvalidInput {
                                    field: "storage admission",
                                    reason: "one entry exceeds max_batch_bytes",
                                });
                            }
                        }
                    }
                }
                return Ok(());
            }
            Self::Records(values) => StoragePart::Records(values),
            Self::Stream(values) => StoragePart::Stream(values),
            Self::File(value) => StoragePart::Files(std::slice::from_ref(value)),
            Self::Recovery(_) | Self::Snapshot(_) => return Ok(()),
        };
        for entry in part.entries() {
            if entry.encoded_bytes()? > maximum.get() {
                return Err(Error::InvalidInput {
                    field: "storage admission",
                    reason: "one entry exceeds max_batch_bytes",
                });
            }
        }
        Ok(())
    }

    pub(super) fn domain(&self) -> Domain {
        match self {
            Self::Recovery(_) | Self::Snapshot(_) => Domain::Recovery,
            _ => Domain::Data,
        }
    }

    pub(super) fn sequence(&self) -> u64 {
        match self {
            Self::Schema(schema) => schema.position.sequence,
            Self::File(file) => file.position.sequence,
            Self::ObjectGroup { objects, .. } => objects.last().unwrap().position.sequence,
            Self::Records(records) => records.last().unwrap().position.sequence,
            Self::Stream(rows) => rows.last().unwrap().record.position.sequence,
            Self::Recovery(entries) => entries.last().unwrap().position.sequence,
            Self::Snapshot(snapshot) => snapshot.through.sequence,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn perform(inner: &Arc<Inner>, action: &Action) -> Result<Completed> {
    match action {
        Action::Save(work) => work.perform(inner).await.map(|()| Completed::Saved),
        Action::Delete(work) => work.perform(inner).await.map(|()| Completed::Deleted),
        Action::Prune(through) => inner
            .bounded(
                inner
                    .primary
                    .prune_objects(through, inner.settings.object_prune_entries),
            )
            .await
            .map(Completed::Pruned),
        Action::File(action) => super::staging::perform(inner, action)
            .await
            .map(Completed::File),
        Action::View(ViewAction::Open(id, saved, execution)) => {
            let acquired = match execution {
                Some(limits) => tokio::time::timeout(
                    limits.timeout,
                    inner.bounded(inner.primary.read_query_view(saved, *limits)),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(Error::ResourceExhausted {
                        resource: "query computation time",
                    })
                }),
                None => inner.bounded(inner.primary.read_view(saved)).await,
            };
            Ok(Completed::Opened(*id, acquired))
        }
        Action::View(ViewAction::Release(id, snapshot)) => {
            inner.primary.release_view(snapshot).await?;
            Ok(Completed::Released(*id))
        }
        Action::Write(entry) => {
            if let Some(batch) = Batch::prepare(inner, entry) {
                return batch.write(inner).await.map(Completed::Write);
            }
            let mut progress = Progress::default();
            progress.include(entry);
            let part = match &entry.operation {
                Operation::File(file) => Some(StoragePart::Files(std::slice::from_ref(file))),
                Operation::Schema(schema) => {
                    Some(StoragePart::Schemas(std::slice::from_ref(schema)))
                }
                Operation::Records(records) => Some(StoragePart::Records(records)),
                Operation::ObjectGroup {
                    schemas, objects, ..
                } => {
                    let mut parts = Vec::with_capacity(schemas.len() * 2);
                    for (schema, object) in schemas.iter().zip(objects) {
                        parts.push(StoragePart::Schemas(std::slice::from_ref(schema)));
                        parts.push(StoragePart::Objects(std::slice::from_ref(object)));
                    }
                    progress.saved = Some(write_data(inner, &parts).await?);
                    return Ok(Completed::Write(progress));
                }
                Operation::Stream(rows) => Some(StoragePart::Stream(rows)),
                Operation::Recovery(entries) => {
                    inner.bounded(inner.recovery.write_entries(entries)).await?;
                    None
                }
                Operation::Snapshot(snapshot) => {
                    inner
                        .bounded(inner.recovery.write_snapshot(snapshot))
                        .await?;
                    None
                }
            };
            if let Some(part) = part {
                progress.saved = Some(write_data(inner, &[part]).await?);
            }
            Ok(Completed::Write(progress))
        }
        Action::Recovery(target) => {
            let through = RecoveryPosition {
                history: inner.recovery.history().clone(),
                sequence: target.sequence,
            };
            let saved = inner.bounded(inner.recovery.flush(&through)).await?;
            let written = inner.state.lock().unwrap().recovery_written.sequence;
            if !saved.covers(&through)? || saved.sequence > written {
                return Err(Error::InvalidInput {
                    field: "recovery flush receipt",
                    reason: "receipt does not cover the requested written frontier",
                });
            }
            Ok(Completed::Recovery(saved))
        }
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
