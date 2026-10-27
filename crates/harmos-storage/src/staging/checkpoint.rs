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
        Ok(state.capture_file_snapshot(through)?.map(|snapshot| FileSave { snapshot }))
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
        // Recover The Complete Bounded Checkpoint Set.
        let bytes = super::super::config::MAX_BATCH_BYTES;
        let limits = ReadLimits { rows: crate::storage::config::MAX_FILES, bytes: NonZeroUsize::new(bytes).ok_or_else(size_error)? };
        let checkpoints = self.inner.bounded("primary.recover_productions", self.inner.primary.recover_productions(limits)).await?;
        if checkpoints.len() > limits.rows.get()
            || checkpoints.iter().try_fold(0usize, |total, checkpoint| encoded_size(checkpoint).map(|size| total.saturating_add(size)))?
                > bytes
        {
            return Err(Error::InvalidInput { field: "file recovery", reason: "backend exceeded recovery bounds" });
        }
        let mut restored = std::collections::BTreeMap::new();
        let mut names = std::collections::BTreeSet::new();
        for checkpoint in checkpoints {
            self.inner.primary.history().check(&checkpoint.session.history)?;
            let hash = checkpoint.restore_hash()?;
            if !names.insert(checkpoint.session.id.clone()) {
                return Err(Error::InvalidInput { field: "file recovery", reason: "duplicate file identity" });
            }
            let reference_bytes = file_bytes(&checkpoint.metadata, &checkpoint.session.history)?;
            if reference_bytes > self.inner.settings.max_batch_bytes().get() {
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

                reference_bytes,
            };
            restored.insert(id, file);
        }
        {
            let mut state = self.inner.state.lock().unwrap();
            if !state.files.is_empty() {
                return Err(Error::AlreadyRegistered { domain: "file recovery", name: "active productions".into() });
            }
            state.files = restored;
            state.collect();
        }
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
    pub(crate) fn capture_file_snapshot(&mut self, through: Option<&StorageCursor>) -> Result<Option<FileSnapshot>> {
        let selected = |file: &&FileState| file.session.is_some() && file.progress.lock().unwrap().ready;
        let count = self.files.values().filter(selected).count();
        if count == 0 && self.receipts.is_empty() {
            return Ok(None);
        }
        self.collect();
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
        Ok(Some(FileSnapshot {
            context: crate::execution::Context::current(),
            frontier: self.status.saved.sequence,
            sources,
            checkpoints: crate::StorageBatch::new(checkpoints),
            receipts: crate::StorageBatch::new(receipts),
        }))
    }
}

impl FileSnapshot {
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
                if !self.checkpoints.is_empty() {
                    inner.bounded("primary.save_production", inner.primary.save_production(self.checkpoints.clone())).await?;
                }
                if !self.receipts.is_empty() {
                    inner.bounded("primary.save_mutations", inner.primary.save_mutations(self.receipts.clone())).await?;
                    inner.state.lock().unwrap().receipts.saved(self.receipts.as_slice());
                }
                Ok(())
            }
            .await;
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
