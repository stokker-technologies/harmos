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

impl StorageFile {
    pub fn length(&self) -> u64 {
        self.progress.lock().unwrap().length
    }

    #[tracing::instrument(name = "storage.staging.producer.write_all", level = "debug", skip_all)]
    pub async fn write_all(&mut self, mut bytes: &[u8]) -> Result<()> {
        let mut cancel = CancelProduction {
            manager: self.manager.clone(),
            id: self.id,
            armed: true,
            generation: self.session.as_ref().map(|token| token.generation),
        };
        loop {
            let session = self.session.as_ref();
            let id = self.id;
            let admitted = self
                .manager
                .inner
                .admit_when(|state| {
                    admission(state)?;
                    state.collect();
                    let available =
                        self.manager.inner.settings.request.max_pending_write_bytes.get().saturating_sub(state.status.pending_bytes);
                    let capacity = state.status.pending_batches < self.manager.inner.settings.request.max_pending_write_batches.get();
                    let file = state.files.get_mut(&id).ok_or_else(closed)?;
                    file.check_session(session)?;
                    if file.abandoned || file.command.is_some() {
                        return Err(closed());
                    }
                    let total = file.length.checked_add(u64::try_from(bytes.len()).map_err(|_| size_error())?).ok_or(size_error())?;
                    if file.metadata.expected_length.is_some_and(|expected| total > expected) {
                        return Err(Error::InvalidInput { field: "file length", reason: "input exceeds expected length" });
                    }
                    if bytes.is_empty() {
                        return Ok(Some(None));
                    }
                    if available != 0 && capacity {
                        let count = bytes.len().min(available).min(self.manager.inner.settings.response.batch_bytes.get());
                        file.ticket = file.ticket.checked_add(1).ok_or(size_error())?;
                        file.context = crate::execution::Context::current();
                        file.queued = Some(tracing::debug_span!("storage.file.queue"));
                        file.command = Some(Command::Write(Arc::from(&bytes[..count])));
                        let ticket = file.ticket;
                        state.collect();
                        Ok(Some(Some((count, ticket))))
                    } else {
                        self.manager.inner.relieve_pressure(state, 1, 1);
                        Ok(None)
                    }
                })
                .await?;
            let Some((count, ticket)) = admitted else {
                cancel.armed = false;
                return Ok(());
            };
            self.manager.inner.notify();
            self.wait(Await::Written(ticket)).await?;
            bytes = &bytes[count..];
        }
    }

    #[tracing::instrument(name = "storage.staging.producer.finish", level = "debug", skip_all)]
    pub async fn finish(&mut self) -> Result<FileReference> {
        self.check_generation()?;
        if let Some(reference) = &self.progress.lock().unwrap().reference {
            return Ok(reference.clone());
        }
        let session = self.session.as_ref();
        let id = self.id;
        self.manager
            .inner
            .admit_when(|state| {
                if state.status.closing {
                    return Err(closed());
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "storage worker" });
                }
                state.collect();
                let capacity = state.status.pending_batches < self.manager.inner.settings.request.max_pending_write_batches.get();
                let file = state.files.get_mut(&id).ok_or_else(closed)?;
                if let Some(Command::Finish { .. }) = file.command {
                    file.progress.lock().unwrap().failure = None;
                    file.retrying = false;
                    state.status.failure = None;
                    state.retry = state.retry.wrapping_add(1);
                    Ok(Some(()))
                } else {
                    file.check_session(session)?;
                    if file.abandoned || file.command.is_some() {
                        return Err(closed());
                    }
                    let digest: [u8; 32] = file.hash.clone().finalize().into();
                    if file.metadata.expected_length.is_some_and(|length| length != file.length)
                        || file.metadata.expected_sha256.is_some_and(|expected| expected != digest)
                    {
                        file.abandoned = true;
                        file.progress.lock().unwrap().closed = true;
                        self.manager.inner.notify();
                        return Err(Error::InvalidInput { field: "file integrity", reason: "length or digest does not match" });
                    }
                    if capacity {
                        file.context = crate::execution::Context::current();
                        file.queued = Some(tracing::debug_span!("storage.file.queue"));
                        file.command = Some(Command::Finish { target: None, sha256: digest });
                        state.collect();
                        Ok(Some(()))
                    } else {
                        self.manager.inner.relieve_pressure(state, 0, 1);
                        Ok(None)
                    }
                }
            })
            .await?;
        self.manager.inner.notify();
        self.wait(Await::Finished).await?;
        self.progress.lock().unwrap().reference.clone().ok_or_else(closed)
    }

    #[tracing::instrument(name = "storage.staging.producer.abort", level = "debug", skip_all)]
    pub async fn abort(&mut self) -> Result<()> {
        self.check_generation()?;
        {
            let mut state = self.manager.inner.state.lock().unwrap();
            if let Some(file) = state.files.get_mut(&self.id) {
                if matches!(file.command, Some(Command::Finish { target: Some(_), .. })) {
                    return Err(Error::InvalidInput { field: "file abort", reason: "finish is already being reconciled" });
                }
                file.abandoned = true;
                if let Some(session) = &mut file.session {
                    session.reason = "producer explicitly abandoned";
                }
                if !file.in_flight {
                    file.context = crate::execution::Context::current();
                    file.queued = Some(tracing::debug_span!("storage.file.queue"));
                    file.command = Some(Command::Abort);
                }
                let mut progress = file.progress.lock().unwrap();
                progress.closed = true;
                let failure = progress.failure.take();
                drop(progress);
                file.retrying = false;
                if state.status.failure.as_ref().zip(failure.as_ref()).is_some_and(|(current, previous)| Arc::ptr_eq(current, previous)) {
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

    #[tracing::instrument(name = "storage.staging.producer.wait", level = "debug", skip_all)]
    pub(super) async fn wait(&self, goal: Await) -> Result<()> {
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
                    return Err(Error::Persistence { source: source.clone() });
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
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "storage worker" });
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
        self.manager.cancel_file(self.id, self.session.as_ref().map(|token| token.generation));
    }
}

impl Drop for CancelProduction {
    fn drop(&mut self) {
        if self.armed {
            self.manager.cancel_file(self.id, self.generation);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
