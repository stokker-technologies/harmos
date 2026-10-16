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
use super::manager::{Domain, Inner, State, Target};
use super::receipts::ReceiptState;
use super::staging::checkpoint::FileSnapshot;
use super::{MutationId, MutationReceipt, MutationValue, StorageCursor, StorageManager};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct SaveState {
    pub(super) work: Arc<SaveWork>,
    pub(super) bytes: usize,
    pub(super) failure: Option<Arc<Error>>,
}

pub(super) struct SaveWork {
    target: Target,
    files: FileSnapshot,
    pub(super) receipt: MutationReceipt,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn save_once(
        &self,
        mutation: &MutationId,
        through: &StorageCursor,
    ) -> Result<StorageCursor> {
        self.inner.primary.history().check(&through.history)?;
        if !self.inner.primary.supports_mutation_receipts() {
            return Err(invalid("backend does not support mutation receipts"));
        }
        let fingerprint = fingerprint(through);
        let mut changed = self.inner.changed.subscribe();
        let mut retry = true;
        loop {
            let mut notify = false;
            {
                let mut state = self.inner.state.lock().unwrap();
                if state.status.closing {
                    return Err(Error::Closing {
                        domain: "storage save",
                    });
                }
                if !state.ready || !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage save",
                    });
                }
                match state.receipts.lookup(mutation, &fingerprint)? {
                    ReceiptState::Complete(value) => return value.cursor(),
                    ReceiptState::Pending => {
                        let save = state
                            .saves
                            .get(mutation)
                            .ok_or_else(|| invalid("pending receipt owns different work"))?;
                        if let Some(source) = save.failure.clone() {
                            if !retry {
                                return Err(Error::Persistence { source });
                            }
                            if let Some(other) = &state.status.failure
                                && !Arc::ptr_eq(other, &source)
                            {
                                return Err(Error::Persistence {
                                    source: other.clone(),
                                });
                            }
                            state.saves.get_mut(mutation).unwrap().failure = None;
                            state.status.failure = None;
                            state.retry = state.retry.wrapping_add(1);
                            notify = true;
                        } else if let Some(source) = &state.status.failure {
                            return Err(Error::Persistence {
                                source: source.clone(),
                            });
                        }
                        retry = false;
                    }
                    ReceiptState::Missing => {
                        if let Some(source) = &state.status.failure {
                            return Err(Error::Persistence {
                                source: source.clone(),
                            });
                        }
                        if state.saves.len() >= super::config::MAX_MUTATIONS.get() {
                            return Err(exhausted());
                        }
                        let target = state.data_target(through)?;
                        let extra = crate::btree_map_memory::<MutationId, SaveState>(1)
                            .saturating_add(std::mem::size_of::<SaveWork>())
                            .saturating_add(std::mem::size_of::<MutationReceipt>())
                            .saturating_add(std::mem::size_of::<MutationValue>())
                            .saturating_add(mutation.memory_size().saturating_mul(2))
                            .saturating_add(through.memory_size().saturating_mul(2))
                            .saturating_add(2 * std::mem::size_of::<usize>());
                        let (files, bytes) = state.capture_file_snapshot(
                            Some(through),
                            self.inner.settings.budget.read_memory_bytes.get(),
                            extra,
                        )?;
                        let outcome = MutationValue::Cursor(through.clone());
                        state
                            .receipts
                            .insert(mutation, fingerprint, outcome.memory_size())?;
                        let receipt = state.receipts.prepared(mutation, outcome);
                        state.saves.insert(
                            mutation.clone(),
                            SaveState {
                                work: Arc::new(SaveWork {
                                    target,
                                    files,
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
            changed.changed().await.map_err(|_| Error::Closing {
                domain: "storage save",
            })?;
        }
    }

    pub(super) async fn settle_saves(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.saves.is_empty() {
                return Ok(());
            }
            for save in state.saves.values_mut() {
                save.failure = None;
            }
            state.status.failure = None;
            state.retry = state.retry.wrapping_add(1);
        }
        self.inner.notify();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if state.saves.is_empty() {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage save worker",
                    });
                }
            }
            changed.changed().await.map_err(|_| Error::Closing {
                domain: "storage save",
            })?;
        }
    }
}

impl State {
    pub(super) fn data_target(&self, through: &StorageCursor) -> Result<Target> {
        if !self.status.accepted.covers(through)? {
            return Err(Error::InvalidInput {
                field: "storage cursor",
                reason: "cursor is ahead of admission",
            });
        }
        if self.status.saved.covers(through)? {
            return Ok(Target {
                index: self.data_flushed,
                sequence: self.status.saved.sequence,
            });
        }
        let entry = self
            .queue
            .iter()
            .find(|entry| {
                entry.operation.domain() == Domain::Data
                    && entry.operation.sequence() >= through.sequence
            })
            .ok_or(Error::InvalidInput {
                field: "storage cursor",
                reason: "admitted cursor has no pending owner",
            })?;
        Ok(Target {
            index: entry.index,
            sequence: entry.operation.sequence(),
        })
    }

    pub(super) fn request_saves(&mut self) {
        for save in self.saves.values().filter(|save| save.failure.is_none()) {
            let target = save.work.target;
            if target.index > self.data_flushed
                && self
                    .data_request
                    .is_none_or(|current| target.index < current.index)
            {
                self.data_request = Some(target);
            }
        }
    }

    pub(super) fn save_action(&self) -> Option<Arc<SaveWork>> {
        let save = self
            .saves
            .values()
            .find(|save| save.failure.is_none() && save.work.target.index <= self.data_flushed)?;
        Some(save.work.clone())
    }

    pub(super) fn saving_data(&self, index: u64) -> bool {
        self.saves
            .values()
            .any(|save| save.failure.is_none() && save.work.target.index >= index)
    }

    pub(super) fn save_failed(&mut self, mutation: &MutationId, source: Arc<Error>) {
        tracing::error!(window = %mutation.window, mutation = %mutation.key, error = %source, reason = "save persistence failed", "storage save retained for retry");
        if let Some(save) = self.saves.get_mut(mutation) {
            save.failure = Some(source);
        }
    }

    pub(super) fn save_data_failed(&mut self, index: u64, source: Arc<Error>) {
        for save in self
            .saves
            .values_mut()
            .filter(|save| save.work.target.index >= index)
        {
            tracing::error!(window = %save.work.receipt.mutation.window, mutation = %save.work.receipt.mutation.key, index, error = %source, reason = "data persistence failed before save", "storage save retained for retry");
            save.failure = Some(source.clone());
        }
    }
}

impl SaveWork {
    pub(super) async fn perform(&self, inner: &Arc<Inner>) -> Result<()> {
        self.files.persist(inner).await?;
        inner
            .bounded(
                inner
                    .primary
                    .save_mutations(std::slice::from_ref(&self.receipt)),
            )
            .await
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn fingerprint(through: &StorageCursor) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"storage.save:v1\0");
    for bytes in [
        through.history.store.as_str().as_bytes(),
        through.history.epoch.as_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.update(through.sequence.to_le_bytes());
    hash.finalize().into()
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "storage save",
        reason,
    }
}
fn exhausted() -> Error {
    Error::ResourceExhausted {
        resource: "storage save requests",
    }
}

/* === Tests === */
