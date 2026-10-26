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

impl FileState {
    pub(crate) fn identifies(&self, file: &StagedFile) -> bool {
        self.staging == *file
    }

    pub(crate) fn production(&self, file: &StagedFile) -> Option<Result<FileProgress>> {
        if self.staging != *file {
            return None;
        }
        if self.session.is_none() {
            return Some(Err(Error::InvalidInput { field: "file following", reason: "private transfer staging is not readable" }));
        }
        if let Some(source) = &self.progress.lock().unwrap().failure {
            return Some(Err(Error::Persistence { source: source.clone() }));
        }
        Some(Ok(FileProgress { length: self.length, phase: FilePhase::Producing }))
    }

    pub(crate) fn bytes(&self) -> usize {
        self.base_bytes
            + self.mutation.as_ref().map_or(0, crate::MemorySize::memory_size)
            + match &self.command {
                Some(Command::Write(bytes)) if !bytes.charged() => bytes.allocation_bytes(),
                _ => 0,
            }
    }

    pub(crate) fn pending_batch(&self) -> bool {
        self.command.is_some()
    }
}

impl Drop for FileState {
    fn drop(&mut self) {
        if let Some(Command::Write(bytes)) = &self.command {
            bytes.retire();
        }
    }
}

impl FileAction {
    pub(crate) fn caller_retries(&self, state: &State) -> bool {
        let Some(mutation) = state.files.get(&self.id()).and_then(|file| file.mutation.as_ref()) else {
            return false;
        };
        // Inactivity Cleanup Retains The Worker's Own Retry Policy.
        !matches!(self, Self::Abandon { .. }) || mutation.kind == mutations::Kind::Abandon
    }

    pub(crate) fn id(&self) -> u64 {
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
    pub(crate) fn file_action(&mut self, allow_finish: bool, selected: Option<u64>) -> Option<FileAction> {
        let ids: Vec<_> = self.files.keys().copied().collect();
        let id =
            ids.iter().copied().filter(|id| *id > self.file_cursor).chain(ids.iter().copied().filter(|id| *id <= self.file_cursor)).find(
                |id| {
                    let file = &self.files[id];
                    selected.is_none_or(|selected| selected == *id)
                        && !file.in_flight
                        && (allow_finish || !matches!(file.command, Some(Command::Finish { .. })))
                        && (file.abandoned || file.command.is_some())
                        && (!matches!(file.command, Some(Command::Finish { .. }))
                            || (self.reserved == 0 && self.finishing.is_none_or(|owner| owner == *id)))
                },
            )?;
        self.file_cursor = id;
        let file = self.files.get_mut(&id).unwrap();
        if file.abandoned && !matches!(file.command, Some(Command::Finish { .. })) {
            if let Some(Command::Write(bytes)) = &file.command {
                bytes.retire();
            }
            file.command = Some(Command::Abort);
        }
        let action = match file.command.as_mut()? {
            Command::Create => {
                FileAction::Create { id, metadata: file.metadata.clone(), staging: file.staging.clone(), resumable: file.session.is_some() }
            }
            Command::Resume { expected, next } => {
                FileAction::Resume { id, staging: file.staging.clone(), expected: *expected, next: *next }
            }
            Command::Write(bytes) => FileAction::Write { id, staging: file.staging.clone(), offset: file.length, bytes: bytes.clone() },
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
            Command::Abort if file.session.is_some() => {
                FileAction::Abandon { id, staging: file.staging.clone(), extra: file.extra.clone() }
            }
            Command::Abort => FileAction::Abort { id, staging: file.staging.clone(), extra: file.extra.clone() },
        };
        file.in_flight = true;
        self.collect();
        Some(action)
    }

    pub(crate) fn file_failed(&mut self, action: &FileAction, source: Arc<Error>, retrying: bool) {
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
                if let Some(Command::Write(bytes)) = &file.command {
                    bytes.retire();
                }
                file.command = Some(Command::Abort);
                file.abandoned = true;
            }
        }
    }

    pub(crate) fn file_completed(
        &mut self, action: FileAction, completed: FileCompleted, session_timeout: Duration, inner: &Arc<crate::manager::Inner>,
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
                        Some(Command::Write(bytes)) if matches!(completed, FileCompleted::Written) => file.length + bytes.len() as u64,
                        _ => file.length,
                    };
                    crate::MutationValue::Producer(crate::FileProducer {
                        session: FileSession { history: file.staging.history.clone(), id: file.staging.id.clone(), generation },
                        length,
                    })
                }
                (FileCompleted::Finished(reference), Kind::Finish) => crate::MutationValue::Reference(reference.clone()),
                (FileCompleted::Aborted, Kind::Abandon) => {
                    crate::MutationValue::Progress(FileProgress { length: file.length, phase: FilePhase::Abandoned })
                }
                _ => crate::MutationValue::Rejected,
            }
        });
        let mut progress = file.progress.lock().unwrap();
        progress.failure = None;
        let remove = matches!(completed, FileCompleted::Finished(_) | FileCompleted::Aborted);
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
                let Some(Command::Write(bytes)) = file.command.take() else { unreachable!("one owned batch") };
                bytes.retire();
                file.hash.update(bytes.as_slice());
                file.length += bytes.len() as u64;
                progress.length = file.length;
                progress.completed = file.ticket;
            }
            FileCompleted::Finished(reference) => {
                let reference = *reference;
                let FileAction::Finish { target, .. } = action else { unreachable!("one finish owner") };
                self.status.accepted = reference.position.clone();
                self.next = target.index;
                self.data_accepted = target;
                self.finishing = None;
                self.queue.push_back(Arc::new(Pending::new(
                    inner,
                    None,
                    target.index,
                    file.reference_bytes,
                    Operation::File(crate::StorageBatch::new(vec![reference.clone()])),
                )));
                progress.reference = Some(reference);
                progress.closed = true;
            }
            FileCompleted::Aborted => {
                progress.closed = true;
            }
        }
        drop(progress);
        if let Some(mutation) = mutation {
            self.receipts.complete(&mutation.id, outcome.expect("receipt outcome"));
        }
        if remove {
            self.files.remove(&id);
        }
        self.collect();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::{Data, Recovery, history, storage_settings};

    fn storage() -> crate::StorageManager {
        crate::StorageManager::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            storage_settings(),
        )
        .unwrap()
    }

    fn file(inner: &Arc<crate::manager::Inner>) -> FileState {
        let metadata = FileMetadata {
            id: crate::Identifier::new("charged-file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        };
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(b"body");
        let charge = crate::batch::WriteCharge::new(inner, bytes.capacity());
        FileState {
            staging: StagedFile { history: history(), id: metadata.id.clone() },
            metadata: Arc::new(metadata),
            session: None,
            mutation: None,
            extra: None,
            progress: Arc::new(Mutex::new(Progress::default())),
            hash: Sha256::new(),
            length: 0,
            ticket: 1,
            command: Some(Command::Write(crate::FileChunk::new(bytes).retained(charge))),
            abandoned: false,
            in_flight: false,
            retrying: false,
            base_bytes: 1024,
            reference_bytes: 256,
        }
    }

    #[test]
    fn command_retention() {
        let storage = storage();
        let (failed, detached) = {
            let mut state = storage.inner.state.lock().unwrap();
            state.files.insert(1, file(&storage.inner));
            state.collect();
            assert_eq!(state.status.pending_bytes, 1152);
            assert_eq!(state.retained_write_bytes.load(std::sync::atomic::Ordering::SeqCst), 0);
            let action = state.file_action(true, None).unwrap();
            let FileAction::Write { bytes, .. } = &action else { unreachable!() };
            let detached = bytes.slice(1..2);
            state.file_failed(&action, Arc::new(Error::NotReady { domain: "injected write" }), false);
            state.collect();
            assert_eq!(state.status.pending_bytes, 1152);
            let retry = state.file_action(true, None).unwrap();
            state.file_completed(retry, FileCompleted::Written, Duration::from_secs(60), &storage.inner);
            state.collect();
            assert_eq!(state.files[&1].length, 4);
            assert_eq!(state.status.pending_bytes, 1152);
            (action, detached)
        };
        drop(failed);
        assert_eq!(storage.status().pending_bytes, 1152);
        drop(detached);
        assert_eq!(storage.status().pending_bytes, 1024);
        storage.inner.state.lock().unwrap().files.clear();
        assert_eq!(storage.status().pending_bytes, 0);
    }

    #[test]
    fn shutdown_retention() {
        let storage = storage();
        let detached = {
            let mut state = storage.inner.state.lock().unwrap();
            let mut file = file(&storage.inner);
            let Some(Command::Write(bytes)) = &file.command else { unreachable!() };
            let detached = bytes.slice(0..0);
            file.abandoned = true;
            state.files.insert(1, file);
            state.status.closing = true;
            let abort = state.file_action(true, None).unwrap();
            state.file_completed(abort, FileCompleted::Aborted, Duration::from_secs(60), &storage.inner);
            state.status.closed = true;
            state.collect();
            assert_eq!(state.status.pending_bytes, 128);
            detached
        };
        assert_eq!(storage.status().pending_bytes, 128);
        drop(detached);
        assert_eq!(storage.status().pending_bytes, 0);
    }
}
