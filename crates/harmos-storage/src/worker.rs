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

type Attempt =
    Pin<Box<dyn Future<Output = (Action, usize, Option<u64>, u64, Option<crate::execution::Context>, Result<Completed>)> + Send>>;

/* === Structs === */

pub struct Worker(pub(crate) StorageManager);

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
    Definition(Arc<super::definitions::DefinitionWork>),
    Receipts(crate::StorageBatch<crate::MutationReceipt>),
    Recovery(Target),
    Prune(super::StorageCursor),
}

enum Completed {
    Write { data: Option<Target>, saved: Option<super::StorageCursor>, recovery: Option<Target> },
    Opened(u64, Result<super::BackendSnapshot>),
    Released(u64),
    File(FileCompleted),
    Deleted,
    Definition(crate::MutationReceipt),
    Receipts(crate::StorageBatch<crate::MutationReceipt>),
    Recovery(RecoveryPosition),
    Pruned(bool),
}

/* === Traits === */

/* === Implementations === */

impl Worker {
    #[tracing::instrument(name = "storage.worker.cleanup", level = "debug", skip_all)]
    pub async fn cleanup(&mut self) -> Result<()> {
        // StorageManager owns physical backend teardown before this worker's
        // lease retires. An early stop cannot claim that teardown completed.
        if !self.0.inner.state.lock().unwrap().status.closed {
            return Err(Error::NotReady { domain: "storage cleanup" });
        }
        Ok(())
    }

    #[tracing::instrument(name = "storage.worker.run", level = "debug", skip_all)]
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
        let mut snapshot_running = false;
        let mut snapshot_blocked = false;
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
                state.request_definitions();
                state.collect();
                if state.queue.bytes() >= inner.settings.flush_trigger_bytes() {
                    state.request_flush();
                }
                let retry = failures.iter().find_map(|(&id, failure)| {
                    let action = failure.action.as_ref()?;
                    let available = match action {
                        Action::Snapshot(_) => !snapshot_running && !snapshot_blocked,
                        Action::Prune(_) | Action::Delete(_) => !ordered_running && !snapshot_running,
                        _ if action.independent() => files_running < file_limit,
                        _ => !ordered_running,
                    };
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
                        && !snapshot_running
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
                    } else if !snapshot_running && let Some(deletion) = state.deletion_action() {
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
                        // Only the Earliest Unwritten State Entry May Bypass Data,
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
                    let definition = (!ordered_running && !snapshot_running && state.status.failure.is_none())
                        .then(|| state.definition_action())
                        .flatten()
                        .map(Action::Definition);
                    let receipts = if !ordered_running && state.ready && state.status.failure.is_none() {
                        let pending = state.receipts.pending_positions(&state.status.saved);
                        (!pending.is_empty()).then(|| Action::Receipts(crate::StorageBatch::new(pending)))
                    } else {
                        None
                    };
                    let control = definition.or(receipts);
                    let data = if prefer_control { control.or(data) } else { data.or(control) };
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
                            .or_else(|| {
                                if !snapshot_running && !snapshot_blocked {
                                    state.snapshot_action(healthy).map(Action::Snapshot)
                                } else {
                                    None
                                }
                            })
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
                        let building = std::time::Instant::now();
                        {
                            let super::manager::State { queue, persistence, .. } = &mut *state;
                            queue
                                .iter()
                                .find(|entry| entry.index == index)
                                .expect("selected pending owner")
                                .measure_selection(persistence, building);
                        }
                        let batch = Batch::prepare(inner, &mut state, index);
                        state.persistence.batch_build.record(building.elapsed());
                        if let Some(batch) = &batch {
                            batch.measure_selection(&mut state.persistence, building);
                        }
                        let entry = state.queue.iter().find(|entry| entry.index == index).expect("selected pending owner").clone();
                        Some(Action::Write(entry, batch))
                    }
                    action => action,
                };
                if matches!(selected, Some(Action::Receipts(_))) {
                    state.persisting_receipts = true;
                }
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
                prefer_control = !matches!(action, Action::Snapshot(_) | Action::File(_) | Action::Definition(_) | Action::Receipts(_));
                if matches!(action, Action::Snapshot(_)) {
                    snapshot_running = true;
                } else if action.independent() {
                    files_running += 1;
                } else {
                    ordered_running = true;
                    snapshot_blocked = matches!(action, Action::Prune(_) | Action::Delete(_));
                }
                let (context, queued) = {
                    let mut state = inner.state.lock().unwrap();
                    match &action {
                        Action::Write(entry, _) => (Some(entry.context.clone()), None),
                        Action::Snapshot(SnapshotAction::Open(id, _) | SnapshotAction::Release(id, _)) => {
                            state.snapshots.get_mut(id).map(|held| (Some(held.context.clone()), held.queued.take())).unwrap_or((None, None))
                        }
                        Action::File(file) => state
                            .files
                            .get_mut(&file.id())
                            .map(|held| (Some(held.context.clone()), held.queued.take()))
                            .unwrap_or((None, None)),
                        Action::Delete(work) => (Some(work.context.clone()), work.queued.lock().unwrap().take()),
                        Action::Definition(work) => (Some(work.context.clone()), work.queued.lock().unwrap().take()),
                        _ => (None, None),
                    }
                };
                let owner = Arc::clone(inner);
                active.push(Box::pin(async move {
                    let mut action = action;
                    let future = async {
                        let span = tracing::debug_span!("storage.work", outcome = "cancelled");
                        if let Some(queued) = queued {
                            span.follows_from(queued.id());
                        }
                        let physical = async {
                            let result = perform(&owner, &action).await;
                            tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
                            result
                        };
                        tracing::Instrument::instrument(physical, span).await
                    };
                    let result = match &context {
                        Some(context) => context.scope(future).await,
                        None => future.await,
                    };
                    // Release Completed Work Outside The State Mutex.
                    if let Action::Write(_, batch) = &mut action {
                        batch.take();
                    }
                    (action, attempts, failure_id, generation, context, result)
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
            let Some((action, attempts, failure_id, generation, context, result)) = outcome else {
                continue;
            };
            if matches!(action, Action::Snapshot(_)) {
                snapshot_running = false;
            } else if action.independent() {
                files_running -= 1;
            } else {
                ordered_running = false;
                snapshot_blocked = false;
            }
            if matches!(action, Action::Receipts(_)) {
                inner.state.lock().unwrap().persisting_receipts = false;
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
                    let mut state = inner.state.lock().unwrap();
                    // A Peer Completion Cannot Clear Another Action's Failure.
                    state.status.failure = failures.values().next().map(|failure| Arc::clone(&failure.source));
                    match (action, completed) {
                        (Action::Receipts(_), Completed::Receipts(receipts)) => {
                            state.receipts.saved(receipts.as_slice());
                        }
                        (Action::Definition(work), Completed::Definition(receipt)) => {
                            state.definition_completed(&work, receipt)?;
                        }
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
                            let publish = || {
                                let _publication = tracing::debug_span!("storage.file.publish").entered();
                                state.file_completed(action, result, inner.settings.policy.file_session_timeout, inner)
                            };
                            match &context {
                                Some(context) => context.sync(publish),
                                None => publish(),
                            }
                        }
                        (Action::Snapshot(SnapshotAction::Release(_, _)), Completed::Released(id)) => {
                            state.snapshot_removed(id);
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
                }
                Err(error) => {
                    let attempts = if generation == retry_generation { attempts.saturating_add(1) } else { 1 };
                    let retry_policy = match &action {
                        Action::Delete(_) => crate::config::RetryPolicy::Never,
                        Action::Write(pending, _)
                            if pending.operation.domain() == Domain::Data && inner.state.lock().unwrap().deleting_data(pending.index) =>
                        {
                            crate::config::RetryPolicy::Never
                        }
                        Action::File(file) if file.caller_retries(&inner.state.lock().unwrap()) => crate::config::RetryPolicy::Never,
                        _ => inner.settings.policy.retries,
                    };
                    let retry_at = retry_policy
                        .retry_after(
                            attempts.saturating_sub(1),
                            if transient(&error) {
                                harmos_protocol::policy::FailureKind::Transient
                            } else {
                                harmos_protocol::policy::FailureKind::Permanent
                            },
                            harmos_protocol::policy::ReplaySafety::Idempotent,
                            generation ^ failure_id.unwrap_or(next_failure),
                        )
                        .map(|delay| inner.execution.now() + delay);
                    let source = Arc::new(error);
                    let mut state = inner.state.lock().unwrap();
                    if let Action::File(file) = &action {
                        state.file_failed(file, Arc::clone(&source), retry_at.is_some());
                    }
                    match &action {
                        Action::Definition(work) => state.definition_failed(&work.header.mutation, Arc::clone(&source), retry_at.is_some()),
                        Action::Delete(work) => state.deletion_failed(&work.receipt().mutation, Arc::clone(&source)),
                        Action::Write(pending, _) if pending.operation.domain() == Domain::Data => {
                            state.deletion_data_failed(pending.index, Arc::clone(&source));
                            state.definition_data_failed(pending.index, Arc::clone(&source));
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
                    failures.insert(
                        id,
                        Failed {
                            action: Some(action),
                            source,
                            attempts,
                            retry_at,
                        },
                    );
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
        state.persisting_receipts = false;
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
                bytes.checked_add(super::manager::encoded_size(value)?).ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "encoded batch size overflow",
                })
            })
        }
        match self {
            Self::Conditional(exchange) => {
                exchange.validate()?;
                exchange.schema.encoded_bytes()?;
                super::manager::encoded_size(exchange.as_ref())
            }
            Self::SetSchema(exchange) => Ok(exchange.value.encoded_bytes()?),
            Self::Schema(values) => values.as_slice().iter().try_fold(0usize, |bytes, value| {
                bytes.checked_add(value.encoded_bytes()?).ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "encoded schema size overflow",
                })
            }),
            Self::ObjectGroup { schemas, objects, .. } => {
                for object in objects.as_slice() {
                    object.version.check_position(&object.position)?;
                }
                let schema_bytes = schemas.as_slice().iter().try_fold(0usize, |bytes, value| {
                    bytes.checked_add(value.encoded_bytes()?).ok_or(Error::InvalidInput {
                        field: "storage admission",
                        reason: "encoded schema size overflow",
                    })
                })?;
                schema_bytes.checked_add(sum(objects.as_slice())?).ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "encoded batch size overflow",
                })
            }
            Self::Records(values) => sum(values.as_slice()),
            Self::Stream(values) => sum(values.as_slice()),
            Self::File(values) => sum(values.as_slice()),
            Self::Recovery(_) | Self::Snapshot(_) => Ok(0),
        }
    }

    pub(super) fn check_entry_bytes(&self, maximum: std::num::NonZeroUsize) -> Result<usize> {
        let bytes = self.encoded_batch_bytes()?;
        if bytes > maximum.get() {
            return Err(Error::InvalidInput {
                field: "storage admission",
                reason: "admitted batch exceeds max_batch_bytes; split before submission",
            });
        }
        Ok(bytes)
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
            Self::Conditional(exchange) => exchange.value.position.sequence,
            Self::SetSchema(exchange) => exchange.value.position.sequence,
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

#[tracing::instrument(name = "storage.worker.perform", level = "debug", skip_all)]
async fn perform(inner: &Arc<Inner>, action: &Action) -> Result<Completed> {
    match action {
        Action::Delete(work) => work.perform(inner).await.map(|()| Completed::Deleted),
        Action::Definition(work) => work.perform(inner).await.map(Completed::Definition),
        Action::Receipts(receipts) => {
            inner.bounded("primary.append_mutation_receipts", inner.primary.append_mutation_receipts(receipts.clone())).await?;
            Ok(Completed::Receipts(receipts.clone()))
        }
        Action::Prune(through) => inner
            .bounded(
                "primary.delete_expired_object_revisions",
                inner.primary.delete_expired_object_revisions(through, super::config::OBJECT_PRUNE_ENTRIES, &inner.catalog),
            )
            .await
            .map(Completed::Pruned),
        Action::File(action) => super::staging::perform(inner, action).await.map(Completed::File),
        Action::Snapshot(SnapshotAction::Open(id, at)) => {
            let opened = inner.bounded("primary.create_snapshot", inner.primary.create_snapshot(at)).await;
            Ok(Completed::Opened(*id, opened))
        }
        Action::Snapshot(SnapshotAction::Release(id, snapshot)) => {
            if let Some(snapshot) = snapshot {
                inner.bounded("primary.delete_snapshot", inner.primary.delete_snapshot(snapshot)).await?;
            }
            Ok(Completed::Released(*id))
        }
        Action::Write(entry, batch) => {
            let future = async {
                let operation = batch.as_ref().map_or(&entry.operation, |batch| &batch.operation);
                let target = batch.as_ref().map_or(
                    Target {
                        index: entry.index,
                        sequence: operation.sequence(),
                    },
                    |batch| batch.target,
                );
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
                        inner.bounded("recovery.append_recovery_entries", inner.recovery.append_recovery_entries(entries.clone())).await?;
                        Ok(Completed::Write {
                            data: None,
                            saved: None,
                            recovery: Some(target),
                        })
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
                        inner.bounded("recovery.set_recovery_snapshot", inner.recovery.set_recovery_snapshot(snapshot.clone())).await?;
                        Ok(Completed::Write {
                            data: None,
                            saved: None,
                            recovery: Some(target),
                        })
                    }
                    _ => {
                        let encoded_bound = batch.as_ref().map_or(entry.encoded_bound, |batch| batch.encoded_bound);
                        let saved = write_data(inner, operation, encoded_bound).await?;
                        Ok(Completed::Write {
                            data: Some(target),
                            saved: Some(saved),
                            recovery: None,
                        })
                    }
                }
            };
            if let Some(batch) = batch {
                batch.trace(inner, future).await
            } else {
                if let Some(queued) = entry.queued.lock().unwrap().take() {
                    tracing::Span::current().follows_from(queued.id());
                }
                future.await
            }
        }

        Action::Recovery(target) => {
            let through = RecoveryPosition {
                history: inner.recovery.get_history().clone(),
                sequence: target.sequence,
            };
            let saved = inner.bounded("recovery.flush", inner.recovery.flush(&through)).await?;
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
