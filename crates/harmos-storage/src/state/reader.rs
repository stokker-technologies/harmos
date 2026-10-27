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
use std::num::NonZeroUsize;

// Third Party Imports
use tokio::sync::{RwLockReadGuard, watch};

// Local Imports
use super::{RecoveryPage, RecoverySnapshot};
use crate::storage::capacity::{Pool, Reservation};
use crate::storage::manager::{ENTRY_OVERHEAD, Lifecycle, encoded_size, recovery_preparation_bytes};
use crate::storage::{ReadLimits, RecoveryPosition, StorageManager};
use crate::{Error, Result};
use harmos_domain::state::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct RecoveryReader<'a, C> {
    context: crate::execution::Context,
    storage: &'a StorageManager<C>,
    _lifecycle: RwLockReadGuard<'a, Lifecycle>,
    through: RecoveryPosition,
    limits: ReadLimits,
    page_bytes: usize,
    snapshot_bound: Option<NonZeroUsize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.state.reader.recovery_reader", level = "debug", skip_all)]
    pub async fn recovery_reader(&self, limits: ReadLimits, snapshot_bytes: Option<NonZeroUsize>) -> Result<RecoveryReader<'_, C>> {
        let lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        let invalid = |reason| Error::InvalidInput { field: "state recovery", reason };
        let snapshot_bound = snapshot_bytes
            .map(|bytes| {
                recovery_preparation_bytes(bytes.get(), self.inner.recovery.history(), self.inner.primary.history())
                    .map(|charged| NonZeroUsize::new(charged).expect("positive snapshot budget"))
            })
            .transpose()?;
        let page_bytes = limits
            .bytes
            .get()
            .checked_mul(2)
            .and_then(|n| n.checked_add(limits.rows.get().checked_mul(ENTRY_OVERHEAD)?))
            .ok_or_else(|| invalid("recovery read size overflow"))?;
        let mut bytes = page_bytes;
        if let Some(bound) = snapshot_bound {
            bytes = bytes.max(bound.get().checked_mul(2).ok_or_else(|| invalid("snapshot read size overflow"))?);
        }
        let through = {
            let mut current = self.inner.state.lock().unwrap();
            current.collect();
            if current.status.running || current.status.closing || current.journal_saved.is_some() {
                return Err(invalid("managed recovery requires a dormant acquired manager"));
            }
            if bytes > self.inner.settings.request.max_pending_write_bytes.get().saturating_sub(current.status.pending_bytes) {
                return Err(invalid("recovery page exceeds shared memory budget"));
            }
            current.status.recovery_saved.clone()
        };
        Ok(RecoveryReader {
            context: crate::execution::Context::current(),
            storage: self,
            _lifecycle: lifecycle,
            through,
            limits,
            page_bytes,
            snapshot_bound,
        })
    }
}

impl<C> RecoveryReader<'_, C> {
    pub fn through(&self) -> &RecoveryPosition {
        &self.through
    }

    pub async fn snapshot(&self) -> Result<Option<crate::ReadBatch<RecoverySnapshot>>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            let Some(bound) = self.snapshot_bound else {
                return Ok(None);
            };
            let reservation = self.reserve(bound.get().saturating_mul(2))?.retain();
            let snapshot = self
                .storage
                .inner
                .bounded("recovery.read_snapshot", self.storage.inner.recovery.read_snapshot(bound, reservation.clone()))
                .await?;
            if let Some(snapshot) = &snapshot
                && (!self.through.covers(&snapshot.through)? || encoded_size(snapshot)? > bound.get())
            {
                return Err(Error::InvalidInput {
                    field: "state snapshot",
                    reason: "invalid managed snapshot boundary or size",
                });
            }
            if let Some(snapshot) = &snapshot {
                self.storage.check_state_dependency(&snapshot.required_data)?;
            }
            Ok(snapshot.map(|snapshot| crate::ReadBatch::retained(snapshot, reservation)))
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    pub async fn page(&self, after: &RecoveryPosition) -> Result<crate::ReadBatch<RecoveryPage>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            let reservation = self.reserve(self.page_bytes)?.retain();
            let page = self
                .storage
                .inner
                .bounded(
                    "recovery.read_entries",
                    self.storage.inner.recovery.read_entries(after, &self.through, self.limits, reservation.clone()),
                )
                .await?;
            page.check(after, &self.through, self.limits, |entry| encoded_size(entry).unwrap_or(usize::MAX))?;
            for entry in &page.entries {
                self.storage.check_state_dependency(&entry.required_data)?;
            }
            Ok(crate::ReadBatch::retained(page, reservation))
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    fn reserve(&self, bytes: usize) -> Result<Reservation> {
        let mut state = self.storage.inner.state.lock().unwrap();
        state.collect();
        if bytes > self.storage.inner.settings.request.max_pending_write_bytes.get().saturating_sub(state.status.pending_bytes) {
            return Err(Error::ResourceExhausted {
                resource: "retained recovery read memory",
            });
        }
        state.scratch_bytes += bytes;
        state.collect();
        Ok(Reservation::charged_lifecycle(&self.storage.inner, Pool::Write, bytes))
    }

    pub fn finish(self, position: Position) -> watch::Receiver<Position> {
        let (saved, durable) = watch::channel(position);
        let mut current = self.storage.inner.state.lock().unwrap();
        current.journal_required_data = current.status.saved.clone();
        current.journal_saved = Some(saved);
        current.journal_limit = self.limits.bytes.get();
        durable
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
