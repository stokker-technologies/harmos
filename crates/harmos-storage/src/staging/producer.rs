/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl StorageFile {
    pub fn length(&self) -> u64 {
        self.progress.lock().unwrap().length
    }

    pub async fn retry_append(&mut self) -> Result<Option<crate::FileChunk>> {
        let Some((offset, bytes, _)) = &self.append else {
            return Ok(None);
        };
        let offset = *offset;
        let bytes = bytes.clone();
        self.write_retained_at(offset, bytes.clone()).await?;
        Ok(Some(bytes))
    }

    pub async fn write_retained_at(&mut self, offset: u64, bytes: crate::FileChunk) -> Result<()> {
        self.check_generation()?;
        if bytes.is_empty() || bytes.len() > self.manager.inner.settings.response.batch_bytes.get() {
            return Err(Error::InvalidInput {
                field: "file append",
                reason: "append requires a nonempty bounded batch",
            });
        }
        if let Some((accepted, pending, _)) = &self.append {
            if *accepted != offset || pending.as_slice() != bytes.as_slice() {
                return Err(Error::InvalidInput {
                    field: "file append",
                    reason: "retry differs from accepted bytes",
                });
            }
        } else {
            self.append = Some((offset, bytes.clone(), None));
        }
        let ticket = match self.append.as_ref().and_then(|(_, _, ticket)| *ticket) {
            Some(ticket) => {
                let mut state = self.manager.inner.state.lock().unwrap();
                let failure = self.progress.lock().unwrap().failure.take();
                if state.status.failure.as_ref().zip(failure.as_ref()).is_some_and(|(current, own)| Arc::ptr_eq(current, own)) {
                    state.status.failure = None;
                    state.retry = state.retry.wrapping_add(1);
                }
                ticket
            }
            None => {
                let admitted = self.queue_append(offset, &bytes, false).await;
                if matches!(admitted, Err(Error::InvalidInput { .. })) {
                    self.append = None;
                }
                let (_, ticket) = admitted?.expect("retained appends are nonempty");
                self.append.as_mut().expect("the original chunk remains retained").2 = Some(ticket);
                ticket
            }
        };
        self.manager.inner.notify();
        self.wait(Await::Written(ticket)).await?;
        self.append = None;
        Ok(())
    }

    #[tracing::instrument(name = "storage.staging.producer.write_all", level = "debug", skip_all)]
    pub async fn write_all(&mut self, bytes: Vec<u8>) -> Result<()> {
        if self.append.is_some() {
            return Err(Error::NotReady { domain: "file append" });
        }
        let source = crate::FileChunk::new(bytes);
        let mut cancel = CancelProduction {
            manager: self.manager.clone(),
            id: self.id,
            armed: true,
            generation: self.session.as_ref().map(|token| token.generation),
        };
        let mut offset = 0usize;
        loop {
            let remaining = source.slice(offset..source.len());
            let admitted = self.queue_append(self.length(), &remaining, true).await?;
            let Some((count, ticket)) = admitted else {
                cancel.armed = false;
                return Ok(());
            };
            self.manager.inner.notify();
            self.wait(Await::Written(ticket)).await?;
            offset += count;
            if offset == source.len() {
                cancel.armed = false;
                return Ok(());
            }
        }
    }

    async fn queue_append(&self, offset: u64, bytes: &crate::FileChunk, partial: bool) -> Result<Option<(usize, u64)>> {
        self.manager
            .inner
            .admit_when(|state| {
                if state.status.closing || !state.ready || !state.status.running {
                    return Err(closed());
                }
                state.collect();
                let file = state.files.get_mut(&self.id).ok_or_else(closed)?;
                file.check_session(self.session.as_ref())?;
                if file.abandoned {
                    return Err(closed());
                }
                if file.length != offset {
                    return Err(Error::InvalidInput {
                        field: "file offset",
                        reason: "offset differs from acknowledged length",
                    });
                }
                let total = offset.checked_add(u64::try_from(bytes.len()).map_err(|_| size_error())?).ok_or_else(size_error)?;
                if file.metadata.expected_length.is_some_and(|expected| total > expected) {
                    return Err(Error::InvalidInput {
                        field: "file length",
                        reason: "append exceeds expected length",
                    });
                }
                if let Some(Command::Write(pending)) = &file.command {
                    if pending.as_slice() != bytes.as_slice() {
                        return Err(Error::InvalidInput {
                            field: "file append",
                            reason: "retry differs from accepted bytes",
                        });
                    }
                    let ticket = file.ticket;
                    let failure = file.progress.lock().unwrap().failure.take();
                    if state.status.failure.as_ref().zip(failure.as_ref()).is_some_and(|(current, own)| Arc::ptr_eq(current, own)) {
                        state.status.failure = None;
                        state.retry = state.retry.wrapping_add(1);
                    }
                    return Ok(Some(Some((bytes.len(), ticket))));
                }
                if file.command.is_some() || file.in_flight {
                    return Err(Error::NotReady { domain: "file append" });
                }
                admission(state)?;
                if bytes.is_empty() {
                    return Ok(Some(None));
                }
                let budget = self.manager.inner.settings.request.max_pending_write_bytes.get();
                if !partial && bytes.len() > budget {
                    return Err(Error::ResourceExhausted { resource: "file append" });
                }
                let available = budget.saturating_sub(state.status.pending_bytes);
                let required = if partial { 1 } else { bytes.len() };
                if available < required
                    || state.status.pending_batches >= self.manager.inner.settings.request.max_pending_write_batches.get()
                {
                    self.manager.inner.relieve_pressure(state, required, 1);
                    return Ok(None);
                }
                let count = bytes.len().min(available).min(self.manager.inner.settings.response.batch_bytes.get());
                let file = state.files.get_mut(&self.id).expect("selected file");
                file.ticket = file.ticket.checked_add(1).ok_or_else(size_error)?;
                file.context = crate::execution::Context::current();
                file.queued = Some(tracing::debug_span!("storage.file.queue"));
                file.command = Some(Command::Write(bytes.slice(0..count)));
                let ticket = file.ticket;
                state.collect();
                Ok(Some(Some((count, ticket))))
            })
            .await
    }

    #[tracing::instrument(name = "storage.staging.producer.finish", level = "debug", skip_all)]
    pub async fn finish(&mut self) -> Result<FileReference> {
        self.check_generation()?;
        if self.append.is_some() {
            return Err(Error::NotReady { domain: "file append" });
        }
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
                        return Err(Error::InvalidInput {
                            field: "file integrity",
                            reason: "length or digest does not match",
                        });
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
                    && !held.is_some_and(|file| file.retrying || file.in_flight)
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
