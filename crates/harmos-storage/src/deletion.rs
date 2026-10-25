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
use sha2::{Digest, Sha256};

// Local Imports
use super::config::MAX_MUTATIONS;
use super::manager::{Inner, State, Target};
use super::receipts::ReceiptState;
use super::{MutationId, MutationReceipt, MutationValue, StagedFile, StorageManager};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Deletion {
    pub(super) work: Arc<DeleteWork>,
    pub(super) bytes: usize,
    failure: Option<Arc<Error>>,
}

pub(super) struct DeleteWork {
    pub(super) context: crate::execution::Context,
    pub(super) queued: std::sync::Mutex<Option<tracing::Span>>,
    target: Target,
    pub(super) file: StagedFile,
    pub(super) receipt: MutationReceipt,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for DeleteWork {
    fn drop(&mut self) {
        self.context.sync(|| drop(self.queued.get_mut().unwrap().take()));
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.deletion.delete_file_once", level = "debug", skip_all, fields(outcome = "cancelled"))]
    pub async fn delete_file_once(&self, mutation: &MutationId, file: &StagedFile) -> Result<StagedFile> {
        let result = async {
            self.inner.primary.history().check(&file.history)?;
            if !self.inner.primary.supports_mutation_receipts() || !self.inner.primary.supports_file_deletion() {
                return Err(invalid("backend must support durable deletion"));
            }
            let fingerprint = fingerprint(file);
            let mut retry = true;
            let mut completion = None;
            let mut changed = self.inner.changed.subscribe();
            loop {
                let mut notify = false;
                {
                    let mut state = self.inner.state.lock().unwrap();
                    if state.status.closing {
                        return Err(Error::Closing { domain: "file deletion" });
                    }
                    if !state.ready || !state.status.running {
                        return Err(Error::NotReady { domain: "file deletion" });
                    }
                    match state.receipts.lookup(mutation, &fingerprint, completion.as_ref())? {
                        ReceiptState::Complete(value) => return value.deleted(),
                        ReceiptState::Pending => {
                            if completion.is_none() {
                                let owner = state.receipts.completion(mutation);
                                completion = Some(owner);
                            }
                            let deletion = state.deletions.get(mutation).ok_or_else(|| invalid("pending identity owns different work"))?;
                            if let Some(source) = deletion.failure.clone() {
                                if !retry {
                                    return Err(Error::Persistence { source });
                                }
                                if let Some(other) = &state.status.failure
                                    && !Arc::ptr_eq(other, &source)
                                {
                                    return Err(Error::Persistence { source: other.clone() });
                                }
                                state.deletions.get_mut(mutation).unwrap().failure = None;
                                state.status.failure = None;
                                state.retry = state.retry.wrapping_add(1);
                                notify = true;
                            } else if let Some(source) = &state.status.failure {
                                return Err(Error::Persistence { source: source.clone() });
                            }
                            retry = false;
                        }
                        ReceiptState::Missing => {
                            if let Some(source) = &state.status.failure {
                                return Err(Error::Persistence { source: source.clone() });
                            }
                            if state.files.values().any(|active| active.identifies(file)) {
                                tracing::error!(file = %file.id, reason = "active producer", "file deletion rejected");
                                return Err(invalid("active production must finish or abandon before deletion"));
                            }
                            if state.deleting(&file.id) {
                                return Err(invalid("another deletion owns this file"));
                            }
                            if state.deletions.len() >= MAX_MUTATIONS.get() {
                                return Err(exhausted());
                            }
                            let bytes = crate::btree_map_memory::<MutationId, Deletion>(1)
                                .saturating_add(std::mem::size_of::<DeleteWork>())
                                .saturating_add(std::mem::size_of::<MutationReceipt>())
                                .saturating_add(std::mem::size_of::<MutationValue>())
                                .saturating_add(mutation.memory_size().saturating_mul(2))
                                .saturating_add(file.memory_size().saturating_mul(2))
                                .saturating_add(2 * std::mem::size_of::<usize>());
                            state.collect();
                            let capacity = self.inner.settings.budget.read_memory_bytes.get();
                            if bytes > capacity || state.status.read_bytes > capacity - bytes {
                                return Err(exhausted());
                            }
                            let outcome = MutationValue::Deleted(file.clone());
                            state.receipts.insert(mutation, fingerprint, outcome.memory_size())?;
                            let owner = state.receipts.completion(mutation);
                            completion = Some(owner);
                            let receipt = state.receipts.prepared(mutation, outcome);
                            let target = state.data_accepted;
                            state.deletions.insert(
                                mutation.clone(),
                                Deletion {
                                    work: Arc::new(DeleteWork {
                                        context: crate::execution::Context::current(),
                                        queued: std::sync::Mutex::new(Some(tracing::debug_span!("storage.deletion.queue"))),
                                        target,
                                        file: file.clone(),
                                        receipt,
                                    }),
                                    bytes,
                                    failure: None,
                                },
                            );
                            retry = false;
                            notify = true;
                        }
                    }
                    state.collect();
                }
                if notify {
                    self.inner.notify();
                }
                changed.changed().await.map_err(|_| Error::Closing { domain: "file deletion" })?;
            }
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    #[tracing::instrument(name = "storage.deletion.settle_deletions", level = "debug", skip_all)]
    pub(super) async fn settle_deletions(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.deletions.is_empty() {
                return Ok(());
            }
            for deletion in state.deletions.values_mut() {
                deletion.failure = None;
            }
            state.status.failure = None;
            state.retry = state.retry.wrapping_add(1);
        }
        self.inner.notify();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if state.deletions.is_empty() {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "file deletion worker" });
                }
            }
            changed.changed().await.map_err(|_| Error::Closing { domain: "file deletion" })?;
        }
    }
}

impl State {
    pub(super) fn deleting(&self, file: &Identifier) -> bool {
        self.deletions.values().any(|deletion| deletion.work.file.id == *file)
    }

    pub(super) fn request_deletions(&mut self) {
        for deletion in self.deletions.values().filter(|deletion| deletion.failure.is_none()) {
            let target = deletion.work.target;
            if target.index > self.data_flushed && self.data_request.is_none_or(|current| target.index < current.index) {
                self.data_request = Some(target);
            }
        }
    }

    pub(super) fn deletion_action(&self) -> Option<Arc<DeleteWork>> {
        self.deletions
            .values()
            .find(|deletion| deletion.failure.is_none() && deletion.work.target.index <= self.data_flushed)
            .map(|deletion| deletion.work.clone())
    }

    pub(super) fn deleting_data(&self, index: u64) -> bool {
        self.deletions.values().any(|deletion| deletion.failure.is_none() && deletion.work.target.index >= index)
    }

    pub(super) fn deletion_failed(&mut self, mutation: &MutationId, source: Arc<Error>) {
        if let Some(deletion) = self.deletions.get_mut(mutation) {
            tracing::error!(file = %deletion.work.file.id, window = %mutation.window, mutation = %mutation.key, reason = "file deletion failed", "file deletion retained for retry");
            deletion.failure = Some(source);
        }
    }

    pub(super) fn deletion_data_failed(&mut self, index: u64, source: Arc<Error>) {
        for deletion in self.deletions.values_mut().filter(|deletion| deletion.work.target.index >= index) {
            deletion.failure = Some(source.clone());
        }
    }
}

impl DeleteWork {
    #[tracing::instrument(name = "storage.deletion.perform", level = "debug", skip_all)]
    pub(super) async fn perform(&self, inner: &Arc<Inner>) -> Result<()> {
        inner.bounded("primary.delete_file", inner.primary.delete_file(&self.file)).await?;
        inner.bounded("primary.save_mutations", inner.primary.save_mutations(std::slice::from_ref(&self.receipt))).await?;
        tracing::info!(file = %self.file.id, mutation = %self.receipt.mutation.key, reason = "explicit deletion", "file deleted");
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn fingerprint(file: &StagedFile) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"storage.delete_file:v1\0");
    for bytes in [file.history.store.as_str().as_bytes(), file.history.epoch.as_str().as_bytes(), file.id.as_str().as_bytes()] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().into()
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "file deletion", reason }
}
fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "file deletion requests" }
}

/* === Tests === */
