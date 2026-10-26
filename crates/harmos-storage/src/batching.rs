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
// Third Party Imports
// Local Imports
use super::manager::{Domain, Inner, Operation, Pending, State, Target};
use super::{StorageBatch, StorageCursor};
use crate::capacity::{Pool, Reservation};
use crate::selection::Entry;
use crate::{Error, Result};
use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Batch {
    pub(super) operation: Operation,
    entries: Vec<Arc<Pending>>,
    pub(super) target: Target,
    _scratch: Reservation,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Records,
    Stream,
    Recovery,
}

/* === Traits === */

/* === Implementations === */

impl Batch {
    #[tracing::instrument(name = "storage.batching.write", parent = None, level = "trace", skip_all, fields(entries = self.entries.len(), store = %inner.primary.history().store, epoch = %inner.primary.history().epoch, domain = if self.entries[0].operation.domain() == Domain::Data { "data" } else { "recovery" }, first_index = self.entries[0].index, last_index = self.entries[self.entries.len() - 1].index, through = self.target.sequence, outcome = "cancelled"))]
    pub(super) async fn trace<T>(&self, inner: &Inner, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        let mut contributions = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            contributions.push(entry.context.sync(|| {
                let span = tracing::trace_span!("storage.batch.contribution",
                    store = %inner.primary.history().store, epoch = %inner.primary.history().epoch,
                    domain = if entry.operation.domain() == Domain::Data { "data" } else { "recovery" },
                    request_domain = if entry.operation.domain() == Domain::Data { "data" } else { "recovery" },
                    first_index = self.entries[0].index, last_index = self.entries[self.entries.len() - 1].index,
                    through = self.target.sequence, request_index = entry.index,
                    request_sequence = entry.operation.sequence(), outcome = "cancelled");
                if let Some(queued) = entry.queued.lock().unwrap().take() {
                    span.follows_from(queued.id());
                }
                crate::execution::Context::with_span(span)
            }));
        }
        let result = future.await;
        let outcome = if result.is_ok() { "completed" } else { "failed" };
        tracing::Span::current().record("outcome", outcome);
        for contribution in &contributions {
            contribution.sync(|| {
                contribution.span.record("outcome", outcome);
            });
        }
        result
    }

    pub(super) fn prepare(inner: &Arc<Inner>, state: &mut State, first: u64) -> Option<Self> {
        let entry = state.queue.iter().find(|entry| entry.index == first)?;
        let selected_kind = kind(&entry.operation)?;
        let domain = entry.operation.domain();
        let boundary = if domain == Domain::Recovery { state.recovery_request?.index } else { state.write_boundary()? };
        let available = inner.settings.request.max_pending_write_bytes.get().saturating_sub(state.status.pending_bytes);
        let mut indices = Vec::new();
        let mut rows = 0usize;
        let mut encoded = 0usize;
        let mut scratch = 0usize;
        let mut target = Target::default();
        for entry in state.queue.iter().filter(|entry| entry.index >= first && state.needs_write(entry)) {
            if entry.index > boundary {
                break;
            }
            if entry.operation.domain() != domain {
                if domain == Domain::Data {
                    continue;
                }
                break;
            }
            if kind(&entry.operation) != Some(selected_kind) {
                break;
            }
            if domain == Domain::Recovery
                && entry.operation.required_data().is_some_and(|required| !state.status.saved.covers(required).unwrap_or(false))
            {
                break;
            }
            let (count, inline) = match &entry.operation {
                Operation::Records(values) => (values.len(), std::mem::size_of::<crate::StoredRecord>()),
                Operation::Stream(values) => (values.len(), std::mem::size_of::<crate::StoredRow>()),
                Operation::Recovery(values) => (values.len(), std::mem::size_of::<crate::state::RecoveryEntry>()),
                _ => unreachable!("coalescing selects one homogeneous append kind"),
            };
            let next_rows = rows.checked_add(count)?;
            let next_encoded = encoded.checked_add(entry.operation.encoded_batch_bytes().ok()?)?;
            let next_scratch = scratch
                .checked_add(count.checked_mul(inline)?)?
                .checked_add(128 + std::mem::size_of::<Arc<Pending>>() + std::mem::size_of::<crate::execution::Context>())?;
            if next_rows > 4096 || next_encoded > inner.settings.max_batch_bytes().get() || next_scratch > available {
                break;
            }
            rows = next_rows;
            encoded = next_encoded;
            scratch = next_scratch;
            indices.push(entry.index);
            target = Target { index: entry.index, sequence: entry.operation.sequence() };
        }
        if indices.len() < 2 {
            return None;
        }
        // Reserve The Temporary Inline Array Before Moving Any Payload Owner.
        state.scratch_bytes += scratch;
        state.collect();
        let operation = match selected_kind {
            Kind::Records => merge(state, &indices, |operation| match operation {
                Operation::Records(values) => values,
                _ => unreachable!("selected record kind"),
            })
            .map(Operation::Records),
            Kind::Stream => merge(state, &indices, |operation| match operation {
                Operation::Stream(values) => values,
                _ => unreachable!("selected stream kind"),
            })
            .map(Operation::Stream),
            Kind::Recovery => merge(state, &indices, |operation| match operation {
                Operation::Recovery(values) => values,
                _ => unreachable!("selected recovery kind"),
            })
            .map(Operation::Recovery),
        };
        let Some(operation) = operation else {
            state.scratch_bytes -= scratch;
            state.collect();
            return None;
        };
        let entries = state.queue.iter().filter(|entry| indices.binary_search(&entry.index).is_ok()).cloned().collect();
        Some(Self { operation, entries, target, _scratch: Reservation::charged(inner, Pool::Write, scratch) })
    }
}

/* === Public Functions === */

#[tracing::instrument(name = "storage.batching.write_data", level = "trace", skip_all, fields(through = tracing::field::Empty, saved = tracing::field::Empty))]
pub(super) async fn write_data(inner: &Inner, operation: &Operation) -> Result<StorageCursor> {
    match operation {
        Operation::Schema(values) => persist(inner, values, |batch| inner.primary.write_schemas(batch)).await,
        Operation::File(values) => persist(inner, values, |batch| inner.primary.write_files(batch)).await,
        Operation::Records(values) => persist(inner, values, |batch| inner.primary.write_records(batch)).await,
        Operation::Stream(values) => persist(inner, values, |batch| inner.primary.write_stream(batch)).await,
        Operation::ObjectGroup { schemas, objects, .. } => {
            for index in 0..objects.len() {
                persist(inner, &schemas.slice(index..index + 1), |batch| inner.primary.write_schemas(batch)).await?;
                persist(inner, &objects.slice(index..index + 1), |batch| inner.primary.write_objects(batch)).await?;
            }
            Ok(inner.state.lock().unwrap().status.saved.clone())
        }
        Operation::Recovery(_) | Operation::Snapshot(_) => unreachable!("primary persistence requires primary data"),
    }
}

/* === Private Functions === */

fn kind(operation: &Operation) -> Option<Kind> {
    match operation {
        Operation::Records(_) => Some(Kind::Records),
        Operation::Stream(_) => Some(Kind::Stream),
        Operation::Recovery(_) => Some(Kind::Recovery),
        _ => None,
    }
}

fn merge<T>(state: &mut State, indices: &[u64], select: fn(&mut Operation) -> &mut StorageBatch<T>) -> Option<StorageBatch<T>> {
    let mut batches = state
        .queue
        .iter_mut()
        .filter(|entry| indices.binary_search(&entry.index).is_ok())
        .map(|entry| Arc::get_mut(entry).map(|entry| select(&mut entry.operation)))
        .collect::<Option<Vec<_>>>()?;
    StorageBatch::merge(&mut batches)
}

async fn persist<'a, T: Entry>(
    inner: &'a Inner, values: &StorageBatch<T>, write: impl FnOnce(StorageBatch<T>) -> super::StorageFuture<'a, StorageCursor>,
) -> Result<StorageCursor> {
    let first = values.as_slice().first().ok_or_else(|| invalid("typed batch is empty"))?.position();
    let through = values.as_slice().last().expect("nonempty batch").position();
    tracing::Span::current().record("through", through.sequence);
    inner.primary.history().check(&first.history)?;
    let mut previous = first.sequence;
    for value in &values.as_slice()[1..] {
        first.history.check(&value.position().history)?;
        if previous.checked_add(1) != Some(value.position().sequence) {
            return Err(invalid("typed batch positions are not contiguous"));
        }
        previous = value.position().sequence;
    }
    let start = {
        let mut state = inner.state.lock().unwrap();
        let saved = &state.status.saved;
        if saved.covers(through)? {
            return Ok(saved.clone());
        }
        let start = values.as_slice().partition_point(|value| value.position().sequence <= saved.sequence);
        if values.as_slice()[start].position().sequence != saved.sequence.checked_add(1).ok_or_else(|| invalid("position overflow"))? {
            return Err(invalid("typed write would skip the durable frontier"));
        }
        state.data_attempted = state.data_attempted.max(through.sequence);
        start
    };
    let saved = inner.bounded("primary.write", write(values.slice(start..values.len()))).await?;
    tracing::Span::current().record("saved", saved.sequence);
    let mut state = inner.state.lock().unwrap();
    if !saved.covers(through)? || saved.sequence > state.data_attempted {
        return Err(invalid("receipt does not cover the typed write within its attempted frontier"));
    }
    // Each Typed Call Is Durable Independently; Retain Its Prefix If A Later Type Fails.
    state.status.saved = saved.clone();
    Ok(saved)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "data write receipt", reason }
}

/* === Tests === */
