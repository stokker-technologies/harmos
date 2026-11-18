/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod append;
mod create;
mod end;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::mem;

// Third Party Imports

// Local Imports
use super::*;
use crate::MemorySize;
use crate::manager::Inner;
use crate::receipts::ReceiptState;
use crate::{FileProducer, MutationId, MutationValue};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Mutation {
    pub(super) id: MutationId,
    pub(super) kind: Kind,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Create,
    Append,
    Resume,
    Finish,
    Abandon,
}

/// Outcome of One Admission Attempt of a File Mutation.
pub(super) enum Admit {
    /// The Command Owns Its Receipt and Budget Share; the Worker Runs It.
    Queued,
    /// The File Still Runs Another Command; Retry Once It Settles.
    Busy,
    /// The Shared Write Budget Is Full; Wait for Capacity in FIFO Order.
    Full,
}

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    #[tracing::instrument(name = "storage.staging.mutations.resume_file_once", level = "debug", skip_all)]
    pub async fn resume_file_once(&self, mutation: &MutationId, token: &FileSession) -> Result<FileProducer> {
        self.inner.primary.get_history().check(&token.history)?;
        let next = token.generation.checked_add(1).ok_or_else(size_error)?;
        let fingerprint = fingerprint("storage.resume_file:v1", token);
        self.mutate_file(mutation, fingerprint, |state| {
            let (id, file) = active(state, token)?;
            if matches!(file.command, Some(Command::Finish { .. } | Command::Abort | Command::Resume { .. })) {
                return Err(invalid("production is ending or another resume owns it"));
            }
            if file.command.is_some() || file.in_flight {
                return Ok(Admit::Busy);
            }
            if !reserve(&self.inner, state, mutation, fingerprint, 0)? {
                return Ok(Admit::Full);
            }
            let file = state.files.get_mut(&id).expect("selected file");
            file.mutation = Some(Mutation { id: mutation.clone(), kind: Kind::Resume });
            file.command = Some(Command::Resume { expected: token.generation, next });
            file.progress.lock().unwrap().generation = next;
            Ok(Admit::Queued)
        })
        .await?
        .producer()
    }

    /// Admits One File Mutation, Waiting While the Shared Write Budget Is Full.
    ///
    /// A full budget applies back-pressure: waiters hold a FIFO turn, so a large command is never starved by smaller
    /// ones, and the caller's operation deadline bounds the wait by dropping this future, which releases the turn.
    /// Only a command larger than the whole budget fails at once.
    #[tracing::instrument(name = "storage.staging.mutations.mutate_file", level = "debug", skip_all)]
    async fn mutate_file(
        &self, mutation: &MutationId, fingerprint: [u8; 32], mut admit: impl FnMut(&mut State) -> Result<Admit>,
    ) -> Result<MutationValue> {
        if !self.inner.primary.capabilities().mutation_receipts {
            return Err(invalid("backend does not support mutation receipts"));
        }
        let resolution = self.resolve_mutation(mutation, fingerprint).await?;
        let mut retry = true;
        let mut completion = Some(resolution.completion.clone());
        let mut changed = self.inner.changed.subscribe();
        let mut turn = None;
        loop {
            let mut queued = false;
            let mut full = false;
            let mut take_turn = false;
            {
                let mut state = self.inner.state.lock().unwrap();
                if state.status.closing {
                    return Err(closed());
                }
                if !state.ready || !state.status.running {
                    return Err(Error::NotReady { domain: "storage mutations" });
                }
                state.expire_files();
                state.collect();
                match state.receipts.lookup(mutation, &fingerprint, completion.as_ref())? {
                    ReceiptState::Complete(value) => return Ok(value),
                    ReceiptState::Pending => {
                        if completion.is_none() {
                            let owner = state.receipts.completion(mutation);
                            completion = Some(owner);
                        }
                        let file = state
                            .files
                            .values_mut()
                            .find(|file| file.mutation.as_ref().is_some_and(|held| held.id == *mutation))
                            .ok_or_else(|| invalid("pending receipt lost its file owner"))?;
                        let retrying = file.retrying || file.in_flight;
                        let failure = file.progress.lock().unwrap().failure.clone();
                        let progress = file.progress.clone();
                        if !retrying {
                            if let Some(source) = &state.status.failure
                                && !failure.as_ref().is_some_and(|own| Arc::ptr_eq(source, own))
                            {
                                return Err(Error::Persistence { source: source.clone() });
                            }
                            if let Some(source) = failure {
                                if !retry {
                                    return Err(Error::Persistence { source });
                                }
                                progress.lock().unwrap().failure = None;
                                state.status.failure = None;
                                state.retry = state.retry.wrapping_add(1);
                                queued = true;
                            }
                        }
                        retry = false;
                    }
                    ReceiptState::Missing => {
                        admission(&state)?;
                        if turn.is_none() {
                            // Even a Command That Fits Queues Behind Earlier Waiters.
                            take_turn = true;
                        } else {
                            match admit(&mut state)? {
                                Admit::Queued => {
                                    queued = true;
                                    let owner = state.receipts.completion(mutation);
                                    completion = Some(owner);
                                    retry = false;
                                }
                                Admit::Busy => {}
                                Admit::Full => full = true,
                            }
                        }
                    }
                }
                state.collect();
            }
            if take_turn {
                turn = Some(self.inner.file_mutation_admission.lock().await);
                continue;
            }
            if !full {
                turn = None;
            }
            if queued {
                self.inner.notify();
            }
            changed.changed().await.map_err(|_| closed())?;
        }
    }
}

impl MemorySize for Mutation {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>() + self.id.memory_size() - mem::size_of::<MutationId>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn fingerprint(domain: &str, session: &FileSession) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash_part(&mut hash, domain.as_bytes());
    hash_session(&mut hash, session);
    hash.finalize().into()
}

fn hash_part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hash_session(hash: &mut Sha256, session: &FileSession) {
    hash_part(hash, session.history.store.as_str().as_bytes());
    hash_part(hash, session.history.epoch.as_str().as_bytes());
    hash_part(hash, session.id.as_str().as_bytes());
    hash.update(session.generation.to_le_bytes());
}

fn active<'a>(state: &'a State, token: &FileSession) -> Result<(u64, &'a FileState)> {
    let (id, file) =
        state.files.iter().find(|(_, file)| file.staging.id == token.id).ok_or_else(|| invalid("session unavailable or expired"))?;
    let session = file.session.as_ref().ok_or_else(|| invalid("private staging is not resumable"))?;
    if session.generation != token.generation || file.abandoned {
        tracing::error!(file = %token.id, generation = token.generation, reason = "expired or fenced producer", "file mutation rejected");
        return Err(invalid("expired or fenced producer"));
    }
    Ok((*id, file))
}

/// Reserves One Command's Share of the Shared Write Budget; False Means Wait for Capacity.
fn reserve(inner: &Inner, state: &mut State, mutation: &MutationId, fingerprint: [u8; 32], bytes: usize) -> Result<bool> {
    let budget = inner.settings.request.max_pending_write_bytes.get();
    if bytes > budget {
        return Err(invalid(
            "one command exceeds the whole storage write budget; raise request.max_pending_write_bytes or send smaller chunks",
        ));
    }
    if bytes > budget.saturating_sub(state.status.pending_bytes)
        || state.status.pending_batches >= inner.settings.request.max_pending_write_batches.get()
    {
        tracing::debug!(
            bytes,
            pending_bytes = state.status.pending_bytes,
            pending_batches = state.status.pending_batches,
            "file mutation waits for storage write capacity"
        );
        inner.relieve_pressure(state, bytes, 1);
        return Ok(false);
    }
    state.receipts.admit(mutation, fingerprint)?;
    Ok(true)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "file mutation", reason }
}

/* === Tests === */
