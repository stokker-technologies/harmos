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
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FileFollower {
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
    pub async fn follow_file(&self, file: &StagedFile, offset: u64) -> Result<FileFollower> {
        self.inner.primary.history().check(&file.history)?;
        let held = Held::default();
        let metadata = Reservation::wait(
            &self.inner,
            Pool::Read,
            &held,
            file.memory_size()
                .saturating_add(std::mem::size_of::<FileFollower>()),
            None,
        )
        .await
        .map_err(Refusal::into_error)?;
        let changed = self.inner.changed.subscribe();
        let progress = self.file_progress(file).await?;
        if offset > progress.length {
            tracing::error!(file = %file.id, offset, available = progress.length, reason = "offset exceeds retained bytes", "file following rejected");
            return Err(Error::InvalidInput {
                field: "file offset",
                reason: "offset exceeds retained bytes",
            });
        }
        Ok(FileFollower {
            manager: self.erased(),
            file: file.clone(),
            offset,
            progress,
            changed,
            held,
            _metadata: metadata,
        })
    }

    pub async fn file_progress(&self, file: &StagedFile) -> Result<FileProgress> {
        self.inner.primary.history().check(&file.history)?;
        let _lifecycle = self.inner.lifecycle.read().await;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing || !state.ready {
                return Err(Error::Closing {
                    domain: "storage file reader",
                });
            }
            if let Some(progress) = state
                .files
                .values()
                .find_map(|active| active.production(file))
            {
                return progress;
            }
        }
        self.inner
            .bounded(self.inner.primary.file_progress(file))
            .await
    }
}

impl FileFollower {
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn progress(&self) -> FileProgress {
        self.progress
    }

    /// The next retained byte batch, waiting while the file is still producing.
    pub async fn next_batch(&mut self, max_bytes: NonZeroUsize) -> Result<Option<FileBatch>> {
        loop {
            self.changed.borrow_and_update();
            let progress = self.manager.file_progress(&self.file).await?;
            if self.offset > progress.length {
                return Err(Error::InvalidInput {
                    field: "file offset",
                    reason: "retained bytes moved behind reader offset",
                });
            }
            self.progress = progress;
            if self.offset < progress.length {
                let size = max_bytes
                    .get()
                    .min(self.manager.inner.settings.response.batch_bytes.get())
                    .min(usize::try_from(progress.length - self.offset).unwrap_or(usize::MAX));
                let maximum = NonZeroUsize::new(size).expect("nonempty file prefix");
                let _transfer =
                    Reservation::wait(&self.manager.inner, Pool::Read, &self.held, size, None)
                        .await
                        .map_err(Refusal::into_error)?;
                let _lifecycle = self.manager.inner.lifecycle.read().await;
                if self.manager.inner.state.lock().unwrap().status.closing {
                    return Err(Error::Closing {
                        domain: "storage file reader",
                    });
                }
                let bytes = self
                    .manager
                    .inner
                    .bounded(self.manager.inner.primary.read_production(
                        &self.file,
                        self.offset,
                        maximum,
                    ))
                    .await?;
                if bytes.is_empty() || bytes.len() > size || bytes.capacity() > size {
                    return Err(Error::InvalidInput {
                        field: "file batch",
                        reason: "backend returned an empty or oversized retained prefix",
                    });
                }
                let offset = self.offset;
                self.offset += bytes.len() as u64;
                return Ok(Some(FileBatch { offset, bytes }));
            }
            if progress.phase != FilePhase::Producing {
                return Ok(None);
            }
            self.changed.changed().await.map_err(|_| Error::Closing {
                domain: "storage file reader",
            })?;
        }
    }
}

impl Drop for FileFollower {
    fn drop(&mut self) {
        tracing::debug!(file = %self.file.id, offset = self.offset, reason = "viewer disconnected", "file following stopped");
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
