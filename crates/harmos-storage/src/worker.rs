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
use super::snapshot::SnapshotAction;
use super::staging::{FileAction, FileCompleted};
use super::{RecoveryPosition, StorageManager, StoragePart};
use crate::{BackendFailure, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Worker(pub(crate) StorageManager<crate::storage::ErasedCatalog>);

struct Running(Arc<Inner>);

/* === Enums === */

enum Action {
    Write(Arc<Pending>),
    Snapshot(SnapshotAction),
    File(FileAction),
    Delete(Arc<super::deletion::DeleteWork>),
    Recovery(Target),
    Prune(super::StorageCursor),
}

enum Completed {
    Write(Progress),
    Opened(u64, Result<super::BackendSnapshot>),
    Released(u64),
    File(FileCompleted),
    Deleted,
    Recovery(RecoveryPosition),
    Pruned(bool),
}

/* === Traits === */

/* === Implementations === */

impl Worker {
    pub async fn cleanup(&mut self) -> Result<()> {
        // StorageManager owns physical backend teardown before this worker's
        // lease retires. An early stop cannot claim that teardown completed.
        if !self.0.inner.state.lock().unwrap().status.closed {
            return Err(Error::NotReady { domain: "storage cleanup" });
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let inner = &self.0.inner;
        let _running = Running(inner.clone());
        let mut changed = inner.changed.subscribe();
        let mut next_flush = inner.execution.now() + super::config::FLUSH_INTERVAL;
        let mut retry_at = None;
        let mut failures = 0usize;
        let mut retry_generation = 0;
        let mut prefer_control = true;
        let mut prefer_files = true;
        let mut prefer_pruning = true;
        let mut prefer_state = true;
        loop {
            changed.borrow_and_update();
            let now = inner.execution.now();
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
                    next_flush = now + super::config::FLUSH_INTERVAL;
                }
                state.expire_files();
                state.request_deletions();
                state.collect();
                if state.queue.bytes() >= inner.settings.target_batch_bytes() {
                    state.request_flush();
                }
                // Already registered snapshots acquire their pins before maintenance.
                let prune_through = if state.snapshot_opening() { 0 } else { state.status.saved.sequence };
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
                } else if let Some(deletion) = state.deletion_action() {
                    Some(Action::Delete(deletion))
                } else if let Some(target) = state.recovery_request.filter(|target| target.index <= state.recovery_written.index) {
                    Some(Action::Recovery(target))
                } else if let Some(boundary) = state.write_boundary() {
                    // A primary barrier does not require recovery I/O. Keep
                    // both domains in the same queue, but prefer its data while
                    // preserving the earliest explicitly requested boundary.
                    let mut pending = state.queue.iter().filter(|entry| entry.index <= boundary && state.needs_write(entry));
                    let first = pending.next();
                    let selected =
                        if state.data_request.is_some() && first.is_some_and(|entry| entry.operation.domain() == Domain::Recovery) {
                            pending.find(|entry| entry.operation.domain() == Domain::Data).or(first)
                        } else {
                            first
                        };
                    // Journal Order Is Independent Of Unrelated Measurements.
                    // Only The Earliest Unwritten State Entry May Bypass Data,
                    // And Its Declared Evidence Dependency Must Already Be Durable.
                    let ready_state = state
                        .recovery_request
                        .and_then(|target| {
                            state.queue.iter().find(|entry| {
                                entry.index <= target.index && entry.operation.domain() == Domain::Recovery && state.needs_write(entry)
                            })
                        })
                        .filter(|entry| {
                            entry.operation.required_data().is_some_and(|required| state.status.saved.covers(required).unwrap_or(false))
                        });
                    let selected = if prefer_state { ready_state.or(selected) } else { selected };
                    selected.cloned().map(Action::Write)
                } else {
                    None
                };
                let data = data.or(prune);
                // Alternate persistence with bounded control work; rotate files
                // and views so neither producer churn nor reads can starve a peer.
                let selected = if prefer_control || data.is_none() {
                    let healthy = state.status.failure.is_none();
                    let file = if prefer_files && healthy { state.file_action().map(Action::File) } else { None };
                    let control = file
                        .or_else(|| state.snapshot_action(healthy).map(Action::Snapshot))
                        .or_else(|| if healthy { state.file_action().map(Action::File) } else { None });
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
                if let Action::Write(entry) = &action {
                    prefer_state = entry.operation.domain() != Domain::Recovery;
                }
                prefer_pruning = !matches!(action, Action::Prune(_));
                prefer_control = !matches!(action, Action::Snapshot(_) | Action::File(_));
                let result = perform(inner, &action).await;
                if matches!(action, Action::Prune(_)) {
                    inner.state.lock().unwrap().pruning_objects = false;
                }
                if let Ok(Completed::Opened(id, snapshot)) = result {
                    inner.state.lock().unwrap().snapshot_opened(id, snapshot);
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
                                state.receipts.complete(&work.receipt.mutation, work.receipt.value.clone());
                                state.deletions.remove(&work.receipt.mutation);
                            }
                            (Action::Prune(through), Completed::Pruned(done)) => {
                                if done && !state.objects_prune_owed {
                                    state.objects_pruned = through.sequence;
                                }
                            }
                            (Action::File(action), Completed::File(result)) => {
                                state.file_completed(action, result, super::config::FILE_SESSION_TIMEOUT)
                            }
                            (Action::Snapshot(SnapshotAction::Release(..)), Completed::Released(id)) => state.snapshot_removed(id),
                            (Action::Write(_), Completed::Write(progress)) => {
                                if let Some(target) = progress.data {
                                    let saved = progress.saved.expect("data write returns durability");
                                    let through = if saved.sequence == target.sequence {
                                        target.index
                                    } else {
                                        state
                                            .queue
                                            .iter()
                                            .filter(|entry| {
                                                entry.operation.domain() == Domain::Data && entry.operation.sequence() <= saved.sequence
                                            })
                                            .map(|entry| entry.index)
                                            .max()
                                            .unwrap_or(state.data_flushed)
                                    };
                                    state.status.saved = saved;
                                    state.data_flushed = through;
                                    state.retire_data();
                                    if state.data_request.is_some_and(|request| request.index <= through) {
                                        state.data_request = None;
                                    }
                                }
                                if let Some(target) = progress.recovery {
                                    state.recovery_written.index = target.index;
                                    state.recovery_written.sequence = state.recovery_written.sequence.max(target.sequence);
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
                            Action::Delete(_) => 0,
                            Action::Write(pending)
                                if pending.operation.domain() == Domain::Data && {
                                    let state = inner.state.lock().unwrap();
                                    state.deleting_data(pending.index)
                                } =>
                            {
                                0
                            }
                            Action::File(file) if file.caller_retries(&inner.state.lock().unwrap()) => 0,
                            _ => super::config::MAX_RETRIES,
                        };
                        retry_at = if transient(&error) && failures <= retries {
                            let multiplier = 1u32.checked_shl((failures - 1).min(31) as u32).unwrap_or(u32::MAX);
                            Some(inner.execution.now() + super::config::RETRY_BASE.saturating_mul(multiplier).min(super::config::RETRY_CAP))
                        } else {
                            None
                        };
                        let source = Arc::new(error);
                        let mut state = inner.state.lock().unwrap();
                        if let Action::File(action) = &action {
                            state.file_failed(action, source.clone(), retry_at.is_some());
                        }
                        match &action {
                            Action::Delete(work) => state.deletion_failed(&work.receipt.mutation, source.clone()),
                            Action::Write(pending) if pending.operation.domain() == Domain::Data => {
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
                let deadline = inner.state.lock().unwrap().file_deadline().map_or(deadline, |file| file.min(deadline));
                let mut sleep = Box::pin(inner.execution.sleep(deadline.saturating_duration_since(inner.execution.now())));
                let mut wake = Box::pin(changed.changed());
                let wait = std::future::poll_fn(|cx| {
                    if wake.as_mut().poll(cx).is_ready() || sleep.as_mut().poll(cx).is_ready() { Poll::Ready(()) } else { Poll::Pending }
                });
                wait.await;
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
            state.status.failure = Some(Arc::new(Error::NotReady { domain: "storage worker" }));
        }
        self.0.notify();
    }
}

impl Operation {
    pub(super) fn encoded_batch_bytes(&self) -> Result<usize> {
        let sum = |parts: &[StoragePart<'_>]| -> Result<usize> {
            parts.iter().flat_map(|part| part.entries()).try_fold(0usize, |bytes, entry| {
                bytes
                    .checked_add(entry.encoded_bytes()?)
                    .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded batch size overflow" })
            })
        };
        match self {
            Self::Schema(value) => sum(&[StoragePart::Schemas(std::slice::from_ref(value))]),
            Self::ObjectGroup { schemas, objects, .. } => {
                let mut bytes = 0usize;
                for (schema, object) in schemas.iter().zip(objects) {
                    let next =
                        sum(&[StoragePart::Schemas(std::slice::from_ref(schema)), StoragePart::Objects(std::slice::from_ref(object))])?;
                    bytes = bytes
                        .checked_add(next)
                        .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded batch size overflow" })?;
                }
                Ok(bytes)
            }
            Self::Records(values) => sum(&[StoragePart::Records(values)]),
            Self::Stream(values) => sum(&[StoragePart::Stream(values)]),
            Self::File(value) => sum(&[StoragePart::Files(std::slice::from_ref(value))]),
            Self::Recovery(_) | Self::Snapshot(_) => Ok(0),
        }
    }

    pub(super) fn check_entry_bytes(&self, maximum: std::num::NonZeroUsize) -> Result<()> {
        if self.encoded_batch_bytes()? > maximum.get() {
            return Err(Error::InvalidInput {
                field: "storage admission",
                reason: "atomic batch exceeds max_batch_bytes; split before submission",
            });
        }
        Ok(())
    }

    pub(super) fn required_data(&self) -> Option<&super::StorageCursor> {
        match self {
            Self::Recovery(entries) => entries.iter().max_by_key(|entry| entry.required_data.sequence).map(|entry| &entry.required_data),
            Self::Snapshot(snapshot) => Some(&snapshot.required_data),
            _ => None,
        }
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
        Action::Delete(work) => work.perform(inner).await.map(|()| Completed::Deleted),
        Action::Prune(through) => {
            inner.bounded(inner.primary.prune_objects(through, super::config::OBJECT_PRUNE_ENTRIES)).await.map(Completed::Pruned)
        }
        Action::File(action) => super::staging::perform(inner, action).await.map(Completed::File),
        Action::Snapshot(SnapshotAction::Open(id, at)) => {
            let opened = inner.bounded(inner.primary.open_snapshot(at)).await;
            Ok(Completed::Opened(*id, opened))
        }
        Action::Snapshot(SnapshotAction::Release(id, snapshot)) => {
            inner.bounded(inner.primary.release_snapshot(snapshot)).await?;
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
                Operation::Schema(schema) => Some(StoragePart::Schemas(std::slice::from_ref(schema))),
                Operation::Records(records) => Some(StoragePart::Records(records)),
                Operation::ObjectGroup { schemas, objects, .. } => {
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
                    let saved = inner.state.lock().unwrap().status.saved.clone();
                    for entry in entries {
                        if !saved.covers(&entry.required_data)? {
                            return Err(Error::InvalidInput {
                                field: "state evidence dependency",
                                reason: "journal cannot precede required primary persistence",
                            });
                        }
                    }
                    inner.bounded(inner.recovery.write_entries(entries)).await?;
                    None
                }
                Operation::Snapshot(snapshot) => {
                    if !inner.state.lock().unwrap().status.saved.covers(&snapshot.required_data)? {
                        return Err(Error::InvalidInput {
                            field: "state snapshot dependency",
                            reason: "snapshot cannot precede required primary persistence",
                        });
                    }
                    inner.bounded(inner.recovery.write_snapshot(snapshot)).await?;
                    None
                }
            };
            if let Some(part) = part {
                progress.saved = Some(write_data(inner, &[part]).await?);
            }
            Ok(Completed::Write(progress))
        }
        Action::Recovery(target) => {
            let through = RecoveryPosition { history: inner.recovery.history().clone(), sequence: target.sequence };
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
        Error::Backend { kind: BackendFailure::Transient, .. } => true,
        Error::Persistence { source } => transient(source),
        _ => false,
    }
}

/* === Tests === */
