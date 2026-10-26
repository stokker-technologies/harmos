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
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl State {
    pub(crate) fn data_target(&self, through: &StorageCursor) -> Result<Target> {
        if !self.status.accepted.covers(through)? {
            return Err(Error::InvalidInput { field: "storage cursor", reason: "cursor is ahead of admission" });
        }
        if self.status.saved.covers(through)? {
            return Ok(Target { index: self.data_flushed, sequence: self.status.saved.sequence });
        }
        let entry = self
            .queue
            .iter()
            .find(|entry| entry.operation.domain() == Domain::Data && entry.operation.sequence() >= through.sequence)
            .ok_or(Error::InvalidInput { field: "storage cursor", reason: "admitted cursor has no pending owner" })?;
        Ok(Target { index: entry.index, sequence: entry.operation.sequence() })
    }

    pub(crate) fn collect(&mut self) {
        let now = self.execution.now();
        self.receipts.purge(now);
        self.status.mutation_bytes = self.receipts.snapshot_bytes();
        self.status.pending_batches =
            self.reserved_batches + self.queue.len() + self.files.values().filter(|file| file.pending_batch()).count();
        self.status.pending_bytes = self.reserved
            + self.scratch_bytes
            + self.retained_write_bytes.load(std::sync::atomic::Ordering::SeqCst)
            + self.queue.bytes()
            + self.files.values().map(|file| file.bytes()).sum::<usize>();
        self.status.read_bytes = self.read_bytes
            + self.deletions.values().map(|deletion| deletion.bytes).sum::<usize>()
            + self.retained_read_bytes.load(std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn needs_write(&self, entry: &Pending) -> bool {
        let written = match entry.operation.domain() {
            Domain::Data => self.data_flushed,
            Domain::Recovery => self.recovery_written.index,
        };
        entry.index > written
    }

    pub(crate) fn write_boundary(&self) -> Option<u64> {
        [self.data_request, self.recovery_request].into_iter().flatten().map(|target| target.index).min()
    }

    pub(crate) fn request_flush(&mut self) -> bool {
        let mut requested = false;
        if self.data_request.is_none() && self.data_accepted.index > self.data_flushed {
            self.data_request = Some(self.data_accepted);
            requested = true;
        }
        if self.recovery_request.is_none() && self.recovery_accepted.index > self.recovery_flushed {
            self.recovery_request = Some(self.recovery_accepted);
            requested = true;
        }
        requested
    }
}

impl Inner {
    pub(crate) fn notify(&self) {
        self.changed.send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    pub(crate) fn relieve_pressure(&self, state: &mut State, bytes: usize, batches: usize) {
        let full = state.status.pending_bytes.saturating_add(bytes) > self.settings.request.max_pending_write_bytes.get()
            || state.status.pending_batches.saturating_add(batches) > self.settings.request.max_pending_write_batches.get();
        if full {
            let lagged = state.lag_followers();
            let requested = state.request_flush();
            if lagged || requested {
                self.notify();
            }
        }
    }

    pub(crate) async fn bounded<T>(&self, future: StorageFuture<'_, T>) -> Result<T> {
        self.execution.timeout(IO_TIMEOUT, future).await.map_err(|_| {
            Error::backend(
                "storage I/O",
                crate::BackendFailure::Transient,
                io::Error::new(io::ErrorKind::TimedOut, "backend deadline exceeded"),
            )
        })?
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
