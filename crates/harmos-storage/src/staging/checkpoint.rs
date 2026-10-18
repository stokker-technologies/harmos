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
// Local Imports
use super::*;
use crate::MemorySize;
use crate::storage::capacity::{Pool, Reservation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FileSave {
    snapshot: FileSnapshot,
    _reservation: Reservation,
}

pub(crate) struct FileSnapshot {
    checkpoints: Vec<FileCheckpoint>,
    receipts: Vec<super::super::MutationReceipt>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub fn capture_files(&self, through: Option<&StorageCursor>) -> Result<Option<FileSave>> {
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closed {
            return Err(Error::NotReady { domain: "storage" });
        }
        let (snapshot, bytes) = state.capture_file_snapshot(through, self.inner.settings.budget.read_memory_bytes.get(), 0)?;
        if bytes == 0 {
            return Ok(None);
        }
        state.read_bytes += bytes;
        state.collect();
        Ok(Some(FileSave { snapshot, _reservation: Reservation::charged(&self.inner, Pool::Read, bytes) }))
    }

    // The Caller Holds Lifecycle Ownership Across Backend Access.
    pub async fn persist_files(&self, save: &Option<FileSave>) -> Result<()> {
        if let Some(save) = save {
            save.snapshot.persist(&self.inner).await?;
        }
        Ok(())
    }

    pub(crate) async fn recover_files(&self) -> Result<()> {
        if !self.inner.primary.supports_file_sessions() {
            return Ok(());
        }
        // Reserve Before Asking The Backend To Allocate Its Complete Bounded Set.
        let capacity = self.inner.settings.budget.read_memory_bytes.get();
        let bytes = capacity / 2;
        let limits = ReadLimits { rows: crate::storage::config::MAX_FILES, bytes: NonZeroUsize::new(bytes).ok_or_else(size_error)? };
        let reservation = {
            let mut state = self.inner.state.lock().unwrap();
            state.collect();
            if state.status.read_bytes > capacity - bytes {
                return Err(Error::ResourceExhausted { resource: "file recovery memory" });
            }
            state.read_bytes += bytes;
            state.collect();
            Reservation::charged(&self.inner, Pool::Read, bytes)
        };
        let checkpoints = self.inner.bounded(self.inner.primary.recover_productions(limits)).await?;
        if checkpoints.len() > limits.rows.get() || checkpoints.memory_size() > bytes {
            return Err(Error::InvalidInput { field: "file recovery", reason: "backend exceeded recovery bounds" });
        }
        let write_bytes = checkpoints.iter().try_fold(0usize, |total, checkpoint| {
            let (_, bytes) = file_bytes(&checkpoint.metadata, &checkpoint.session.history)?;
            total.checked_add(bytes).ok_or_else(size_error)
        })?;
        let mut write_reservation = {
            let mut state = self.inner.state.lock().unwrap();
            state.collect();
            if write_bytes >= self.inner.settings.request.max_pending_write_bytes.get()
                || state.status.pending_bytes > self.inner.settings.request.max_pending_write_bytes.get() - write_bytes
            {
                return Err(Error::ResourceExhausted { resource: "recovered file production" });
            }
            state.scratch_bytes += write_bytes;
            state.collect();
            Reservation::charged(&self.inner, Pool::Write, write_bytes)
        };
        let mut restored = std::collections::BTreeMap::new();
        let mut names = std::collections::BTreeSet::new();
        let mut total = 0usize;
        for checkpoint in checkpoints {
            self.inner.primary.history().check(&checkpoint.session.history)?;
            let hash = checkpoint.restore_hash()?;
            if !names.insert(checkpoint.session.id.clone()) {
                return Err(Error::InvalidInput { field: "file recovery", reason: "duplicate file identity" });
            }
            let (reference_bytes, base_bytes) = file_bytes(&checkpoint.metadata, &checkpoint.session.history)?;
            total = total.checked_add(base_bytes).ok_or_else(size_error)?;
            if reference_bytes > self.inner.settings.max_batch_bytes().get()
                || total >= self.inner.settings.request.max_pending_write_bytes.get()
            {
                return Err(Error::ResourceExhausted { resource: "recovered file production" });
            }
            let id = NEXT_FILE.try_update(Ordering::Relaxed, Ordering::Relaxed, |value| value.checked_add(1)).map_err(|_| size_error())?;
            let progress = Arc::new(Mutex::new(Progress {
                generation: checkpoint.session.generation,
                ready: true,
                length: checkpoint.length,
                ..Progress::default()
            }));
            let file = FileState {
                metadata: Arc::new(checkpoint.metadata),
                staging: StagedFile { history: checkpoint.session.history, id: checkpoint.session.id },
                session: Some(Session::recovered(
                    checkpoint.session.generation,
                    crate::storage::config::FILE_SESSION_TIMEOUT,
                    self.inner.execution,
                )),
                extra: None,
                mutation: None,
                progress,
                hash,
                length: checkpoint.length,
                ticket: 0,
                command: None,
                abandoned: false,
                in_flight: false,
                retrying: false,
                base_bytes,
                reference_bytes,
            };
            restored.insert(id, file);
        }
        {
            let mut state = self.inner.state.lock().unwrap();
            if !state.files.is_empty() {
                return Err(Error::AlreadyRegistered { domain: "file recovery", name: "active productions".into() });
            }
            state.scratch_bytes -= write_reservation.disown();
            state.files = restored;
            state.collect();
        }
        drop(reservation);
        Ok(())
    }

    pub(crate) fn retire_files(&self) {
        let mut state = self.inner.state.lock().unwrap();
        for file in state.files.values() {
            file.progress.lock().unwrap().closed = true;
        }
        state.files.clear();
        state.collect();
        self.inner.notify();
    }
}

impl State {
    pub(crate) fn capture_file_snapshot(
        &mut self, through: Option<&StorageCursor>, capacity: usize, extra: usize,
    ) -> Result<(FileSnapshot, usize)> {
        let selected = |file: &&FileState| file.session.is_some() && file.progress.lock().unwrap().ready;
        let count = self.files.values().filter(selected).count();
        if count == 0 && self.receipts.snapshot_bytes() == 0 && extra == 0 {
            return Ok((FileSnapshot { checkpoints: Vec::new(), receipts: Vec::new() }, extra));
        }
        let bytes = self.files.values().filter(selected).try_fold(
            std::mem::size_of::<FileSave>()
                .checked_add(extra)
                .ok_or_else(size_error)?
                .checked_add(self.receipts.snapshot_bytes())
                .ok_or_else(size_error)?,
            |bytes, file| bytes.checked_add(file.base_bytes.checked_mul(2).ok_or_else(size_error)?).ok_or_else(size_error),
        )?;

        self.collect();
        if bytes > capacity || self.status.read_bytes > capacity - bytes {
            return Err(Error::ResourceExhausted { resource: "file checkpoint memory" });
        }

        let mut checkpoints = Vec::with_capacity(count);
        for file in self.files.values().filter(selected) {
            let session = file.session.as_ref().expect("selected session");
            checkpoints.push(FileCheckpoint::new(
                (*file.metadata).clone(),
                FileSession { history: file.staging.history.clone(), id: file.staging.id.clone(), generation: session.generation },
                file.length,
                &file.hash,
            ));
        }
        let receipts = self.receipts.snapshot(through);
        self.collect();
        Ok((FileSnapshot { checkpoints, receipts }, bytes))
    }
}

impl FileSnapshot {
    pub(crate) async fn persist(&self, inner: &Arc<super::super::manager::Inner>) -> Result<()> {
        for checkpoint in &self.checkpoints {
            inner
                .bounded(inner.primary.save_production(checkpoint))
                .await
                .inspect_err(|error| {
                    tracing::error!(file = %checkpoint.session.id, generation = checkpoint.session.generation, offset = checkpoint.length, error = %error, reason = "file checkpoint failed", "storage save failed");
                })?;
        }
        if !self.receipts.is_empty() {
            inner.bounded(inner.primary.save_mutations(&self.receipts)).await?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
