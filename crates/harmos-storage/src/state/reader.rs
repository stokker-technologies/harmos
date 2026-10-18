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
    storage: &'a StorageManager<C>,
    // Release Budget Before Unlocking The Recovery Lifecycle.
    _reservation: Reservation,
    _lifecycle: RwLockReadGuard<'a, Lifecycle>,
    through: RecoveryPosition,
    limits: ReadLimits,
    snapshot_bound: Option<NonZeroUsize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn recovery_reader(&self, limits: ReadLimits, snapshot_bytes: Option<NonZeroUsize>) -> Result<RecoveryReader<'_, C>> {
        let lifecycle = self.inner.lifecycle.read().await;
        let invalid = |reason| Error::InvalidInput { field: "state recovery", reason };
        let snapshot_bound = snapshot_bytes
            .map(|bytes| {
                recovery_preparation_bytes(bytes.get(), self.inner.recovery.history(), self.inner.primary.history())
                    .map(|charged| NonZeroUsize::new(charged).expect("positive snapshot budget"))
            })
            .transpose()?;
        let mut bytes = limits
            .bytes
            .get()
            .checked_mul(2)
            .and_then(|n| n.checked_add(limits.rows.get().checked_mul(ENTRY_OVERHEAD)?))
            .ok_or_else(|| invalid("recovery read size overflow"))?;
        if let Some(bound) = snapshot_bound {
            bytes = bytes.max(bound.get().checked_mul(2).ok_or_else(|| invalid("snapshot read size overflow"))?);
        }
        let through = {
            let mut current = self.inner.state.lock().unwrap();
            if current.status.running || current.status.closing || current.journal_saved.is_some() {
                return Err(invalid("managed recovery requires a dormant acquired manager"));
            }
            if bytes > self.inner.settings.request.max_pending_write_bytes.get().saturating_sub(current.status.pending_bytes) {
                return Err(invalid("recovery page exceeds shared memory budget"));
            }
            current.scratch_bytes += bytes;
            current.collect();
            current.status.recovery_saved.clone()
        };
        Ok(RecoveryReader {
            storage: self,
            _lifecycle: lifecycle,
            _reservation: Reservation::charged(&self.inner, Pool::Write, bytes),
            through,
            limits,
            snapshot_bound,
        })
    }
}

impl<C> RecoveryReader<'_, C> {
    pub fn through(&self) -> &RecoveryPosition {
        &self.through
    }

    pub async fn snapshot(&self) -> Result<Option<RecoverySnapshot>> {
        let Some(bound) = self.snapshot_bound else {
            return Ok(None);
        };
        let snapshot = self.storage.inner.bounded(self.storage.inner.recovery.read_snapshot(bound)).await?;
        if let Some(snapshot) = &snapshot
            && (!self.through.covers(&snapshot.through)? || encoded_size(snapshot)? > bound.get())
        {
            return Err(Error::InvalidInput { field: "state snapshot", reason: "invalid managed snapshot boundary or size" });
        }
        if let Some(snapshot) = &snapshot {
            self.storage.check_state_dependency(&snapshot.required_data)?;
        }
        Ok(snapshot)
    }

    pub async fn page(&self, after: &RecoveryPosition) -> Result<RecoveryPage> {
        let page = self.storage.inner.bounded(self.storage.inner.recovery.read_entries(after, &self.through, self.limits)).await?;
        page.check(after, &self.through, self.limits, |entry| encoded_size(entry).unwrap_or(usize::MAX))?;
        for entry in &page.entries {
            self.storage.check_state_dependency(&entry.required_data)?;
        }
        Ok(page)
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
