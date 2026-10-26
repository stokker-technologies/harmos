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
use std::num::NonZeroUsize;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::capacity::{Held, Pool, Refusal, Reservation};
use super::{ErasedCatalog, FileBatch, FilePhase, FileProgress, StagedFile, StorageManager};
use crate::FileReadBatch;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ProductionFollower {
    manager: StorageManager<ErasedCatalog>,
    file: StagedFile,
    offset: u64,
    progress: FileProgress,
    changed: watch::Receiver<u64>,
    held: Held,
    _metadata: Reservation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn follow_production(&self, file: &StagedFile, offset: u64) -> Result<ProductionFollower> {
        self.inner.primary.history().check(&file.history)?;
        let held = Held::default();
        let metadata = Reservation::wait(
            &self.inner,
            Pool::Read,
            &held,
            file.memory_size().saturating_add(std::mem::size_of::<ProductionFollower>()),
            None,
        )
        .await
        .map_err(Refusal::into_error)?;
        let changed = self.inner.changed.subscribe();
        let progress = self.file_progress_with_held(file, &held).await?;
        if offset > progress.length {
            tracing::error!(file = %file.id, offset, available = progress.length, reason = "offset exceeds retained bytes", "file following rejected");
            return Err(Error::InvalidInput { field: "file offset", reason: "offset exceeds retained bytes" });
        }
        Ok(ProductionFollower { manager: self.erased(), file: file.clone(), offset, progress, changed, held, _metadata: metadata })
    }

    pub async fn file_progress(&self, file: &StagedFile) -> Result<FileProgress> {
        self.file_progress_with_held(file, &Held::default()).await
    }

    async fn file_progress_with_held(&self, file: &StagedFile, held: &Held) -> Result<FileProgress> {
        self.inner.primary.history().check(&file.history)?;
        let _lifecycle = self.inner.lifecycle.read().await;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing || !state.ready {
                return Err(Error::Closing { domain: "storage file reader" });
            }
            if let Some(progress) = state.files.values().find_map(|active| active.production(file)) {
                return progress;
            }
        }
        let charge = file.memory_size().saturating_add(std::mem::size_of::<FileProgress>() + crate::manager::ENTRY_OVERHEAD);
        let memory = Reservation::wait(&self.inner, Pool::Read, held, charge, None).await.map_err(Refusal::into_error)?.retain();
        self.inner.bounded(self.inner.primary.file_progress(file, memory)).await
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
        loop {
            self.changed.borrow_and_update();
            let progress = self.manager.file_progress_with_held(&self.file, &self.held).await?;
            if self.offset > progress.length {
                return Err(Error::InvalidInput { field: "file offset", reason: "retained bytes moved behind reader offset" });
            }
            self.progress = progress;
            if self.offset < progress.length {
                let ceiling = self.manager.inner.settings.response.batch_bytes.get();
                let remaining = (progress.length - self.offset).min(max_bytes.get() as u64);
                let maximum = crate::file_batch::batch_limit(ceiling, self.offset, remaining, false)?;
                let size = maximum.get();
                let transfer = Reservation::wait(
                    &self.manager.inner,
                    Pool::Read,
                    &self.held,
                    size.saturating_add(std::mem::size_of::<FileReadBatch>() + crate::manager::ENTRY_OVERHEAD),
                    None,
                )
                .await
                .map_err(Refusal::into_error)?
                .retain();
                let _lifecycle = self.manager.inner.lifecycle.read().await;
                if self.manager.inner.state.lock().unwrap().status.closing {
                    return Err(Error::Closing { domain: "storage file reader" });
                }
                let bytes = self
                    .manager
                    .inner
                    .bounded(self.manager.inner.primary.read_production(&self.file, self.offset, maximum, transfer.clone()))
                    .await?;
                if bytes.is_empty() || bytes.len() > size || bytes.capacity() > size {
                    return Err(Error::InvalidInput {
                        field: "file batch",
                        reason: "backend returned an empty or oversized retained prefix",
                    });
                }
                let offset = self.offset;
                self.offset += bytes.len() as u64;
                return Ok(Some(FileReadBatch::retained(FileBatch { offset, bytes }, transfer)));
            }
            if progress.phase != FilePhase::Producing {
                return Ok(None);
            }
            self.changed.changed().await.map_err(|_| Error::Closing { domain: "storage file reader" })?;
        }
    }
}

impl Drop for ProductionFollower {
    fn drop(&mut self) {
        tracing::debug!(file = %self.file.id, offset = self.offset, reason = "viewer disconnected", "file following stopped");
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
