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
}

pub(crate) struct FileSnapshot {
    context: crate::execution::Context,
    frontier: u64,
    sources: Vec<CheckpointSource>,
    checkpoints: crate::StorageBatch<FileCheckpoint>,
    receipts: crate::StorageBatch<super::super::MutationReceipt>,
}

struct CheckpointSource {
    context: crate::execution::Context,
    queued: std::sync::Mutex<Option<tracing::Span>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for CheckpointSource {
    fn drop(&mut self) {
        self.context.sync(|| drop(self.queued.get_mut().unwrap().take()));
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.files.capture", level = "debug", skip_all)]
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
        Ok(Some(FileSave { snapshot: snapshot.retained(Reservation::charged(&self.inner, Pool::Read, bytes).retain()) }))
    }

    // The Caller Holds Lifecycle Ownership Across Backend Access.
    #[tracing::instrument(name = "storage.staging.checkpoint.persist_files", level = "debug", skip_all)]
    pub async fn persist_files(&self, save: &Option<FileSave>) -> Result<()> {
        if let Some(save) = save {
            save.snapshot.persist(&self.inner).await?;
        }
        Ok(())
    }

    #[tracing::instrument(name = "storage.staging.checkpoint.recover_files", level = "debug", skip_all)]
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
            Reservation::charged_lifecycle(&self.inner, Pool::Read, bytes)
        };
        let memory = reservation.retain();
        let checkpoints =
            self.inner.bounded("primary.recover_productions", self.inner.primary.recover_productions(limits, memory.clone())).await?;
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
                context: crate::execution::Context::current(),
                queued: Some(tracing::debug_span!("storage.file.queue")),
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
        drop(memory);
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
            return Ok((
                FileSnapshot {
                    context: crate::execution::Context::current(),
                    frontier: self.status.saved.sequence,
                    sources: Vec::new(),
                    checkpoints: crate::StorageBatch::new(Vec::new()),
                    receipts: crate::StorageBatch::new(Vec::new()),
                },
                extra,
            ));
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
        let mut sources = Vec::with_capacity(count);
        for file in self.files.values().filter(selected) {
            let queued = file.context.sync(|| tracing::debug_span!("storage.file.checkpoint.queue", file = %file.staging.id));
            sources.push(CheckpointSource { context: file.context.clone(), queued: std::sync::Mutex::new(Some(queued)) });
            let session = file.session.as_ref().expect("selected session");
            checkpoints.push(FileCheckpoint::new(
                (*file.metadata).clone(),
                FileSession { history: file.staging.history.clone(), id: file.staging.id.clone(), generation: session.generation },
                file.length,
                &file.hash,
            ));
        }
        let receipts = self.receipts.snapshot(through);
        for receipt in &receipts {
            let span = tracing::debug_span!("storage.receipt.checkpoint.queue", mutation = %receipt.mutation.key, window = %receipt.mutation.window);
            sources.push(CheckpointSource { context: crate::execution::Context::current(), queued: std::sync::Mutex::new(Some(span)) });
        }
        self.collect();
        Ok((
            FileSnapshot {
                context: crate::execution::Context::current(),
                frontier: self.status.saved.sequence,
                sources,
                checkpoints: crate::StorageBatch::new(checkpoints),
                receipts: crate::StorageBatch::new(receipts),
            },
            bytes,
        ))
    }
}

impl FileSnapshot {
    fn retained(mut self, memory: crate::arrow::RetainedMemory) -> Self {
        self.checkpoints = self.checkpoints.retain_memory(memory.clone());
        self.receipts = self.receipts.retain_memory(memory);
        self
    }

    #[tracing::instrument(name = "storage.staging.checkpoint.persist", level = "debug", skip_all)]
    pub(crate) async fn persist(&self, inner: &Arc<super::super::manager::Inner>) -> Result<()> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::debug_span!(parent: None, "storage.file.checkpoint.batch", store = %inner.primary.history().store, epoch = %inner.primary.history().epoch, frontier = self.frontier, checkpoints = self.checkpoints.len(), receipts = self.receipts.len(), outcome = "cancelled"));
        let future = async {
            let mut contributions = Vec::with_capacity(self.sources.len());
            for (index, source) in self.sources.iter().enumerate() {
                contributions.push(source.context.sync(|| {
                    let span = if let Some(checkpoint) = self.checkpoints.as_slice().get(index) {
                        tracing::trace_span!("storage.file.checkpoint.contribution", store = %inner.primary.history().store,
                            epoch = %inner.primary.history().epoch, frontier = self.frontier, checkpoints = self.checkpoints.len(), receipts = self.receipts.len(),
                            file = %checkpoint.session.id, generation = checkpoint.session.generation, offset = checkpoint.length, outcome = "cancelled")
                    } else {
                        let receipt = &self.receipts.as_slice()[index - self.checkpoints.len()];
                        tracing::trace_span!("storage.receipt.checkpoint.contribution", store = %inner.primary.history().store,
                            epoch = %inner.primary.history().epoch, frontier = self.frontier, checkpoints = self.checkpoints.len(), receipts = self.receipts.len(),
                            mutation = %receipt.mutation.key, window = %receipt.mutation.window, outcome = "cancelled")
                    };
                    if let Some(queued) = source.queued.lock().unwrap().take() {
                        span.follows_from(queued.id());
                    }
                    crate::execution::Context::with_span(span)
                }));
            }

            let result = async {
            for index in 0..self.checkpoints.len() {
                let checkpoint = &self.checkpoints.as_slice()[index];
                inner
                .bounded("primary.save_production", inner.primary.save_production(self.checkpoints.slice(index..index + 1)))
                .await
                .inspect_err(|_error| {
                    tracing::error!(file = %checkpoint.session.id, generation = checkpoint.session.generation, offset = checkpoint.length, reason = "file checkpoint failed", "storage save failed");
                })?;
            }
            if !self.receipts.is_empty() {
                inner.bounded("primary.save_mutations", inner.primary.save_mutations(self.receipts.clone())).await?;
            }
            Ok(())
            }.await;
            let outcome = if result.is_ok() { "completed" } else { "failed" };
            for contribution in &contributions {
                contribution.sync(|| {
                    contribution.span.record("outcome", outcome);
                });
            }
            tracing::Span::current().record("outcome", outcome);
            result
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
