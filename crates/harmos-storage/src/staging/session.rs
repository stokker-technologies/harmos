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
use std::time::Duration;
// Third Party Imports
// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Session {
    execution: crate::execution::Execution,
    pub(super) generation: u64,
    disconnected: bool,
    deadline: Option<tokio::time::Instant>,
    pub(super) reason: &'static str,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Session {
    pub(super) fn new(execution: crate::execution::Execution) -> Self {
        Self { execution, generation: 1, disconnected: false, deadline: None, reason: "producer abandoned" }
    }

    pub(super) fn token(timeout: Duration, execution: crate::execution::Execution) -> Self {
        Self { deadline: Some(execution.now() + timeout), ..Self::new(execution) }
    }

    pub(super) fn recovered(generation: u64, timeout: Duration, execution: crate::execution::Execution) -> Self {
        Self { generation, disconnected: true, deadline: Some(execution.now() + timeout), ..Self::new(execution) }
    }

    pub(super) fn resumed(&mut self, generation: u64, timeout: Duration, disconnected: bool) {
        self.generation = generation;
        self.disconnected = disconnected;
        self.deadline = Some(self.execution.now() + timeout);
    }

    pub(super) fn connect(&mut self) {
        self.disconnected = false;
        self.deadline = None;
    }

    pub(super) fn renew(&mut self, timeout: Duration) {
        if !self.disconnected && self.deadline.is_some() {
            self.deadline = Some(self.execution.now() + timeout);
        }
    }

    pub(super) fn disconnect(&mut self, file: &StagedFile, offset: u64, timeout: Duration) {
        if !self.disconnected {
            self.disconnected = true;
            self.deadline = Some(self.execution.now() + timeout);
            tracing::warn!(file = %file.id, generation = self.generation, offset, reason = "producer disconnected", "file production retained for resume");
        }
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.staging.session.produce_file", level = "debug", skip_all)]
    pub async fn produce_file(&self, metadata: FileMetadata) -> Result<StorageFile> {
        if !self.inner.primary.supports_file_sessions() {
            return Err(Error::InvalidInput { field: "file session", reason: "backend does not support resumable file sessions" });
        }
        self.create(metadata, true).await
    }

    #[tracing::instrument(name = "storage.staging.session.resume_file", level = "debug", skip_all)]
    pub async fn resume_file(&self, token: &FileSession) -> Result<StorageFile> {
        if let Err(error) = self.inner.primary.history().check(&token.history) {
            tracing::error!(file = %token.id, generation = token.generation, reason = "history mismatch", %error, "file resume rejected");
            return Err(error.into());
        }
        let next = token.generation.checked_add(1).ok_or_else(size_error)?;
        let mut claimed = false;
        let mut changed = self.inner.changed.subscribe();
        loop {
            let mut queued = false;
            let resumed = {
                let mut state = self.inner.state.lock().unwrap();
                match admission(&state) {
                    // The Original Worker Still Owns This Admitted Transition.
                    Err(Error::Persistence { .. })
                        if claimed && state.files.values().any(|file| file.staging.id == token.id && file.retrying) => {}
                    result => result?,
                }
                state.expire_files();
                let Some((id, file)) = state.files.iter_mut().find(|(_, file)| file.staging.id == token.id) else {
                    tracing::error!(file = %token.id, generation = token.generation, reason = "session unavailable or expired", "file resume rejected");
                    return Err(Error::InvalidInput { field: "file resume", reason: "session unavailable or expired" });
                };
                let session = file
                    .session
                    .as_mut()
                    .ok_or(Error::InvalidInput { field: "file resume", reason: "private staging is not resumable" })?;
                if claimed && session.generation == next && !file.abandoned {
                    session.connect();
                    let mut token = token.clone();
                    token.generation = next;
                    Some(StorageFile {
                        manager: self.erased(),
                        id: *id,
                        progress: file.progress.clone(),
                        session: Some(token),
                        append: None,
                    })
                } else {
                    if session.generation != token.generation || file.abandoned {
                        tracing::error!(file = %token.id, generation = token.generation, offset = file.length, reason = "expired or fenced producer", "file resume rejected");
                        return Err(Error::InvalidInput { field: "file resume", reason: "expired or fenced producer" });
                    }
                    if matches!(file.command, Some(Command::Finish { .. } | Command::Abort)) {
                        return Err(Error::InvalidInput { field: "file resume", reason: "production is ending" });
                    }
                    if !claimed && matches!(file.command, Some(Command::Resume { .. })) {
                        return Err(Error::InvalidInput { field: "file resume", reason: "another resume owns the transition" });
                    }
                    if file.command.is_none() && !file.in_flight {
                        file.context = crate::execution::Context::current();
                        file.queued = Some(tracing::debug_span!("storage.file.queue"));
                        file.command = Some(Command::Resume { expected: token.generation, next });
                        // Fence Old Handles At Admission, Before The Durable Transition.
                        file.progress.lock().unwrap().generation = next;
                        claimed = true;
                        queued = true;
                    }
                    None
                }
            };
            if queued {
                self.inner.notify();
            }
            if let Some(file) = resumed {
                return Ok(file);
            }
            changed.changed().await.map_err(|_| closed())?;
        }
    }
}

impl StorageFile {
    pub fn session(&self) -> Option<&FileSession> {
        self.session.as_ref()
    }

    pub(super) fn check_generation(&self) -> Result<()> {
        if let Some(token) = &self.session
            && token.generation != self.progress.lock().unwrap().generation
        {
            tracing::error!(file = %token.id, generation = token.generation, reason = "producer fenced", "file operation rejected");
            return Err(Error::InvalidInput { field: "file session", reason: "producer fenced" });
        }
        Ok(())
    }
}

impl FileState {
    pub(super) fn check_session(&self, token: Option<&FileSession>) -> Result<()> {
        match (&self.session, token) {
            (None, None) => Ok(()),
            (Some(session), Some(token))
                if session.generation == token.generation
                    && !session.disconnected
                    && session.deadline.is_none_or(|deadline| deadline > session.execution.now())
                    && !self.abandoned =>
            {
                Ok(())
            }
            _ => {
                tracing::error!(file = %self.staging.id, offset = self.length, reason = "producer disconnected, expired or fenced", "file operation rejected");
                Err(Error::InvalidInput { field: "file session", reason: "producer disconnected, expired or fenced" })
            }
        }
    }
}

impl State {
    pub(crate) fn expire_files(&mut self) {
        if self.status.closing {
            return;
        }
        let now = self.execution.now();
        for file in self.files.values_mut() {
            let Some(session) = &mut file.session else {
                continue;
            };
            if session.deadline.is_some_and(|deadline| deadline <= now)
                && !file.in_flight
                && !file.abandoned
                && !matches!(file.command, Some(Command::Finish { .. }))
            {
                file.abandoned = true;
                session.reason =
                    if session.disconnected { "disconnected producer resume timeout" } else { "token producer inactivity timeout" };
                file.progress.lock().unwrap().closed = true;
                tracing::error!(file = %file.staging.id, generation = session.generation, offset = file.length, reason = session.reason, "file production abandoned; partial bytes retained");
            }
        }
    }

    pub(crate) fn file_deadline(&self) -> Option<tokio::time::Instant> {
        if self.status.closing {
            return None;
        }
        self.files
            .values()
            .filter(|file| !file.abandoned && !file.in_flight && !matches!(file.command, Some(Command::Finish { .. })))
            .filter_map(|file| file.session.as_ref().and_then(|session| session.deadline))
            .min()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
