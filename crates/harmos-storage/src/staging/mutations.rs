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

// Third Party Imports

// Local Imports
use super::*;
use crate::MemorySize;
use crate::storage::receipts::ReceiptState;
use crate::storage::{FileProducer, MutationId, MutationValue};

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

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.staging.mutations.resume_file_once", level = "debug", skip_all)]
    pub async fn resume_file_once(&self, mutation: &MutationId, token: &FileSession) -> Result<FileProducer> {
        self.inner.primary.history().check(&token.history)?;
        let next = token.generation.checked_add(1).ok_or_else(size_error)?;
        let fingerprint = fingerprint("storage.resume_file:v1", token);
        self.mutate_file(mutation, fingerprint, |state| {
            let (id, file) = active(state, token)?;
            if matches!(file.command, Some(Command::Finish { .. } | Command::Abort | Command::Resume { .. })) {
                return Err(invalid("production is ending or another resume owns it"));
            }
            if file.command.is_some() || file.in_flight {
                return Ok(false);
            }
            let outcome =
                MutationValue::Producer(FileProducer { session: FileSession { generation: next, ..token.clone() }, length: file.length });
            reserve(state, mutation, fingerprint, outcome.memory_size(), 0, &self.inner.settings)?;
            let file = state.files.get_mut(&id).expect("selected file");
            file.mutation = Some(Mutation { id: mutation.clone(), kind: Kind::Resume });
            file.command = Some(Command::Resume { expected: token.generation, next });
            file.progress.lock().unwrap().generation = next;
            Ok(true)
        })
        .await?
        .producer()
    }

    #[tracing::instrument(name = "storage.staging.mutations.mutate_file", level = "debug", skip_all)]
    async fn mutate_file(
        &self, mutation: &MutationId, fingerprint: [u8; 32], mut admit: impl FnMut(&mut State) -> Result<bool>,
    ) -> Result<MutationValue> {
        if !self.inner.primary.supports_mutation_receipts() {
            return Err(invalid("backend does not support mutation receipts"));
        }
        let mut retry = true;
        let mut completion = None;
        let mut changed = self.inner.changed.subscribe();
        loop {
            let mut queued = false;
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
                        queued = admit(&mut state)?;
                        if queued {
                            let owner = state.receipts.completion(mutation);
                            completion = Some(owner);
                            retry = false;
                        }
                    }
                }
                state.collect();
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
        std::mem::size_of::<Self>() + self.id.memory_size() - std::mem::size_of::<MutationId>()
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

fn reserve(
    state: &mut State, mutation: &MutationId, fingerprint: [u8; 32], outcome_bytes: usize, command_bytes: usize, settings: &StorageSettings,
) -> Result<()> {
    let bytes = mutation
        .memory_size()
        .checked_add(std::mem::size_of::<Mutation>() - std::mem::size_of::<MutationId>())
        .and_then(|bytes| bytes.checked_add(command_bytes))
        .ok_or_else(size_error)?;
    if bytes > settings.request.max_pending_write_bytes.get().saturating_sub(state.status.pending_bytes)
        || state.status.pending_batches >= settings.request.max_pending_write_batches.get()
    {
        return Err(Error::ResourceExhausted { resource: "file mutation command" });
    }
    state.receipts.insert(mutation, fingerprint, outcome_bytes)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "file mutation", reason }
}

/* === Tests === */
