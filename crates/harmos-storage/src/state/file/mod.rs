/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Durable JSON State Backend

mod files;
mod format;
mod journal;

use std::{
    fs::{File, OpenOptions},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::Mutex;

use super::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StateBackend, StorageStateSettings};
use crate::storage::{ReadLimits, RecoveryPosition, StorageBatch, StorageFuture, StoreHistory};
use crate::{BackendFailure, Error, Result};
use files::JournalOpener;
use journal::StateLog;

pub struct FileBackend {
    root: PathBuf,
    history: StoreHistory,
    settings: StorageStateSettings,
    opener: JournalOpener,
    journal: Arc<Mutex<Option<StateLog>>>,
}

struct CancelOpen(Arc<AtomicBool>);

impl Drop for CancelOpen {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl FileBackend {
    pub fn new(root: PathBuf, history: StoreHistory, settings: StorageStateSettings) -> Self {
        Self::with_journal_opener(root, history, settings, |path| {
            OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)
        })
    }

    // The Opener Must Return The Named Journal Without Truncating Existing Bytes.
    // It Is Called Under The Exclusive Root Lease, Including After Replacement.
    pub fn with_journal_opener(
        root: PathBuf, history: StoreHistory, settings: StorageStateSettings,
        opener: impl Fn(&Path) -> std::io::Result<File> + Send + Sync + 'static,
    ) -> Self {
        let opener = Arc::new(opener);
        let journal = Mutex::new(None);
        let journal = Arc::new(journal);
        Self { root, history, settings, opener, journal }
    }

    #[tracing::instrument(name = "storage.state.file.mod.call", level = "debug", skip_all)]
    async fn call<T: Send + 'static>(&self, action: impl FnOnce(&mut StateLog) -> Result<T> + Send + 'static) -> Result<T> {
        // Acquire Before Dispatch So Cancellation Cannot Enqueue An Older Write.
        let mut owner = tracing::Instrument::instrument(self.journal.clone().lock_owned(), tracing::trace_span!("storage.lock.wait")).await;
        {
            let context = crate::execution::Context::current();
            tokio::task::spawn_blocking(move || {
                context.blocking(|| {
                    let journal = owner.as_mut().ok_or_else(|| format::invalid("state backend is closed"))?;
                    journal.check()?;
                    action(journal)
                })
            })
        }
        .await
        .map_err(task)?
    }
}

impl StateBackend for FileBackend {
    fn history(&self) -> &StoreHistory {
        &self.history
    }

    fn open(&self) -> StorageFuture<'_, RecoveryPosition> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "open", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        let mut owner =
                            tracing::Instrument::instrument(self.journal.clone().lock_owned(), tracing::trace_span!("storage.lock.wait"))
                                .await;
                        let root = self.root.clone();
                        let history = self.history.clone();
                        let settings = self.settings;
                        let opener = Arc::clone(&self.opener);
                        let cancelled = Arc::new(AtomicBool::new(false));
                        let _cancel = CancelOpen(Arc::clone(&cancelled));
                        {
                            let context = crate::execution::Context::current();
                            tokio::task::spawn_blocking(move || {
                                context.blocking(|| {
                                    if owner.is_none() {
                                        *owner = Some(StateLog::open(&root, history, settings, opener, &mut || {
                                            if cancelled.load(Ordering::Acquire) {
                                                Err(Error::Closing { domain: "cancelled state recovery" })
                                            } else {
                                                Ok(())
                                            }
                                        })?);
                                    }
                                    let journal = owner.as_ref().expect("opened journal");
                                    journal.check()?;
                                    Ok(journal.position())
                                })
                            })
                        }
                        .await
                        .map_err(task)?
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn read_snapshot(&self, maximum: NonZeroUsize) -> StorageFuture<'_, Option<RecoverySnapshot>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "read_snapshot", outcome = "cancelled");
                let result = tracing::Instrument::instrument(self.call(move |journal| journal.snapshot(maximum.get())), span.clone()).await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn write_snapshot(&self, snapshot: StorageBatch<RecoverySnapshot>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "write_snapshot", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        if snapshot.len() != 1 {
                            return Err(format::invalid("snapshot write requires exactly one snapshot"));
                        }
                        self.history.check(&snapshot.as_slice()[0].through.history)?;
                        self.call(move |journal| journal.checkpoint(&snapshot.as_slice()[0])).await
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn read_entries<'a>(
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits,
    ) -> StorageFuture<'a, RecoveryPage> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "read_entries", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        self.history.check(&after.history)?;
                        self.history.check(&through.history)?;
                        let after = after.clone();
                        let through = through.clone();
                        self.call(move |journal| journal.page(&after, &through, limits)).await
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn write_entries(&self, entries: StorageBatch<RecoveryEntry>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "write_entries", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        for entry in entries.as_slice() {
                            self.history.check(&entry.position.history)?;
                        }
                        self.call(move |journal| journal.append(entries.as_slice())).await
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "flush", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        self.history.check(&through.history)?;
                        let through = through.clone();
                        self.call(move |journal| journal.flush(&through)).await
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let future = async move {
                let span = tracing::debug_span!("storage.state.backend", method = "close", outcome = "cancelled");
                let result = tracing::Instrument::instrument(
                    async move {
                        let mut owner =
                            tracing::Instrument::instrument(self.journal.clone().lock_owned(), tracing::trace_span!("storage.lock.wait"))
                                .await;
                        {
                            let context = crate::execution::Context::current();
                            tokio::task::spawn_blocking(move || {
                                context.blocking(|| {
                                    let Some(mut journal) = owner.take() else {
                                        return Ok(());
                                    };
                                    journal.check()?;
                                    journal.flush(&journal.position())?;
                                    Ok(())
                                })
                            })
                        }
                        .await
                        .map_err(task)?
                    },
                    span.clone(),
                )
                .await;
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            };
            context.scope(future).await
        })
    }
}

fn task(error: tokio::task::JoinError) -> Error {
    Error::backend("state file task", BackendFailure::Permanent, error)
}
