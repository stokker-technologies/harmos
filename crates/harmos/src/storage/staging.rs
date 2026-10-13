/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(super) mod checkpoint;
mod mutations;
mod session;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::manager::{ENTRY_OVERHEAD, Operation, Pending, State, Target, encoded_size};
use super::*;
use crate::supervision::executor;
use crate::{Error, Result};
use session::Session;

/* === Constants === */
static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct StorageFile {
    manager: StorageManager<super::manager::ErasedCatalog>,
    id: u64,
    progress: Arc<Mutex<Progress>>,
    session: Option<FileSession>,
}

struct CancelProduction {
    manager: StorageManager<super::manager::ErasedCatalog>,
    id: u64,
    armed: bool,
    generation: Option<u64>,
}

#[derive(Default)]
struct Progress {
    generation: u64,
    ready: bool,
    closed: bool,
    completed: u64,
    length: u64,
    reference: Option<FileReference>,
    failure: Option<Arc<Error>>,
}

pub(super) struct FileState {
    metadata: Arc<FileMetadata>,
    staging: StagedFile,
    session: Option<Session>,
    mutation: Option<mutations::Mutation>,
    extra: Option<StagedFile>,
    progress: Arc<Mutex<Progress>>,
    hash: Sha256,
    length: u64,
    ticket: u64,
    command: Option<Command>,
    abandoned: bool,
    in_flight: bool,
    retrying: bool,
    base_bytes: usize,
    reference_bytes: usize,
}

/* === Enums === */
enum Command {
    Create,
    Resume {
        expected: u64,
        next: u64,
    },
    Write(Arc<[u8]>),
    Finish {
        target: Option<Target>,
        sha256: [u8; 32],
    },
    Abort,
}

enum Await {
    Ready,
    Written(u64),
    Finished,
    Aborted,
}

pub(super) enum FileAction {
    Create {
        id: u64,
        metadata: Arc<FileMetadata>,
        staging: StagedFile,
        resumable: bool,
    },
    Resume {
        id: u64,
        staging: StagedFile,
        expected: u64,
        next: u64,
    },
    Write {
        id: u64,
        staging: StagedFile,
        offset: u64,
        bytes: Arc<[u8]>,
    },
    Finish {
        id: u64,
        staging: StagedFile,
        metadata: Arc<FileMetadata>,
        target: Target,
        length: u64,
        sha256: [u8; 32],
    },
    Abandon {
        id: u64,
        staging: StagedFile,
        extra: Option<StagedFile>,
    },
    Abort {
        id: u64,
        staging: StagedFile,
        extra: Option<StagedFile>,
    },
    Refuse {
        id: u64,
    },
}

pub(super) enum FileCompleted {
    Created,
    Resumed(u64),
    Written,
    Finished(Box<FileReference>),
    Aborted,
}

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn create_file(&self, metadata: FileMetadata) -> Result<StorageFile> {
        self.create(metadata, false).await
    }

    async fn create(&self, metadata: FileMetadata, resumable: bool) -> Result<StorageFile> {
        let (reference_bytes, base_bytes) = file_bytes(&metadata, self.inner.primary.history())?;
        if base_bytes >= self.inner.settings.queue_capacity_bytes.get() {
            return Err(Error::InvalidInput {
                field: "file metadata",
                reason: "metadata exceeds memory budget",
            });
        }
        // The charged envelope includes worst-case position, length and digest
        // metadata. Refuse before creating bytes that cannot later be published.
        if reference_bytes > self.inner.settings.max_batch_bytes.get() {
            return Err(Error::InvalidInput {
                field: "file metadata",
                reason: "reference envelope exceeds max_batch_bytes",
            });
        }
        let mut changed = self.inner.changed.subscribe();
        let progress = Arc::new(Mutex::new(Progress {
            generation: u64::from(resumable),
            ..Progress::default()
        }));
        let token = resumable.then(|| FileSession {
            history: self.inner.primary.history().clone(),
            id: metadata.id.clone(),
            generation: 1,
        });
        let id = loop {
            {
                let mut state = self.inner.state.lock().unwrap();
                admission(&state)?;
                if state
                    .files
                    .values()
                    .any(|file| file.metadata.id == metadata.id)
                    || state.deleting(&metadata.id)
                {
                    return Err(Error::AlreadyRegistered {
                        domain: "storage file",
                        name: metadata.id.to_string(),
                    });
                }
                state.collect();
                // Leave at least one byte for chunk progress even with tiny policies.
                if state.files.len() < self.inner.settings.max_files.get()
                    && state.status.pending_batches < self.inner.settings.queue_capacity.get()
                    && state.status.pending_bytes
                        < self.inner.settings.queue_capacity_bytes.get() - base_bytes
                {
                    let id = NEXT_FILE
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                        .map_err(|_| size_error())?;
                    let staging = StagedFile {
                        history: self.inner.primary.history().clone(),
                        id: metadata.id.clone(),
                    };
                    state.files.insert(
                        id,
                        FileState {
                            metadata: Arc::new(metadata),
                            staging,
                            session: resumable.then(Session::new),
                            extra: None,
                            mutation: None,
                            progress: progress.clone(),
                            hash: Sha256::new(),
                            length: 0,
                            ticket: 0,
                            command: Some(Command::Create),
                            abandoned: false,
                            in_flight: false,
                            retrying: false,
                            base_bytes,
                            reference_bytes,
                        },
                    );
                    state.collect();
                    break id;
                }
                self.inner.relieve_pressure(&mut state, base_bytes + 1, 1);
            }
            changed.changed().await.map_err(|_| closed())?;
        };
        let mut cancel = CancelProduction {
            manager: self.erased(),
            id,
            armed: true,
            generation: token.as_ref().map(|token| token.generation),
        };
        let file = StorageFile {
            manager: self.erased(),
            id,
            progress,
            session: token,
        };
        self.inner.notify();
        file.wait(Await::Ready).await?;
        cancel.armed = false;
        Ok(file)
    }

    pub async fn write_file(&self, metadata: FileMetadata, bytes: &[u8]) -> Result<FileReference> {
        let mut file = self.create_file(metadata).await?;
        file.write_all(bytes).await?;
        file.finish().await
    }

    pub(super) async fn settle_files(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.status.failure.take().is_some() {
                state.retry = state.retry.wrapping_add(1);
            }
            for file in state.files.values_mut() {
                file.retrying = false;
                file.progress.lock().unwrap().failure = None;
                if (file.session.is_none() || !file.progress.lock().unwrap().ready)
                    && !matches!(file.command, Some(Command::Finish { .. }))
                {
                    file.abandoned = true;
                    if let Some(session) = &mut file.session {
                        session.reason = "storage closing";
                    }
                    file.progress.lock().unwrap().closed = true;
                }
            }
        }
        self.inner.notify();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if state.files.values().all(|file| {
                    file.session.is_some()
                        && file.command.is_none()
                        && !file.in_flight
                        && !file.abandoned
                }) {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
            }
            changed.changed().await.map_err(|_| closed())?;
        }
    }

    fn cancel_file(&self, id: u64, generation: Option<u64>) {
        let mut state = self.inner.state.lock().unwrap();
        let mut clear = false;
        let failure = state.status.failure.clone();
        if let Some(file) = state.files.get_mut(&id) {
            if let Some(session) = &mut file.session {
                if generation == Some(session.generation) && !file.abandoned {
                    session.disconnect(
                        &file.staging,
                        file.length,
                        self.inner.settings.file_session_timeout,
                    );
                }
                self.inner.notify();
                return;
            }
            file.abandoned = true;
            if !matches!(file.command, Some(Command::Finish { .. })) {
                let mut progress = file.progress.lock().unwrap();
                progress.closed = true;
                clear = failure
                    .as_ref()
                    .zip(progress.failure.as_ref())
                    .is_some_and(|(a, b)| Arc::ptr_eq(a, b));
            }
        }
        if clear {
            state.status.failure = None;
            state.retry = state.retry.wrapping_add(1);
        }
        self.inner.notify();
    }
}

impl StorageFile {
    pub fn length(&self) -> u64 {
        self.progress.lock().unwrap().length
    }

    pub async fn write_all(&mut self, mut bytes: &[u8]) -> Result<()> {
        let mut cancel = CancelProduction {
            manager: self.manager.clone(),
            id: self.id,
            armed: true,
            generation: self.session.as_ref().map(|token| token.generation),
        };
        let mut changed = self.manager.inner.changed.subscribe();
        loop {
            let admitted = {
                let mut state = self.manager.inner.state.lock().unwrap();
                admission(&state)?;
                state.collect();
                let available = self
                    .manager
                    .inner
                    .settings
                    .queue_capacity_bytes
                    .get()
                    .saturating_sub(state.status.pending_bytes);
                let capacity =
                    state.status.pending_batches < self.manager.inner.settings.queue_capacity.get();
                let file = state.files.get_mut(&self.id).ok_or_else(closed)?;
                file.check_session(self.session.as_ref())?;
                if file.abandoned || file.command.is_some() {
                    return Err(closed());
                }
                let total = file
                    .length
                    .checked_add(u64::try_from(bytes.len()).map_err(|_| size_error())?)
                    .ok_or(size_error())?;
                if file
                    .metadata
                    .expected_length
                    .is_some_and(|expected| total > expected)
                {
                    return Err(Error::InvalidInput {
                        field: "file length",
                        reason: "input exceeds expected length",
                    });
                }
                if bytes.is_empty() {
                    cancel.armed = false;
                    return Ok(());
                }
                if available != 0 && capacity {
                    let count = bytes
                        .len()
                        .min(available)
                        .min(self.manager.inner.settings.file_chunk_bytes.get());
                    file.ticket = file.ticket.checked_add(1).ok_or(size_error())?;
                    file.command = Some(Command::Write(Arc::from(&bytes[..count])));
                    let ticket = file.ticket;
                    state.collect();
                    Some((count, ticket))
                } else {
                    self.manager.inner.relieve_pressure(&mut state, 1, 1);
                    None
                }
            };
            if let Some((count, ticket)) = admitted {
                self.manager.inner.notify();
                self.wait(Await::Written(ticket)).await?;
                bytes = &bytes[count..];
            } else {
                changed.changed().await.map_err(|_| closed())?;
            }
        }
    }

    pub async fn finish(&mut self) -> Result<FileReference> {
        self.check_generation()?;
        if let Some(reference) = &self.progress.lock().unwrap().reference {
            return Ok(reference.clone());
        }
        let mut changed = self.manager.inner.changed.subscribe();
        loop {
            let replica_ceiling = self.manager.inner.replica_ceiling();
            let ready = {
                let mut state = self.manager.inner.state.lock().unwrap();
                if state.status.closing {
                    return Err(closed());
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
                state.collect();
                let capacity =
                    state.status.pending_batches < self.manager.inner.settings.queue_capacity.get();
                let lag_full = state.status.accepted.sequence >= replica_ceiling;
                let file = state.files.get_mut(&self.id).ok_or_else(closed)?;
                if let Some(Command::Finish { .. }) = file.command {
                    file.progress.lock().unwrap().failure = None;
                    file.retrying = false;
                    state.status.failure = None;
                    state.retry = state.retry.wrapping_add(1);
                    true
                } else {
                    file.check_session(self.session.as_ref())?;
                    if file.abandoned || file.command.is_some() {
                        return Err(closed());
                    }
                    let digest: [u8; 32] = file.hash.clone().finalize().into();
                    if file
                        .metadata
                        .expected_length
                        .is_some_and(|length| length != file.length)
                        || file
                            .metadata
                            .expected_sha256
                            .is_some_and(|expected| expected != digest)
                    {
                        file.abandoned = true;
                        file.progress.lock().unwrap().closed = true;
                        self.manager.inner.notify();
                        return Err(Error::InvalidInput {
                            field: "file integrity",
                            reason: "length or digest does not match",
                        });
                    }
                    if lag_full {
                        return Err(Error::ResourceExhausted {
                            resource: "replica retained history",
                        });
                    }
                    if capacity {
                        file.command = Some(Command::Finish {
                            target: None,
                            sha256: digest,
                        });
                        state.collect();
                        true
                    } else {
                        self.manager.inner.relieve_pressure(&mut state, 0, 1);
                        false
                    }
                }
            };
            if ready {
                break;
            }
            changed.changed().await.map_err(|_| closed())?;
        }
        self.manager.inner.notify();
        self.wait(Await::Finished).await?;
        self.progress
            .lock()
            .unwrap()
            .reference
            .clone()
            .ok_or_else(closed)
    }

    pub async fn abort(&mut self) -> Result<()> {
        self.check_generation()?;
        {
            let mut state = self.manager.inner.state.lock().unwrap();
            if let Some(file) = state.files.get_mut(&self.id) {
                if matches!(
                    file.command,
                    Some(Command::Finish {
                        target: Some(_),
                        ..
                    })
                ) {
                    return Err(Error::InvalidInput {
                        field: "file abort",
                        reason: "finish is already being reconciled",
                    });
                }
                file.abandoned = true;
                if let Some(session) = &mut file.session {
                    session.reason = "producer explicitly abandoned";
                }
                if !file.in_flight {
                    file.command = Some(Command::Abort);
                }
                let mut progress = file.progress.lock().unwrap();
                progress.closed = true;
                let failure = progress.failure.take();
                drop(progress);
                file.retrying = false;
                if state
                    .status
                    .failure
                    .as_ref()
                    .zip(failure.as_ref())
                    .is_some_and(|(current, previous)| Arc::ptr_eq(current, previous))
                {
                    state.status.failure = None;
                    state.retry = state.retry.wrapping_add(1);
                }
                state.collect();
            } else {
                return Ok(());
            }
        }
        self.manager.inner.notify();
        self.wait(Await::Aborted).await
    }

    async fn wait(&self, goal: Await) -> Result<()> {
        let mut changed = self.manager.inner.changed.subscribe();
        loop {
            self.check_generation()?;
            {
                let progress = self.progress.lock().unwrap();
                let done = match goal {
                    Await::Ready => progress.ready,
                    Await::Written(ticket) => progress.completed >= ticket,
                    Await::Finished => progress.reference.is_some(),
                    Await::Aborted => false,
                };
                if done {
                    return Ok(());
                }
                if let Some(source) = &progress.failure {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                if progress.closed && !matches!(goal, Await::Finished | Await::Aborted) {
                    return Err(closed());
                }
            }
            {
                let state = self.manager.inner.state.lock().unwrap();
                let held = state.files.get(&self.id);
                if matches!(goal, Await::Aborted) && held.is_none() {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure
                    && !held.is_some_and(|file| file.retrying)
                {
                    return Err(Error::Persistence {
                        source: source.clone(),
                    });
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
                if state.status.closing && !matches!(goal, Await::Finished | Await::Aborted) {
                    return Err(closed());
                }
            }
            changed.changed().await.map_err(|_| closed())?;
        }
    }
}

impl Drop for StorageFile {
    fn drop(&mut self) {
        self.manager
            .cancel_file(self.id, self.session.as_ref().map(|token| token.generation));
    }
}

impl Drop for CancelProduction {
    fn drop(&mut self) {
        if self.armed {
            self.manager.cancel_file(self.id, self.generation);
        }
    }
}

impl FileState {
    pub(super) fn identifies(&self, file: &StagedFile) -> bool {
        self.staging == *file
    }

    pub(super) fn production(&self, file: &StagedFile) -> Option<Result<FileProgress>> {
        if self.staging != *file {
            return None;
        }
        if self.session.is_none() {
            return Some(Err(Error::InvalidInput {
                field: "file following",
                reason: "private transfer staging is not readable",
            }));
        }
        if let Some(source) = &self.progress.lock().unwrap().failure {
            return Some(Err(Error::Persistence {
                source: source.clone(),
            }));
        }
        Some(Ok(FileProgress {
            length: self.length,
            phase: FilePhase::Producing,
        }))
    }

    pub(super) fn bytes(&self) -> usize {
        self.base_bytes
            + self
                .mutation
                .as_ref()
                .map_or(0, crate::MemorySize::memory_size)
            + match &self.command {
                Some(Command::Write(bytes)) => bytes.len(),
                _ => 0,
            }
    }

    pub(super) fn pending_batch(&self) -> bool {
        self.command.is_some()
    }
}

impl FileAction {
    pub(super) fn caller_retries(&self, state: &State) -> bool {
        let Some(mutation) = state
            .files
            .get(&self.id())
            .and_then(|file| file.mutation.as_ref())
        else {
            return false;
        };
        // Inactivity Cleanup Retains The Worker's Own Retry Policy.
        !matches!(self, Self::Abandon { .. }) || mutation.kind == mutations::Kind::Abandon
    }

    pub(super) fn id(&self) -> u64 {
        match self {
            Self::Create { id, .. }
            | Self::Resume { id, .. }
            | Self::Write { id, .. }
            | Self::Finish { id, .. }
            | Self::Abandon { id, .. }
            | Self::Abort { id, .. }
            | Self::Refuse { id } => *id,
        }
    }
}

impl State {
    pub(super) fn file_action(&mut self, replica_ceiling: u64) -> Option<FileAction> {
        let ids: Vec<_> = self.files.keys().copied().collect();
        let id = ids
            .iter()
            .copied()
            .filter(|id| *id > self.file_cursor)
            .chain(ids.iter().copied().filter(|id| *id <= self.file_cursor))
            .find(|id| {
                let file = &self.files[id];
                !file.in_flight
                    && (file.abandoned || file.command.is_some())
                    && (!matches!(file.command, Some(Command::Finish { target: None, .. }))
                        || self.status.accepted.sequence < replica_ceiling)
                    && (!matches!(file.command, Some(Command::Finish { .. }))
                        || (self.reserved == 0 && self.finishing.is_none_or(|owner| owner == *id)))
            })?;
        self.file_cursor = id;
        let file = self.files.get_mut(&id).unwrap();
        if file.abandoned && !matches!(file.command, Some(Command::Finish { .. })) {
            file.command = Some(Command::Abort);
        }
        let action = match file.command.as_mut()? {
            Command::Create => FileAction::Create {
                id,
                metadata: file.metadata.clone(),
                staging: file.staging.clone(),
                resumable: file.session.is_some(),
            },
            Command::Resume { expected, next } => FileAction::Resume {
                id,
                staging: file.staging.clone(),
                expected: *expected,
                next: *next,
            },
            Command::Write(bytes) => FileAction::Write {
                id,
                staging: file.staging.clone(),
                offset: file.length,
                bytes: bytes.clone(),
            },
            Command::Finish { target, sha256 } => {
                if target.is_none() {
                    let Some(index) = self.next.checked_add(1) else {
                        return Some(FileAction::Refuse { id });
                    };
                    let Some(sequence) = self.status.accepted.sequence.checked_add(1) else {
                        return Some(FileAction::Refuse { id });
                    };
                    *target = Some(Target { index, sequence });
                    self.finishing = Some(id);
                }
                FileAction::Finish {
                    id,
                    staging: file.staging.clone(),
                    metadata: file.metadata.clone(),
                    target: target.unwrap(),
                    length: file.length,
                    sha256: *sha256,
                }
            }
            Command::Abort if file.session.is_some() => FileAction::Abandon {
                id,
                staging: file.staging.clone(),
                extra: file.extra.clone(),
            },
            Command::Abort => FileAction::Abort {
                id,
                staging: file.staging.clone(),
                extra: file.extra.clone(),
            },
        };
        file.in_flight = true;
        self.collect();
        Some(action)
    }

    pub(super) fn file_failed(&mut self, action: &FileAction, source: Arc<Error>, retrying: bool) {
        if let Some(file) = self.files.get_mut(&action.id()) {
            if let Some(session) = &file.session {
                tracing::error!(file = %file.staging.id, generation = session.generation, offset = file.length, retrying, error = %source, reason = "storage failure", "file production failed");
            }
            file.in_flight = false;
            file.retrying = retrying;
            if !retrying {
                file.progress.lock().unwrap().failure = Some(source);
            }
            if matches!(action, FileAction::Refuse { .. }) {
                file.command = Some(Command::Abort);
                file.abandoned = true;
            }
        }
    }

    pub(super) fn file_completed(
        &mut self,
        action: FileAction,
        completed: FileCompleted,
        session_timeout: Duration,
    ) {
        let id = action.id();
        let Some(file) = self.files.get_mut(&id) else {
            return;
        };
        file.in_flight = false;
        file.retrying = false;
        if let Some(session) = &file.session {
            match &completed {
                FileCompleted::Finished(_) => {
                    tracing::info!(file = %file.staging.id, generation = session.generation, offset = file.length, reason = "producer finished", "file production completed")
                }
                FileCompleted::Aborted => {
                    tracing::info!(file = %file.staging.id, generation = session.generation, offset = file.length, reason = session.reason, "file production ended; partial bytes retained")
                }
                _ => {}
            }
        }
        let mutation = file.mutation.take();
        if mutation.is_some()
            && let Some(session) = &mut file.session
        {
            session.renew(session_timeout);
        }
        let outcome = mutation.as_ref().map(|mutation| {
            use mutations::Kind;
            match (&completed, mutation.kind) {
                (FileCompleted::Created, Kind::Create)
                | (FileCompleted::Written, Kind::Append)
                | (FileCompleted::Resumed(_), Kind::Resume) => {
                    let generation = match &completed {
                        FileCompleted::Resumed(generation) => *generation,
                        _ => file.session.as_ref().expect("managed file").generation,
                    };
                    let length = match &file.command {
                        Some(Command::Write(bytes))
                            if matches!(completed, FileCompleted::Written) =>
                        {
                            file.length + bytes.len() as u64
                        }
                        _ => file.length,
                    };
                    super::MutationValue::Producer(super::FileProducer {
                        session: FileSession {
                            history: file.staging.history.clone(),
                            id: file.staging.id.clone(),
                            generation,
                        },
                        length,
                    })
                }
                (FileCompleted::Finished(reference), Kind::Finish) => {
                    super::MutationValue::Reference(reference.clone())
                }
                (FileCompleted::Aborted, Kind::Abandon) => {
                    super::MutationValue::Progress(FileProgress {
                        length: file.length,
                        phase: FilePhase::Abandoned,
                    })
                }
                _ => super::MutationValue::Rejected,
            }
        });
        let mut progress = file.progress.lock().unwrap();
        progress.failure = None;
        let remove = matches!(
            completed,
            FileCompleted::Finished(_) | FileCompleted::Aborted
        );
        match completed {
            FileCompleted::Created => {
                file.command = None;
                progress.ready = true;
            }
            FileCompleted::Resumed(generation) => {
                file.command = None;
                let session = file.session.as_mut().expect("owned producer resume");
                session.resumed(generation, session_timeout, mutation.is_none());
                progress.generation = generation;
                tracing::info!(file = %file.staging.id, generation, offset = file.length, reason = "producer resumed", "file production resumed with durable fencing");
            }
            FileCompleted::Written => {
                let Some(Command::Write(bytes)) = file.command.take() else {
                    unreachable!("one owned chunk")
                };
                file.hash.update(&bytes);
                file.length += bytes.len() as u64;
                progress.length = file.length;
                progress.completed = file.ticket;
            }
            FileCompleted::Finished(reference) => {
                let reference = *reference;
                let FileAction::Finish { target, .. } = action else {
                    unreachable!("one finish owner")
                };
                self.status.accepted = reference.position.clone();
                self.next = target.index;
                self.data_accepted = target;
                self.finishing = None;
                self.queue.push_back(Arc::new(Pending {
                    journal: None,
                    index: target.index,
                    bytes: file.reference_bytes,
                    operation: Operation::File(reference.clone()),
                }));
                progress.reference = Some(reference);
                progress.closed = true;
            }
            FileCompleted::Aborted => {
                progress.closed = true;
            }
        }
        drop(progress);
        if let Some(mutation) = mutation {
            self.receipts
                .complete(&mutation.id, outcome.expect("receipt outcome"));
        }
        if remove {
            self.files.remove(&id);
        }
        self.collect();
    }
}

/* === Public Functions === */
pub(super) async fn perform(
    inner: &super::manager::Inner,
    action: &FileAction,
) -> Result<FileCompleted> {
    match action {
        FileAction::Create {
            id,
            metadata,
            staging,
            resumable,
        } => {
            let actual = if *resumable {
                inner
                    .bounded(inner.primary.create_production(metadata))
                    .await?
            } else {
                inner.bounded(inner.primary.create_file(metadata)).await?
            };
            if actual != *staging {
                if let Some(file) = inner.state.lock().unwrap().files.get_mut(id) {
                    file.extra = Some(actual);
                }
                return Err(Error::InvalidInput {
                    field: "file staging",
                    reason: "backend changed staging identity",
                });
            }
            Ok(FileCompleted::Created)
        }
        FileAction::Resume {
            staging,
            expected,
            next,
            ..
        } => {
            inner
                .bounded(inner.primary.fence_production(staging, *expected, *next))
                .await?;
            Ok(FileCompleted::Resumed(*next))
        }
        FileAction::Write {
            staging,
            offset,
            bytes,
            ..
        } => {
            inner
                .bounded(inner.primary.write_file(staging, *offset, bytes))
                .await?;
            Ok(FileCompleted::Written)
        }
        FileAction::Finish {
            staging,
            metadata,
            target,
            length,
            sha256,
            ..
        } => {
            let position = StorageCursor {
                history: inner.primary.history().clone(),
                sequence: target.sequence,
            };
            let reference = inner
                .bounded(
                    inner
                        .primary
                        .finish_file(staging, &position, *length, *sha256),
                )
                .await?;
            if reference.position != position
                || reference.metadata != **metadata
                || reference.length != *length
                || reference.sha256 != *sha256
            {
                return Err(Error::InvalidInput {
                    field: "file finish",
                    reason: "backend changed immutable file reference",
                });
            }
            Ok(FileCompleted::Finished(Box::new(reference)))
        }
        FileAction::Abandon { staging, extra, .. } => {
            if let Some(extra) = extra {
                inner.bounded(inner.primary.abandon_file(extra)).await?;
            }
            inner.bounded(inner.primary.abandon_file(staging)).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Abort { staging, extra, .. } => {
            if let Some(extra) = extra {
                inner.primary.abort_file(extra).await?;
            }
            inner.primary.abort_file(staging).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Refuse { .. } => Err(size_error()),
    }
}

/* === Private Functions === */
fn file_bytes(metadata: &FileMetadata, history: &StoreHistory) -> Result<(usize, usize)> {
    let reference = encoded_size(metadata)?
        .checked_add(encoded_size(history)?)
        .and_then(|size| size.checked_add(ENTRY_OVERHEAD))
        .ok_or_else(size_error)?;
    let base = reference.checked_mul(8).ok_or_else(size_error)?;
    Ok((reference, base))
}

fn closed() -> Error {
    Error::Closing {
        domain: "storage file",
    }
}

fn size_error() -> Error {
    Error::InvalidInput {
        field: "file admission",
        reason: "size or position overflow",
    }
}

fn admission(state: &State) -> Result<()> {
    if state.status.closing {
        return Err(closed());
    }
    if !state.ready || !state.status.running {
        return Err(Error::NotReady { domain: "storage" });
    }
    if let Some(source) = &state.status.failure {
        return Err(Error::Persistence {
            source: source.clone(),
        });
    }
    Ok(())
}

/* === Tests === */
