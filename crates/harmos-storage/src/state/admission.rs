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

use crate::storage::manager::RecordWriteBytes;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{RecoveryEntry, RecoverySnapshot};
use crate::storage::manager::{
    ENTRY_OVERHEAD, Inner, Operation, Pending, Target, encoded_size, recovery_preparation_bytes, recovery_resident_bytes,
};
use crate::storage::{ReadLimits, RecordInput, StorageCursor, StorageManager, StoredRecord};
use crate::{Error, Result};
use harmos_domain::state::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Permit {
    inner: Arc<Inner>,
    bytes: usize,
    target: Target,
    first_record: StorageCursor,
    records: Vec<StoredRecord>,
    record_bytes: usize,
    capture_bytes: usize,
    payload: Vec<u8>,
    active: bool,
    snapshot: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn reserve_transition(&self, payload_bytes: usize, records: Option<ReadLimits>) -> Result<Permit> {
        self.reserve_recovery(payload_bytes, records, false).await
    }

    pub async fn reserve_snapshot(&self, payload_bytes: usize) -> Result<Permit> {
        self.reserve_recovery(payload_bytes, None, true).await
    }

    async fn reserve_recovery(&self, payload_bytes: usize, records: Option<ReadLimits>, snapshot: bool) -> Result<Permit> {
        let recovery_bytes = recovery_preparation_bytes(payload_bytes, self.inner.recovery.history(), self.inner.primary.history())?;
        let bytes = recovery_bytes;
        let batches = 1 + usize::from(records.is_some());
        if batches > self.inner.settings.request.max_pending_write_batches.get() {
            return Err(Error::InvalidInput { field: "managed capture", reason: "requires two queue slots" });
        }
        if bytes > self.inner.settings.request.max_pending_write_bytes.get() {
            return Err(Error::InvalidInput { field: "managed recovery", reason: "group exceeds memory budget" });
        }
        let mut changed = self.inner.changed.subscribe();
        loop {
            {
                let mut state = self.inner.state.lock().unwrap();
                if !snapshot && recovery_bytes > state.journal_limit {
                    return Err(Error::InvalidInput { field: "managed recovery", reason: "group exceeds recovery read bound" });
                }
                if state.status.closing {
                    return Err(Error::Closing { domain: "storage" });
                }
                if !state.ready || !state.status.running {
                    return Err(Error::NotReady { domain: "storage" });
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                state.collect();
                if state.reserved == 0
                    && state.finishing.is_none()
                    && state.status.pending_bytes <= self.inner.settings.request.max_pending_write_bytes.get() - bytes
                    && state.status.pending_batches <= self.inner.settings.request.max_pending_write_batches.get() - batches
                {
                    state
                        .next
                        .checked_add(batches as u64)
                        .ok_or(Error::InvalidInput { field: "managed capture", reason: "queue order overflow" })?;
                    let mut first_record = state.status.accepted.clone();
                    if let Some(limits) = records {
                        first_record
                            .sequence
                            .checked_add(limits.rows.get() as u64)
                            .ok_or(Error::InvalidInput { field: "managed capture", reason: "position overflow" })?;
                        first_record.sequence += 1;
                    }
                    let target = Target {
                        index: state
                            .next
                            .checked_add(1)
                            .ok_or(Error::InvalidInput { field: "managed recovery", reason: "queue order overflow" })?,
                        sequence: state
                            .status
                            .recovery_accepted
                            .sequence
                            .checked_add(u64::from(!snapshot))
                            .ok_or(Error::InvalidInput { field: "managed recovery", reason: "position overflow" })?,
                    };
                    state.reserved = bytes;
                    state.reserved_batches = batches;
                    state.collect();
                    return Ok(Permit {
                        inner: self.inner.clone(),
                        bytes: recovery_bytes,
                        target,
                        first_record,
                        records: Vec::new(),
                        record_bytes: 0,
                        capture_bytes: 0,
                        payload: Vec::new(),
                        active: true,
                        snapshot,
                    });
                }
                self.inner.relieve_pressure(&mut state, bytes, batches);
            }
            changed.changed().await.map_err(|_| Error::NotReady { domain: "storage" })?;
        }
    }

    pub async fn settle_transition(&self) {
        let mut changed = self.inner.changed.subscribe();
        loop {
            if self.inner.state.lock().unwrap().reserved == 0 {
                return;
            }
            if changed.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Permit {
    pub fn snapshot_payload(&mut self, payload: Vec<u8>) -> Result<()> {
        debug_assert!(self.snapshot);
        let bytes = recovery_resident_bytes(payload.capacity(), self.inner.recovery.history(), self.inner.primary.history())?;
        if bytes > self.bytes {
            return Err(Error::InvalidInput { field: "state snapshot", reason: "payload exceeds reservation" });
        }
        self.payload = payload;
        self.bytes = bytes;
        let mut state = self.inner.state.lock().unwrap();
        state.reserved = bytes;
        state.collect();
        self.inner.notify();
        Ok(())
    }

    pub fn projected(&self) -> &[StoredRecord] {
        &self.records
    }

    pub async fn push(&mut self, record: RecordInput, limits: ReadLimits) -> Result<()> {
        let invalid = || Error::InvalidInput { field: "record projection", reason: "capture limit exceeded" };
        if self.records.len() >= limits.rows.get() {
            return Err(invalid());
        }
        let capture = encoded_size(&record.borrowed())?
            .checked_add(encoded_size(&self.first_record.history)?)
            .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD))
            .ok_or_else(invalid)?;
        let capture_total = self.capture_bytes.checked_add(capture).ok_or_else(invalid)?;
        // An incremental Vec may retain spare slots or briefly own both old and
        // replacement allocations. Four inline slots per live record cover
        // initial minimum capacity and geometric growth before copying.
        let bytes =
            record.resident_bytes(&self.first_record.history)?.checked_add(std::mem::size_of::<StoredRecord>() * 3).ok_or_else(invalid)?;
        let total = self.record_bytes.checked_add(bytes).ok_or_else(invalid)?;
        // Refuse a group that cannot fit even after all accepted work drains.
        // Waiting with a partial permit in that case would deadlock itself.
        if capture_total > limits.bytes.get()
            || self.bytes.checked_add(total).is_none_or(|total| total > self.inner.settings.request.max_pending_write_bytes.get())
        {
            return Err(invalid());
        }
        let mut position = self.first_record.clone();
        position.sequence = position.sequence.checked_add(self.records.len() as u64).ok_or_else(invalid)?;
        let capacity = self.inner.settings.request.max_pending_write_bytes.get();
        self.inner
            .admit_when(|state| {
                super::super::capacity::admitting(state, super::super::capacity::Pool::Write)?;
                state.collect();
                if state.status.pending_bytes <= capacity - bytes {
                    state.reserved += bytes;
                    state.collect();
                    return Ok(Some(()));
                }
                // File finalization also needs this exclusive permit to end.
                // Only queued writes and saver scratch can drain underneath us;
                // never wait on bytes held by our own permit or staged owners.
                let held = state.status.pending_bytes.saturating_sub(state.queue.bytes()).saturating_sub(state.scratch_bytes);
                if held > capacity - bytes {
                    return Err(invalid());
                }
                // Flush accepted work only; this permit owns the uncommitted
                // position and partial projection until the whole group settles.
                self.inner.relieve_pressure(state, bytes, 0);
                Ok(None)
            })
            .await?;
        self.records.push(record.stored(position));
        self.record_bytes = total;
        self.capture_bytes = capture_total;
        Ok(())
    }

    pub fn encode(&mut self, value: &impl serde::Serialize) -> Result<()> {
        self.payload =
            serde_json::to_vec(value).map_err(|_| Error::InvalidInput { field: "managed recovery", reason: "cannot encode group" })?;
        // Encoding runs under the original conservative scratch reservation.
        // Only the encoded buffer survives commit; release unused framing and
        // growth allowance before the group becomes queued work.
        let bytes = recovery_resident_bytes(self.payload.capacity(), self.inner.recovery.history(), self.inner.primary.history())?;
        if bytes > self.bytes {
            return Err(Error::InvalidInput { field: "managed recovery", reason: "encoded allocation exceeds reservation" });
        }
        self.bytes = bytes;
        let mut state = self.inner.state.lock().unwrap();
        state.reserved =
            self.bytes.checked_add(self.record_bytes).ok_or(Error::InvalidInput { field: "managed recovery", reason: "size overflow" })?;
        state.reserved_batches = 1 + usize::from(!self.records.is_empty());
        state.collect();
        self.inner.notify();
        Ok(())
    }

    pub fn commit(mut self, frontier: Position) {
        let mut state = self.inner.state.lock().unwrap();
        let required_data = if self.snapshot {
            state.journal_required_data.clone()
        } else {
            self.records.last().map_or_else(|| state.status.saved.clone(), |record| record.position.clone())
        };
        if required_data.sequence > state.journal_required_data.sequence {
            state.journal_required_data = required_data.clone();
        }
        if let Some(last) = self.records.last() {
            state.status.accepted = last.position.clone();
            state.data_accepted = Target { index: self.target.index, sequence: last.position.sequence };
            state.queue.push_back(Arc::new(Pending::new(
                &self.inner,
                None,
                self.target.index,
                self.record_bytes,
                Operation::Records(crate::StorageBatch::new(std::mem::take(&mut self.records))),
            )));
            self.target.index += 1; // Both slots were checked before mutation.
        }
        let position =
            crate::storage::RecoveryPosition { history: state.status.recovery_accepted.history.clone(), sequence: self.target.sequence };
        state.status.recovery_accepted = position.clone();
        state.recovery_accepted = self.target;
        state.next = self.target.index;
        state.queue.push_back(Arc::new(Pending::new(
            &self.inner,
            (!self.snapshot).then_some(frontier),
            self.target.index,
            self.bytes,
            if self.snapshot {
                Operation::Snapshot(crate::StorageBatch::new(vec![RecoverySnapshot {
                    required_data: required_data.clone(),
                    through: position,
                    payload: std::mem::take(&mut self.payload),
                }]))
            } else {
                Operation::Recovery(crate::StorageBatch::new(vec![RecoveryEntry {
                    required_data,
                    position,
                    payload: std::mem::take(&mut self.payload),
                }]))
            },
        )));
        state.reserved = 0;
        state.reserved_batches = 0;
        self.active = false;
        state.collect();
        self.inner.notify();
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        if self.active {
            let mut state = self.inner.state.lock().unwrap();
            state.reserved = 0;
            state.reserved_batches = 0;
            state.collect();
            self.inner.notify();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
