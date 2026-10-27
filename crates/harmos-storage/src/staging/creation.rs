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

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.staging.creation.create_file", level = "debug", skip_all)]
    pub async fn create_file(&self, metadata: FileMetadata) -> Result<StorageFile> {
        self.create(metadata, false).await
    }

    #[tracing::instrument(name = "storage.staging.creation.create", level = "debug", skip_all, fields(outcome = "cancelled"))]
    pub(super) async fn create(&self, metadata: FileMetadata, resumable: bool) -> Result<StorageFile> {
        let result = async {
            let reference_bytes = file_bytes(&metadata, self.inner.primary.history())?;
            // Bound The Finished Reference Before Creating Its Producer.
            if reference_bytes > self.inner.settings.max_batch_bytes().get() {
                return Err(Error::InvalidInput {
                    field: "file metadata",
                    reason: "reference envelope exceeds max_batch_bytes",
                });
            }
            let progress = Arc::new(Mutex::new(Progress {
                generation: u64::from(resumable),
                ..Progress::default()
            }));
            let token = resumable.then(|| FileSession {
                history: self.inner.primary.history().clone(),
                id: metadata.id.clone(),
                generation: 1,
            });
            let mut metadata = Some(metadata);
            let id = self
                .inner
                .admit_when(|state| {
                    admission(state)?;
                    let candidate = metadata.as_ref().expect("unadmitted file metadata");
                    if state.files.values().any(|file| file.metadata.id == candidate.id) || state.deleting(&candidate.id) {
                        return Err(Error::AlreadyRegistered {
                            domain: "storage file",
                            name: candidate.id.to_string(),
                        });
                    }
                    state.collect();
                    // Leave at least one byte for batch progress even with tiny policies.
                    if state.files.len() < crate::storage::config::MAX_FILES.get()
                        && state.status.pending_batches < self.inner.settings.request.max_pending_write_batches.get()
                        && state.status.pending_bytes < self.inner.settings.request.max_pending_write_bytes.get()
                    {
                        let id =
                            NEXT_FILE.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| size_error())?;
                        let metadata = metadata.take().expect("unadmitted file metadata");
                        let staging = StagedFile {
                            history: self.inner.primary.history().clone(),
                            id: metadata.id.clone(),
                        };
                        state.files.insert(
                            id,
                            FileState {
                                context: crate::execution::Context::current(),
                                queued: Some(tracing::debug_span!("storage.file.queue")),
                                metadata: Arc::new(metadata),
                                staging,
                                session: resumable.then(|| Session::new(self.inner.execution)),
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

                                reference_bytes,
                            },
                        );
                        state.collect();
                        return Ok(Some(id));
                    }
                    self.inner.relieve_pressure(state, 1, 1);
                    Ok(None)
                })
                .await?;
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
                append: None,
            };
            self.inner.notify();
            file.wait(Await::Ready).await?;
            cancel.armed = false;
            Ok(file)
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    #[tracing::instrument(name = "storage.staging.creation.write_file", level = "debug", skip_all)]
    pub async fn write_file(&self, metadata: FileMetadata, bytes: Vec<u8>) -> Result<FileReference> {
        let mut file = self.create_file(metadata).await?;
        file.write_all(bytes).await?;
        file.finish().await
    }

    #[tracing::instrument(name = "storage.staging.creation.settle_files", level = "debug", skip_all)]
    pub(crate) async fn settle_files(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.status.failure.take().is_some() {
                state.retry = state.retry.wrapping_add(1);
            }
            for file in state.files.values_mut() {
                file.retrying = false;
                file.progress.lock().unwrap().failure = None;
                if (file.session.is_none() || !file.progress.lock().unwrap().ready) && !matches!(file.command, Some(Command::Finish { .. }))
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
                if state.files.values().all(|file| file.session.is_some() && file.command.is_none() && !file.in_flight && !file.abandoned) {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "storage worker" });
                }
            }
            changed.changed().await.map_err(|_| closed())?;
        }
    }

    pub(super) fn cancel_file(&self, id: u64, generation: Option<u64>) {
        let mut state = self.inner.state.lock().unwrap();
        let mut clear = false;
        let failure = state.status.failure.clone();
        if let Some(file) = state.files.get_mut(&id) {
            if let Some(session) = &mut file.session {
                if generation == Some(session.generation) && !file.abandoned {
                    session.disconnect(&file.staging, file.length, crate::storage::config::FILE_SESSION_TIMEOUT);
                }
                self.inner.notify();
                return;
            }
            file.abandoned = true;
            if !matches!(file.command, Some(Command::Finish { .. })) {
                let mut progress = file.progress.lock().unwrap();
                progress.closed = true;
                clear = failure.as_ref().zip(progress.failure.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b));
            }
        }
        if clear {
            state.status.failure = None;
            state.retry = state.retry.wrapping_add(1);
        }
        self.inner.notify();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
