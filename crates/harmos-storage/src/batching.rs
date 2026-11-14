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
    pub(super) encoded_bound: usize,
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
    pub(super) fn measure_selection(&self, work: &mut crate::PersistenceWork, now: std::time::Instant) {
        for entry in &self.entries {
            entry.measure_selection(work, now);
        }
    }
    #[tracing::instrument(name = "storage.batching.write", parent = None, level = "trace", skip_all, fields(entries = self.entries.len(), store = %inner.primary.get_history().store, epoch = %inner.primary.get_history().epoch, domain = if self.entries[0].operation.domain() == Domain::Data { "data" } else { "recovery" }, first_index = self.entries[0].index, last_index = self.entries[self.entries.len() - 1].index, through = self.target.sequence, outcome = "cancelled"))]
    pub(super) async fn trace<T>(&self, inner: &Inner, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        let mut contributions = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            contributions.push(entry.context.sync(|| {
                let span = tracing::trace_span!("storage.batch.contribution",
                    store = %inner.primary.get_history().store, epoch = %inner.primary.get_history().epoch,
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

        let mut indices = Vec::new();
        let mut rows = 0usize;
        let mut encoded = 0usize;
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
            // A Waiting Small Prefix Must Not Join A Later Large Admission.
            if !indices.is_empty() && entry.encoded_bound >= inner.settings.flush_trigger_bytes() {
                break;
            }
            let count = match &entry.operation {
                Operation::Records(values) => values.len(),
                Operation::Stream(values) => values.len(),
                Operation::Recovery(values) => values.len(),
                _ => unreachable!("coalescing selects one homogeneous append kind"),
            };
            let next_rows = rows.checked_add(count)?;
            let next_encoded = encoded.checked_add(entry.encoded_bound)?;
            if next_rows > 4096 || next_encoded > inner.settings.max_batch_bytes().get() {
                break;
            }
            rows = next_rows;
            encoded = next_encoded;
            indices.push(entry.index);
            target = Target {
                index: entry.index,
                sequence: entry.operation.sequence(),
            };
            if entry.encoded_bound >= inner.settings.flush_trigger_bytes() {
                break;
            }
        }
        if indices.len() < 2 {
            return None;
        }
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
        let operation = operation?;
        let entries = state.queue.iter().filter(|entry| indices.binary_search(&entry.index).is_ok()).cloned().collect();
        Some(Self {
            operation,
            entries,
            target,
            encoded_bound: encoded,
        })
    }
}

/* === Public Functions === */

#[tracing::instrument(name = "storage.batching.write_data", level = "trace", skip_all, fields(through = tracing::field::Empty, saved = tracing::field::Empty))]
pub(super) async fn write_data(inner: &Arc<Inner>, operation: &Operation, encoded_bound: usize) -> Result<StorageCursor> {
    match operation {
        Operation::Conditional(exchange) => {
            let through = &exchange.value.position;
            {
                let mut state = inner.state.lock().unwrap();
                state.data_attempted = state.data_attempted.max(through.sequence);
            }
            inner.bounded("primary.set_object", inner.primary.set_object(exchange)).await?;
            let saved = inner.bounded("primary.get_saved_position", inner.primary.get_saved_position()).await?;
            let mut state = inner.state.lock().unwrap();
            if !saved.covers(through)? || saved.sequence > state.data_attempted {
                return Err(invalid("conditional outcome does not cover its admitted frontier"));
            }
            state.status.saved = saved.clone();
            Ok(saved)
        }
        Operation::SetSchema(exchange) => {
            let through = &exchange.value.position;
            inner.state.lock().unwrap().data_attempted = through.sequence;
            let result =
                inner.bounded("primary.append_schemas", inner.primary.append_schemas(crate::SchemaWrite::Set(exchange.clone()))).await?;
            let receipt = result.mutation.ok_or_else(|| invalid("SetSchema omitted its atomic receipt"))?;
            exchange.replay(&receipt)?;
            let mut state = inner.state.lock().unwrap();
            if !result.saved.covers(through)? || result.saved.sequence > state.data_attempted {
                return Err(invalid("SetSchema receipt does not cover its admitted frontier"));
            }
            state.receipts.complete(&receipt.mutation, receipt.value.clone());
            state.receipts.saved(&[receipt]);
            state.status.saved = result.saved.clone();
            Ok(result.saved)
        }
        Operation::Schema(values) => {
            persist(inner, values, encoded_bound, |batch| {
                Box::pin(async move { inner.primary.append_schemas(batch.into()).await.map(|result| result.saved) })
            })
            .await
        }
        Operation::File(values) => persist(inner, values, encoded_bound, |batch| inner.primary.append_file_references(batch)).await,
        Operation::Records(values) => persist(inner, values, encoded_bound, |batch| inner.primary.append_records(batch)).await,
        Operation::Stream(values) => persist(inner, values, encoded_bound, |batch| inner.primary.append_samples(batch)).await,
        Operation::ObjectGroup { schemas, objects, .. } => {
            for index in 0..objects.len() {
                persist(inner, &schemas.slice(index..index + 1), super::manager::encoded_size(&schemas.as_slice()[index])?, |batch| {
                    Box::pin(async move { inner.primary.append_schemas(batch.into()).await.map(|result| result.saved) })
                })
                .await?;
                persist(inner, &objects.slice(index..index + 1), super::manager::encoded_size(&objects.as_slice()[index])?, |batch| {
                    inner.primary.append_object_revisions(batch)
                })
                .await?;
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
    inner: &'a Arc<Inner>, values: &StorageBatch<T>, encoded_bound: usize,
    mut write: impl FnMut(StorageBatch<T>) -> super::StorageFuture<'a, StorageCursor>,
) -> Result<StorageCursor> {
    let preparing = std::time::Instant::now();
    let first = values.as_slice().first().ok_or_else(|| invalid("typed batch is empty"))?.position();
    let through = values.as_slice().last().expect("nonempty batch").position();
    tracing::Span::current().record("through", through.sequence);
    inner.primary.get_history().check(&first.history)?;
    let mut previous = first.sequence;
    for value in &values.as_slice()[1..] {
        first.history.check(&value.position().history)?;
        if previous.checked_add(1) != Some(value.position().sequence) {
            return Err(invalid("typed batch positions are not contiguous"));
        }
        previous = value.position().sequence;
    }
    let start = {
        let state = inner.state.lock().unwrap();
        let saved = &state.status.saved;
        if saved.covers(through)? {
            return Ok(saved.clone());
        }
        let start = values.as_slice().partition_point(|value| value.position().sequence <= saved.sequence);
        if values.as_slice()[start].position().sequence != saved.sequence.checked_add(1).ok_or_else(|| invalid("position overflow"))? {
            return Err(invalid("typed write would skip the durable frontier"));
        }
        start
    };
    inner.state.lock().unwrap().persistence.batch_build.record(preparing.elapsed());
    let mut offset = start;
    let mut saved = inner.state.lock().unwrap().status.saved.clone();
    while offset < values.len() {
        // Ordinary Admissions Reuse Their Checked Bound. Larger State Captures Split.
        let preparing = std::time::Instant::now();
        let (end, bytes) = if offset == 0 && encoded_bound <= inner.settings.max_batch_bytes().get() {
            (values.len(), encoded_bound)
        } else {
            let mut end = offset;
            let mut bytes = 0usize;
            for value in &values.as_slice()[offset..] {
                let size = super::manager::encoded_size(value)?;
                if size > inner.settings.max_batch_bytes().get() {
                    return Err(invalid("entry exceeds the configured commit byte ceiling"));
                }
                if end > offset && size > inner.settings.max_batch_bytes().get().saturating_sub(bytes) {
                    break;
                }
                bytes += size;
                end += 1;
            }
            (end, bytes)
        };
        let through = values.as_slice()[end - 1].position();
        {
            let mut state = inner.state.lock().unwrap();
            state.data_attempted = state.data_attempted.max(through.sequence);
            state.persistence.batch_build.record(preparing.elapsed());
        }
        let writing = std::time::Instant::now();
        saved = inner.bounded("primary.write", write(values.slice(offset..end))).await?;
        let written = writing.elapsed();
        let publishing = std::time::Instant::now();
        tracing::Span::current().record("saved", saved.sequence);
        {
            let mut state = inner.state.lock().unwrap();
            if !saved.covers(through)? || saved.sequence > state.data_attempted {
                return Err(invalid("receipt does not cover the typed write within its attempted frontier"));
            }
            state.status.saved = saved.clone();
            state.persistence.backend.record(written);
            state.persistence.committed_pieces += 1;
            state.persistence.committed_rows += (end - offset) as u64;
            state.persistence.committed_bytes += bytes as u64;
            state.persistence.maximum_commit_bytes = state.persistence.maximum_commit_bytes.max(bytes);
        }
        inner.notify();
        inner.state.lock().unwrap().persistence.publication.record(publishing.elapsed());
        offset = end;
    }
    Ok(saved)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "data write receipt", reason }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::{Backend, RecoveryBackend, history, storage_settings};
    use crate::{PayloadEncoding, SchemaKey, StorageBackend, StorageBackends, StorageManager, StoredRecord};
    use std::{
        num::{NonZeroU32, NonZeroUsize},
        sync::atomic::Ordering,
    };

    #[tokio::test]
    async fn oversized_state_capture_resumes_after_each_bounded_committed_piece() {
        let primary = Arc::new(Backend::new(history()));
        primary.create_session().await.unwrap();
        let mut settings = storage_settings();
        settings.request.max_pending_write_bytes = NonZeroUsize::new(64 * 1024 * 1024).unwrap();
        let ceiling = settings.max_batch_bytes().get();
        let storage = StorageManager::new(
            StorageBackends {
                primary: primary.clone(),
                state: Arc::new(RecoveryBackend::new(history())),
            },
            settings,
        )
        .unwrap();
        let values = StorageBatch::new(
            (1..=4)
                .map(|sequence| StoredRecord {
                    position: storage.position(sequence),
                    schema: SchemaKey {
                        id: crate::Identifier::new("capture").unwrap(),
                        version: NonZeroU32::MIN,
                        payload_encoding: PayloadEncoding::PostcardV1,
                    },
                    scope: None,
                    item: crate::Identifier::new("capture").unwrap(),
                    source: None,
                    source_time: None,
                    payload: vec![90; 3 * 1024 * 1024],
                })
                .collect(),
        );
        let bytes: usize = values.as_slice().iter().map(|value| crate::encoded_size(value).unwrap()).sum();
        assert!(bytes > ceiling);
        primary.control.write_failure_at.store(3, Ordering::SeqCst);
        assert!(persist(&storage.inner, &values, bytes, |batch| primary.append_records(batch)).await.is_err());
        assert_eq!(storage.get_saved_position().sequence, 2);
        let saved = persist(&storage.inner, &values, bytes, |batch| primary.append_records(batch)).await.unwrap();
        assert_eq!(saved.sequence, 4);
        assert_eq!(primary.records.lock().unwrap().len(), 4);
        assert!(primary.control.committed_bytes.lock().unwrap().iter().all(|bytes| *bytes <= ceiling));
        primary.delete_session().await.unwrap();
    }
}
