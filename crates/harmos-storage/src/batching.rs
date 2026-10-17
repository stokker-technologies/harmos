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
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::manager::{Domain, ENTRY_OVERHEAD, Inner, Operation, Pending, Target, recovery_resident_bytes};
use super::{StorageBatch, StorageCursor, StoragePart};
use crate::storage::capacity::{Pool, Reservation};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct Batch {
    entries: Vec<Arc<Pending>>,
    _reservation: Reservation,
}

#[derive(Default)]
pub(super) struct Progress {
    pub(super) data: Option<Target>,
    pub(super) saved: Option<StorageCursor>,
    pub(super) recovery: Option<Target>,
}

/* === Enums === */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Objects,
    Records,
    Stream,
    Recovery,
}

/* === Traits === */

/* === Implementations === */
impl Progress {
    pub(super) fn include(&mut self, entry: &Pending) {
        let target = Target { index: entry.index, sequence: entry.operation.sequence() };
        match entry.operation.domain() {
            Domain::Data => self.data = Some(target),
            Domain::Recovery => self.recovery = Some(target),
        }
    }
}

impl Batch {
    pub(super) fn prepare(inner: &Arc<Inner>, first: &Pending) -> Option<Self> {
        let kind = kind(&first.operation)?;
        let mut state = inner.state.lock().unwrap();
        // Ready State Writes May Bypass An Earlier Unrelated Primary Barrier.
        let boundary = if kind == Kind::Recovery { state.recovery_request?.index } else { state.write_boundary()? };
        let available = inner
            .settings
            .request
            .max_pending_write_bytes
            .get()
            .saturating_sub(state.status.pending_bytes)
            .min(inner.settings.max_batch_bytes().get());
        let mut bytes = 0usize;
        let mut count = 0usize;
        let mut operations = 0;
        let mut encoded_bytes = 0usize;
        for entry in state.queue.iter().filter(|entry| {
            entry.index >= first.index && state.needs_write(entry) && (kind == Kind::Recovery || entry.operation.domain() == Domain::Data)
        }) {
            if entry.index > boundary {
                break;
            }
            if !compatible(entry, kind) {
                break;
            }
            if kind == Kind::Recovery
                && entry.operation.required_data().is_some_and(|required| !state.status.saved.covers(required).unwrap_or(false))
            {
                break;
            }
            let (length, charge) = charge(&entry.operation)?;
            let next_encoded = encoded_bytes.checked_add(entry.operation.encoded_batch_bytes().ok()?)?;
            if next_encoded > inner.settings.max_batch_bytes().get() {
                break;
            }
            let next_count = count.checked_add(length)?;
            let next_bytes = bytes.checked_add(charge)?.checked_add(std::mem::size_of::<Arc<Pending>>())?;
            if next_count > super::config::WRITE_BATCH_ENTRIES.get() || next_bytes > available {
                break;
            }
            encoded_bytes = next_encoded;
            count = next_count;
            bytes = next_bytes;
            operations += 1;
        }
        if operations < 2 {
            return None;
        }

        // Reserve scratch before cloning payloads. Never await capacity here:
        // the worker must still drain a completely full admission budget.
        state.scratch_bytes += bytes;
        state.status.pending_bytes += bytes;
        let mut entries = Vec::with_capacity(operations);
        entries.extend(
            state
                .queue
                .iter()
                .filter(|entry| {
                    entry.index >= first.index
                        && state.needs_write(entry)
                        && (kind == Kind::Recovery || entry.operation.domain() == Domain::Data)
                })
                .take(operations)
                .cloned(),
        );
        drop(state);
        Some(Self { entries, _reservation: Reservation::charged(inner, Pool::Write, bytes) })
    }

    pub(super) async fn write(&self, inner: &Inner) -> Result<Progress> {
        let mut parts = Vec::new();
        let mut recovery = Vec::new();
        let mut progress = Progress::default();
        for entry in &self.entries {
            match &entry.operation {
                Operation::ObjectGroup { schemas, objects, .. } => {
                    for (schema, object) in schemas.iter().zip(objects) {
                        parts.push(StoragePart::Schemas(std::slice::from_ref(schema)));
                        parts.push(StoragePart::Objects(std::slice::from_ref(object)));
                    }
                }
                Operation::Records(values) => parts.push(StoragePart::Records(values)),
                Operation::Stream(values) => parts.push(StoragePart::Stream(values)),
                Operation::Recovery(values) => recovery.extend_from_slice(values),
                _ => unreachable!("batch contains only append operations"),
            }
            progress.include(entry);
        }

        // Borrow admitted data across the single durable call instead of cloning
        // payloads into another aggregate. Reserved scratch covers span metadata;
        // the separate recovery protocol still receives one contiguous slice.
        if !parts.is_empty() {
            progress.saved = Some(write_data(inner, &parts).await?);
        }
        if !recovery.is_empty() {
            let saved = inner.state.lock().unwrap().status.saved.clone();
            for entry in &recovery {
                if !saved.covers(&entry.required_data)? {
                    return Err(Error::InvalidInput {
                        field: "state evidence dependency",
                        reason: "journal cannot precede required primary persistence",
                    });
                }
            }
            inner.bounded(inner.recovery.write_entries(&recovery)).await?;
        }
        Ok(progress)
    }
}

/* === Public Functions === */
pub(super) async fn write_data(inner: &Inner, parts: &[StoragePart<'_>]) -> Result<StorageCursor> {
    // Logical admissions are indivisible; never persist a prefix of one batch.
    let batch = StorageBatch::new(parts, inner.settings.max_batch_bytes())?;
    inner.primary.history().check(&batch.first().history)?;
    let through = batch.through();
    {
        // Preserve the attempted ceiling on cancellation or uncertain commit.
        // An acknowledgement may not claim data merely admitted to the queue.
        let mut state = inner.state.lock().unwrap();
        state.data_attempted = state.data_attempted.max(through.sequence);
    }
    let saved = inner.bounded(inner.primary.write(batch)).await?;
    let attempted = inner.state.lock().unwrap().data_attempted;
    if !saved.covers(through)? || saved.sequence > attempted {
        return Err(Error::InvalidInput {
            field: "data write receipt",
            reason: "receipt does not cover the batch within the attempted frontier",
        });
    }
    Ok(saved)
}

/* === Private Functions === */
fn kind(operation: &Operation) -> Option<Kind> {
    match operation {
        Operation::ObjectGroup { .. } => Some(Kind::Objects),
        Operation::Records(_) => Some(Kind::Records),
        Operation::Stream(_) => Some(Kind::Stream),
        Operation::Recovery(_) => Some(Kind::Recovery),
        _ => None,
    }
}

fn compatible(entry: &Pending, first: Kind) -> bool {
    kind(&entry.operation) == Some(first)
}

fn charge(operation: &Operation) -> Option<(usize, usize)> {
    let (count, bytes) = match operation {
        Operation::ObjectGroup { objects, .. } => {
            (objects.len().checked_mul(2)?, objects.len().checked_mul(std::mem::size_of::<StoragePart<'_>>() * 8)?)
        }
        // Data parts borrow admitted owners. Only span/vector metadata coexists;
        // neither payload copies nor a temporary JSON encoding are retained.
        Operation::Records(values) => (values.len(), std::mem::size_of::<StoragePart<'_>>() * 4),
        Operation::Stream(values) => (values.len(), std::mem::size_of::<StoragePart<'_>>() * 4),
        // Recovery's protocol still needs a contiguous owned slice. Reserve its
        // cloned payloads and geometric inline-vector growth before allocation.
        Operation::Recovery(values) => (
            values.len(),
            values.iter().try_fold(0usize, |bytes, value| {
                bytes
                    .checked_add(recovery_resident_bytes(value.payload.len(), &value.position.history, &value.required_data.history).ok()?)?
                    .checked_add(std::mem::size_of::<super::state::RecoveryEntry>() * 3)
            })?,
        ),
        _ => return None,
    };
    Some((count, bytes.checked_add(ENTRY_OVERHEAD)?))
}

/* === Tests === */
