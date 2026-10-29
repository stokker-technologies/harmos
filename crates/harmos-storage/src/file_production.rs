/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::NonZeroUsize;

use tokio::sync::watch;

use super::{ErasedCatalog, FileBatch, FilePhase, FileProgress, StagedFile, StorageManager};
use crate::FileReadBatch;
use crate::{Error, Result};

pub struct ProductionFollower {
    context: crate::execution::Context,
    manager: StorageManager<ErasedCatalog>,
    file: StagedFile,
    offset: u64,
    progress: FileProgress,
    changed: watch::Receiver<u64>,
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.file_production.follow_production", level = "debug", skip_all)]
    pub async fn follow_production(&self, file: &StagedFile, offset: u64) -> Result<ProductionFollower> {
        self.open_production_follower(file, offset).await
    }

    pub(crate) async fn open_production_follower(&self, file: &StagedFile, offset: u64) -> Result<ProductionFollower> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.follow_production", outcome = "cancelled"));
        context.result(span, async {
        self.inner.primary.history().check(&file.history)?;

        let changed = self.inner.changed.subscribe();
        let progress = self.production_progress(file).await?;
        if offset > progress.length {
            tracing::error!(file = %file.id, offset, available = progress.length, reason = "offset exceeds retained bytes", "file following rejected");
            return Err(Error::InvalidInput { field: "file offset", reason: "offset exceeds retained bytes" });
        }
        Ok(ProductionFollower {
            context: crate::execution::Context::current(),
            manager: self.erased(),
            file: file.clone(),
            offset,
            progress,
            changed,


        })

        }).await
    }

    #[tracing::instrument(name = "storage.file_production.file_progress", level = "debug", skip_all)]
    pub async fn file_progress(&self, file: &StagedFile) -> Result<FileProgress> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.file_progress", outcome = "cancelled"));
        context.result(span, async { self.production_progress(file).await }).await
    }

    pub(crate) async fn production_progress(&self, file: &StagedFile) -> Result<FileProgress> {
        self.inner.primary.history().check(&file.history)?;
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing || !state.ready {
                return Err(Error::Closing { domain: "storage file reader" });
            }
            if let Some(progress) = state.files.values().find_map(|active| active.production(file)) {
                return progress;
            }
        }

        self.inner.bounded("primary.file_progress", self.inner.primary.file_progress(file)).await
    }
}

impl ProductionFollower {
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn progress(&self) -> FileProgress {
        self.progress
    }

    pub async fn next_batch(&mut self, max_bytes: NonZeroUsize) -> Result<Option<FileReadBatch>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page", outcome = "cancelled"));
        let future = async {
            loop {
                self.changed.borrow_and_update();
                let progress = self.manager.production_progress(&self.file).await?;
                if self.offset > progress.length {
                    return Err(Error::InvalidInput {
                        field: "file offset",
                        reason: "retained bytes moved behind reader offset",
                    });
                }
                self.progress = progress;
                if self.offset < progress.length {
                    let ceiling = self.manager.inner.settings.response.batch_bytes.get();
                    let remaining = (progress.length - self.offset).min(max_bytes.get() as u64);
                    let maximum = crate::file_batch::batch_limit(ceiling, self.offset, remaining, false)?;
                    let size = maximum.get();

                    let _lifecycle =
                        tracing::Instrument::instrument(self.manager.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait"))
                            .await;
                    if self.manager.inner.state.lock().unwrap().status.closing {
                        return Err(Error::Closing { domain: "storage file reader" });
                    }
                    let bytes = self
                        .manager
                        .inner
                        .bounded("primary.read_production", self.manager.inner.primary.read_production(&self.file, self.offset, maximum))
                        .await?;
                    if bytes.is_empty() || bytes.len() > size || bytes.capacity() > size {
                        return Err(Error::InvalidInput {
                            field: "file batch",
                            reason: "backend returned an empty or oversized retained prefix",
                        });
                    }
                    let offset = self.offset;
                    self.offset += bytes.len() as u64;
                    return Ok(Some(FileReadBatch::new(FileBatch { offset, bytes })));
                }
                if progress.phase != FilePhase::Producing {
                    return Ok(None);
                }
                self.changed.changed().await.map_err(|_| Error::Closing { domain: "storage file reader" })?;
            }
        };
        context.result(span, future).await
    }
}

impl Drop for ProductionFollower {
    fn drop(&mut self) {
        tracing::debug!(file = %self.file.id, offset = self.offset, reason = "viewer disconnected", "file following stopped");
    }
}
