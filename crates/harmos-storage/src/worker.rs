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
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

// Third Party Imports
use futures_util::{StreamExt, stream::FuturesUnordered};

// Local Imports
use super::batching::{Batch, write_data};
use super::manager::{Domain, Inner, Operation, Pending, Target};
use super::snapshot::SnapshotAction;
use super::staging::{FileAction, FileCompleted};
use super::{RecoveryPosition, StorageManager};
use crate::{BackendFailure, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Attempt = Pin<Box<dyn Future<Output = (Action, usize, Option<u64>, u64, Result<Completed>)> + Send>>;

/* === Structs === */

pub struct Worker(pub(crate) StorageManager<crate::storage::ErasedCatalog>);

struct Running(Arc<Inner>);

struct Failed {
    action: Option<Action>,
    source: Arc<Error>,
    attempts: usize,
    retry_at: Option<tokio::time::Instant>,
}

/* === Enums === */

enum Action {
    Write(Arc<Pending>, Option<Batch>),
    Snapshot(SnapshotAction),
    File(FileAction),
    Delete(Arc<super::deletion::DeleteWork>),
    Recovery(Target),
    Prune(super::StorageCursor),
}

enum Completed {
    Write { data: Option<Target>, saved: Option<super::StorageCursor>, recovery: Option<Target> },
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
        let _running = Running(Arc::clone(inner));
        let mut changed = inner.changed.subscribe();
        let mut next_flush = inner.execution.now() + super::config::FLUSH_INTERVAL;
        let mut retry_generation = 0;
        let mut failures = BTreeMap::<u64, Failed>::new();
        let mut next_failure = 0u64;
        let mut active = FuturesUnordered::<Attempt>::new();
        let mut files_running = 0usize;
        let mut ordered_running = false;
        let file_limit = inner.settings.request.max_active_file_operations.get();
        let mut prefer_control = true;
        let mut prefer_files = true;
        let mut prefer_pruning = true;
        let mut prefer_state = true;
        loop {
            changed.borrow_and_update();
            let now = inner.execution.now();
            let (action, failure_id, attempts, generation) = {
                let mut state = inner.state.lock().unwrap();
                if state.status.closed && active.is_empty() {
                    return Ok(());
                }
                if state.retry != retry_generation {
                    retry_generation = state.retry;
                    failures.clear();
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
                let retry = failures.iter().find_map(|(&id, failure)| {
                    let action = failure.action.as_ref()?;
                    let available = if action.independent() { files_running < file_limit } else { !ordered_running };
                    (available && failure.retry_at.is_some_and(|deadline| deadline <= now)).then_some(id)
                });
                let mut failure_id = None;
                let mut attempts = 0;
                let selected = if state.status.closed {
                    None
                } else if let Some(id) = retry {
                    let failure = failures.get_mut(&id).expect("selected failed owner");
                    attempts = failure.attempts;
                    failure_id = Some(id);
                    let action = failure.action.take().expect("waiting failed action");
                    match action {
                        Action::File(file) => {
                            let id = file.id();
                            drop(file);
                            state.file_action(!ordered_running, Some(id)).map(Action::File)
                        }
                        action => Some(action),
                    }
                } else {
                    // Already registered snapshots acquire their pins before maintenance.
                    let prune_through = if state.snapshot_opening() { 0 } else { state.status.saved.sequence };
                    let mut prune = (!ordered_running
                        && inner.has_objects
                        && state.ready
                        && !state.status.closing
                        && state.status.failure.is_none()
                        && prune_through > state.objects_pruned)
                        .then(|| Action::Prune(self.0.position(prune_through)));
                    let data = if prefer_pruning && prune.is_some() {
                        prune.take()
                    } else if ordered_running || !state.ready || state.status.failure.is_some() {
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
                        let selected = selected.map(|entry| entry.index);
                        drop(pending);
                        selected.map(|index| {
                            let entry = state.queue.iter().find(|entry| entry.index == index).expect("selected pending owner").clone();
                            Action::Write(entry, None)
                        })
                    } else {
                        None
                    };
                    let data = data.or(prune);
                    // Alternate persistence with bounded control work; rotate files
                    // and views so neither producer churn nor reads can starve a peer.
                    if prefer_control || data.is_none() {
                        let healthy = state.status.failure.is_none();
                        let file = if prefer_files && healthy && files_running < file_limit {
                            state.file_action(!ordered_running, None).map(Action::File)
                        } else {
                            None
                        };
                        let control = file
                            .or_else(|| if !ordered_running { state.snapshot_action(healthy).map(Action::Snapshot) } else { None })
                            .or_else(|| {
                                if healthy && files_running < file_limit {
                                    state.file_action(!ordered_running, None).map(Action::File)
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
                    }
                };
                if selected.is_none()
                    && let Some(id) = failure_id.take()
                {
                    failures.remove(&id);
                    state.status.failure = failures.values().next().map(|failure| Arc::clone(&failure.source));
                }
                // Prepare Scratch Only After The Scheduler Chose Its Final Action.
                let selected = match selected {
                    Some(Action::Write(entry, _)) => {
                        let index = entry.index;
                        drop(entry);
                        let batch = Batch::prepare(inner, &mut state, index);
                        let entry = state.queue.iter().find(|entry| entry.index == index).expect("selected pending owner").clone();
                        Some(Action::Write(entry, batch))
                    }
                    action => action,
                };
                if matches!(selected, Some(Action::Prune(_))) {
                    state.pruning_objects = true;
                    state.objects_prune_owed = false;
                }
                (selected, failure_id, attempts, retry_generation)
            };
            if let Some(action) = action {
                if let Action::Write(entry, _) = &action {
                    prefer_state = entry.operation.domain() != Domain::Recovery;
                }
                prefer_pruning = !matches!(action, Action::Prune(_));
                prefer_control = !matches!(action, Action::Snapshot(_) | Action::File(_));
                if action.independent() {
                    files_running += 1;
                } else {
                    ordered_running = true;
                }
                let owner = Arc::clone(inner);
                active.push(Box::pin(async move {
                    let mut action = action;
                    let result = perform(&owner, &action).await;
                    // Reservation Destructors Must Run Outside The State Mutex.
                    if let Action::Write(_, batch) = &mut action {
                        batch.take();
                    }
                    (action, attempts, failure_id, generation, result)
                }));
                continue;
            }
            let retry = failures.values().filter_map(|failure| failure.retry_at).filter(|deadline| *deadline > now).min();
            let deadline = retry.map_or(next_flush, |retry| retry.min(next_flush));
            let deadline = inner.state.lock().unwrap().file_deadline().map_or(deadline, |file| file.min(deadline));
            let mut sleep = Box::pin(inner.execution.sleep(deadline.saturating_duration_since(inner.execution.now())));
            let mut wake = Box::pin(changed.changed());
            let outcome = std::future::poll_fn(|cx| {
                if let Poll::Ready(Some(outcome)) = active.poll_next_unpin(cx) {
                    return Poll::Ready(Some(outcome));
                }
                if wake.as_mut().poll(cx).is_ready() || sleep.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(None);
                }
                Poll::Pending
            })
            .await;
            let Some((action, attempts, failure_id, generation, result)) = outcome else {
                continue;
            };
            if action.independent() {
                files_running -= 1;
            } else {
                ordered_running = false;
            }
            if matches!(action, Action::Prune(_)) {
                inner.state.lock().unwrap().pruning_objects = false;
            }
            if let Ok(Completed::Opened(id, snapshot)) = result {
                let mut state = inner.state.lock().unwrap();
                state.snapshot_opened(id, snapshot);
                if let Some(id) = failure_id {
                    failures.remove(&id);
                }
                state.status.failure = failures.values().next().map(|failure| Arc::clone(&failure.source));
                drop(state);
                inner.notify();
                continue;
            }
            match result {
                Ok(completed) => {
                    if let Some(id) = failure_id {
                        failures.remove(&id);
                    }
                    let mut retired_snapshot = None;
                    let mut state = inner.state.lock().unwrap();
                    // A Peer Completion Cannot Clear Another Action's Failure.
                    state.status.failure = failures.values().next().map(|failure| Arc::clone(&failure.source));
                    match (action, completed) {
                        (Action::Delete(work), Completed::Deleted) => {
                            state.receipts.complete(&work.receipt().mutation, work.receipt().value.clone());
                            state.deletions.remove(&work.receipt().mutation);
                        }
                        (Action::Prune(through), Completed::Pruned(done)) => {
                            if done && !state.objects_prune_owed {
                                state.objects_pruned = through.sequence;
                            }
                        }
                        (Action::File(action), Completed::File(result)) => {
                            state.file_completed(action, result, super::config::FILE_SESSION_TIMEOUT, inner)
                        }
                        (Action::Snapshot(SnapshotAction::Release(_, _, memory)), Completed::Released(id)) => {
                            state.snapshot_removed(id);
                            retired_snapshot = Some(memory);
                        }
                        (Action::Write(_, _), Completed::Write { data, saved, recovery }) => {
                            if let Some(target) = data {
                                let saved = saved.expect("data write returns durability");
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
                            if let Some(target) = recovery {
                                state.recovery_written.index = target.index;
                                state.recovery_written.sequence = state.recovery_written.sequence.max(target.sequence);
                            }
                        }
                        (Action::Recovery(target), Completed::Recovery(saved)) => {
                            if let Some(progress) = &state.journal_saved
                                && let Some(frontier) =
                                    state.queue.iter().filter(|entry| entry.index <= target.index).filter_map(|entry| entry.journal).max()
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
                    drop(state);
                    drop(retired_snapshot);
                }
                Err(error) => {
                    let attempts = if generation == retry_generation { attempts.saturating_add(1) } else { 1 };
                    let retries = match &action {
                        Action::Delete(_) => 0,
                        Action::Write(pending, _)
                            if pending.operation.domain() == Domain::Data && inner.state.lock().unwrap().deleting_data(pending.index) =>
                        {
                            0
                        }
                        Action::File(file) if file.caller_retries(&inner.state.lock().unwrap()) => 0,
                        _ => super::config::MAX_RETRIES,
                    };
                    let retry_at = if transient(&error) && attempts <= retries {
                        let multiplier = 1u32.checked_shl((attempts - 1).min(31) as u32).unwrap_or(u32::MAX);
                        Some(inner.execution.now() + super::config::RETRY_BASE.saturating_mul(multiplier).min(super::config::RETRY_CAP))
                    } else {
                        None
                    };
                    let source = Arc::new(error);
                    let mut state = inner.state.lock().unwrap();
                    if let Action::File(file) = &action {
                        state.file_failed(file, Arc::clone(&source), retry_at.is_some());
                    }
                    match &action {
                        Action::Delete(work) => state.deletion_failed(&work.receipt().mutation, Arc::clone(&source)),
                        Action::Write(pending, _) if pending.operation.domain() == Domain::Data => {
                            state.deletion_data_failed(pending.index, Arc::clone(&source));
                        }
                        _ => {}
                    }
                    let id = match failure_id.filter(|id| failures.contains_key(id)) {
                        Some(id) => id,
                        None => {
                            while failures.contains_key(&next_failure) {
                                next_failure = next_failure.wrapping_add(1);
                            }
                            let id = next_failure;
                            next_failure = next_failure.wrapping_add(1);
                            id
                        }
                    };
                    failures.insert(id, Failed { action: Some(action), source, attempts, retry_at });
                    state.status.failure = failures.values().next().map(|failure| Arc::clone(&failure.source));
                    state.collect();
                }
            }
            inner.notify();
        }
    }
}

impl Action {
    fn independent(&self) -> bool {
        matches!(self, Self::File(file) if file.independent())
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
        fn sum<T: serde::Serialize>(values: &[T]) -> Result<usize> {
            values.iter().try_fold(0usize, |bytes, value| {
                bytes
                    .checked_add(super::manager::encoded_size(value)?)
                    .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded batch size overflow" })
            })
        }
        match self {
            Self::Schema(values) => values.as_slice().iter().try_fold(0usize, |bytes, value| {
                bytes
                    .checked_add(value.encoded_bytes()?)
                    .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded schema size overflow" })
            }),
            Self::ObjectGroup { schemas, objects, .. } => {
                for object in objects.as_slice() {
                    object.version.check_position(&object.position)?;
                }
                let schema_bytes = schemas.as_slice().iter().try_fold(0usize, |bytes, value| {
                    bytes
                        .checked_add(value.encoded_bytes()?)
                        .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded schema size overflow" })
                })?;
                schema_bytes
                    .checked_add(sum(objects.as_slice())?)
                    .ok_or(Error::InvalidInput { field: "storage admission", reason: "encoded batch size overflow" })
            }
            Self::Records(values) => sum(values.as_slice()),
            Self::Stream(values) => sum(values.as_slice()),
            Self::File(values) => sum(values.as_slice()),
            Self::Recovery(_) | Self::Snapshot(_) => Ok(0),
        }
    }

    pub(super) fn check_entry_bytes(&self, maximum: std::num::NonZeroUsize) -> Result<()> {
        if self.encoded_batch_bytes()? > maximum.get() {
            return Err(Error::InvalidInput {
                field: "storage admission",
                reason: "admitted batch exceeds max_batch_bytes; split before submission",
            });
        }
        Ok(())
    }

    pub(super) fn required_data(&self) -> Option<&super::StorageCursor> {
        match self {
            Self::Recovery(entries) => {
                entries.as_slice().iter().max_by_key(|entry| entry.required_data.sequence).map(|entry| &entry.required_data)
            }
            Self::Snapshot(snapshot) => Some(&snapshot.as_slice()[0].required_data),
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
            Self::Schema(schema) => schema.as_slice().last().unwrap().position.sequence,
            Self::File(file) => file.as_slice().last().unwrap().position.sequence,
            Self::ObjectGroup { objects, .. } => objects.as_slice().last().unwrap().position.sequence,
            Self::Records(records) => records.as_slice().last().unwrap().position.sequence,
            Self::Stream(rows) => rows.as_slice().last().unwrap().record.position.sequence,
            Self::Recovery(entries) => entries.as_slice().last().unwrap().position.sequence,
            Self::Snapshot(snapshot) => snapshot.as_slice()[0].through.sequence,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn perform(inner: &Arc<Inner>, action: &Action) -> Result<Completed> {
    match action {
        Action::Delete(work) => work.perform(inner).await.map(|()| Completed::Deleted),
        Action::Prune(through) => inner
            .bounded(inner.primary.prune_objects(through, super::config::OBJECT_PRUNE_ENTRIES, inner.object_retention))
            .await
            .map(Completed::Pruned),
        Action::File(action) => super::staging::perform(inner, action).await.map(Completed::File),
        Action::Snapshot(SnapshotAction::Open(id, at, memory)) => {
            let opened = async {
                let snapshot = inner.bounded(inner.primary.open_snapshot(at, memory.clone())).await?;
                let bytes = snapshot
                    .token
                    .capacity()
                    .checked_mul(3)
                    .and_then(|bytes| bytes.checked_add(super::manager::ENTRY_OVERHEAD))
                    .ok_or(Error::ResourceExhausted { resource: "snapshot token memory" })?;
                memory.grow_to(bytes).await.map_err(crate::capacity::Refusal::into_error)?;
                Ok(snapshot)
            }
            .await;
            Ok(Completed::Opened(*id, opened))
        }
        Action::Snapshot(SnapshotAction::Release(id, snapshot, _)) => {
            if let Some(snapshot) = snapshot {
                inner.bounded(inner.primary.release_snapshot(snapshot)).await?;
            }
            Ok(Completed::Released(*id))
        }
        Action::Write(entry, batch) => {
            let operation = batch.as_ref().map_or(&entry.operation, |batch| &batch.operation);
            let target = batch.as_ref().map_or(Target { index: entry.index, sequence: operation.sequence() }, |batch| batch.target);
            match operation {
                Operation::Recovery(entries) => {
                    let saved = inner.state.lock().unwrap().status.saved.clone();
                    for entry in entries.as_slice() {
                        if !saved.covers(&entry.required_data)? {
                            return Err(Error::InvalidInput {
                                field: "state evidence dependency",
                                reason: "journal cannot precede required primary persistence",
                            });
                        }
                    }
                    inner.bounded(inner.recovery.write_entries(entries.clone())).await?;
                    Ok(Completed::Write { data: None, saved: None, recovery: Some(target) })
                }
                Operation::Snapshot(snapshot) => {
                    if snapshot.len() != 1 {
                        return Err(Error::InvalidInput {
                            field: "state snapshot",
                            reason: "snapshot write requires exactly one snapshot",
                        });
                    }
                    if !inner.state.lock().unwrap().status.saved.covers(&snapshot.as_slice()[0].required_data)? {
                        return Err(Error::InvalidInput {
                            field: "state snapshot dependency",
                            reason: "snapshot cannot precede required primary persistence",
                        });
                    }
                    inner.bounded(inner.recovery.write_snapshot(snapshot.clone())).await?;
                    Ok(Completed::Write { data: None, saved: None, recovery: Some(target) })
                }
                _ => {
                    let saved = write_data(inner, operation).await?;
                    Ok(Completed::Write { data: Some(target), saved: Some(saved), recovery: None })
                }
            }
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
