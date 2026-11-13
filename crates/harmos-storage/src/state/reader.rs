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
use crate::storage::manager::{Lifecycle, encoded_size, recovery_encoded_bound};
use crate::storage::{ReadLimits, RecoveryPosition, StorageManager};
use crate::{Error, Result};
use harmos_domain::state::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct RecoveryReader<'a> {
    context: crate::execution::Context,
    storage: &'a StorageManager,
    _lifecycle: RwLockReadGuard<'a, Lifecycle>,
    through: RecoveryPosition,
    limits: ReadLimits,
    snapshot_bytes: Option<NonZeroUsize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    #[tracing::instrument(name = "storage.state.reader.recovery_reader", level = "debug", skip_all)]
    pub async fn recovery_reader(&self, limits: ReadLimits, snapshot_bytes: Option<NonZeroUsize>) -> Result<RecoveryReader<'_>> {
        let lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        let invalid = |reason| Error::InvalidInput { field: "state recovery", reason };
        let through = {
            let mut current = self.inner.state.lock().unwrap();
            current.collect();
            if current.status.running || current.status.closing || current.journal_saved.is_some() {
                return Err(invalid("managed recovery requires a dormant acquired manager"));
            }
            current.status.recovery_saved.clone()
        };
        Ok(RecoveryReader {
            context: crate::execution::Context::current(),
            storage: self,
            _lifecycle: lifecycle,
            through,
            limits,
            snapshot_bytes,
        })
    }
}

impl RecoveryReader<'_> {
    pub fn through(&self) -> &RecoveryPosition {
        &self.through
    }

    pub async fn snapshot(&self) -> Result<Option<crate::ReadBatch<RecoverySnapshot>>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            let Some(payload_limit) = self.snapshot_bytes else {
                return Ok(None);
            };
            let encoded_bound = recovery_encoded_bound(
                payload_limit.get(),
                self.storage.inner.recovery.get_history(),
                self.storage.inner.primary.get_history(),
            )?;
            let encoded_bound = NonZeroUsize::new(encoded_bound).expect("recovery envelope is nonempty");
            let snapshot = self
                .storage
                .inner
                .bounded("recovery.get_recovery_snapshot", self.storage.inner.recovery.get_recovery_snapshot(encoded_bound))
                .await?;
            if let Some(snapshot) = &snapshot
                && (!self.through.covers(&snapshot.through)? || snapshot.payload.segment_bytes() > payload_limit.get())
            {
                return Err(Error::InvalidInput {
                    field: "state snapshot",
                    reason: "invalid managed snapshot boundary or size",
                });
            }
            if let Some(snapshot) = &snapshot {
                snapshot
                    .payload
                    .verify(payload_limit.get())
                    .map_err(|error| Error::backend("verify snapshot", crate::BackendFailure::Corrupt, error))?;
                self.storage.check_state_dependency(&snapshot.required_data)?;
            }
            Ok(snapshot.map(crate::ReadBatch::new))
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    pub async fn page(&self, after: &RecoveryPosition) -> Result<crate::ReadBatch<RecoveryPage>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            let page = self
                .storage
                .inner
                .bounded(
                    "recovery.list_recovery_entries",
                    self.storage.inner.recovery.list_recovery_entries(after, &self.through, self.limits),
                )
                .await?;
            page.check(after, &self.through, self.limits, |entry| encoded_size(entry).unwrap_or(usize::MAX))?;
            for entry in &page.entries {
                self.storage.check_state_dependency(&entry.required_data)?;
            }
            Ok(crate::ReadBatch::new(page))
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
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
