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
use std::fmt;
use std::io;
use std::num::NonZeroUsize;
use std::ops::Deref;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::manager::{MemoryReservation, Operation, encoded_size};
use super::*;
use crate::{BackendFailure, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct StorageFileReader {
    view: Option<StorageView>,
    file: FileReference,
    offset: u64,
    hash: Sha256,
    _metadata: MemoryReservation,
}

pub struct FileChunk {
    pub(super) bytes: Vec<u8>,
    pub(super) _reservation: MemoryReservation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl FileChunk {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_query_value(self) -> QueryValue<Vec<u8>> {
        QueryValue::from_file_chunk(self)
    }

    pub(super) fn into_charged_parts(self) -> (Vec<u8>, MemoryReservation) {
        (self.bytes, self._reservation)
    }
}

impl Deref for FileChunk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for FileChunk {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for FileChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("FileChunk")
            .field(&self.as_bytes())
            .finish()
    }
}

impl<C> StorageManager<C> {
    pub async fn read_file(&self, file: &FileReference) -> Result<StorageFileReader> {
        // Validate pending provenance before awaiting snapshot acquisition. Saving
        // may then release queue metadata without making an owned read depend on it.
        let pending = {
            let state = self.inner.state.lock().unwrap();
            if !state.status.accepted.covers(&file.position)? {
                return Err(Error::InvalidInput {
                    field: "file reference",
                    reason: "file has not been admitted",
                });
            }
            state.queue.iter().find_map(|entry| match &entry.operation {
                Operation::File(actual) if actual.position == file.position => Some(actual == file),
                _ => None,
            })
        };
        if pending == Some(false) {
            return Err(Error::InvalidInput {
                field: "file reference",
                reason: "immutable metadata does not match a finished file",
            });
        }
        let view = self.view().await?;
        if pending == Some(true) {
            StorageFileReader::new(view, file).await
        } else {
            view.read_file(file).await
        }
    }
}

impl StorageView {
    pub(super) fn read_files(
        &self,
        selection: ReadSelection,
    ) -> Result<StorageReader<FileReference>> {
        self.reader::<FileReference>(selection)
    }

    pub async fn read_file(&self, file: &FileReference) -> Result<StorageFileReader> {
        self.compute(async {
            let mut selection = ReadSelection::new(ReadKind::Files);
            selection.history = ReadHistory::Retained;
            selection.item = Some(file.metadata.id.clone());
            selection.scope = file.metadata.scope.clone();
            selection.after = file
                .position
                .sequence
                .checked_sub(1)
                .ok_or(Error::InvalidInput {
                    field: "file reference",
                    reason: "file has no admission position",
                })?;
            selection.through = Some(file.position.sequence);
            let mut reader = self.read_files(selection)?;
            let limits = ReadLimits {
                rows: NonZeroUsize::new(1).unwrap(),
                bytes: NonZeroUsize::new(encoded_size(file)?).unwrap(),
            };
            let page =
                reader
                    .next_page_in_computation(limits)
                    .await?
                    .ok_or(Error::InvalidInput {
                        field: "file reference",
                        reason: "file is not visible in this view",
                    })?;
            let actual = page.first().ok_or(Error::InvalidInput {
                field: "file reference",
                reason: "file is not visible in this view",
            })?;
            if actual != file {
                return Err(Error::InvalidInput {
                    field: "file reference",
                    reason: "immutable metadata does not match the stored file",
                });
            }
            StorageFileReader::new(self.clone(), actual).await
        })
        .await
    }
}

impl StorageFileReader {
    async fn new(view: StorageView, file: &FileReference) -> Result<Self> {
        let (_pin, _) = view.pin()?;
        let reservation = view.manager().reserve_read(encoded_size(file)?, 1).await?;
        Ok(Self {
            view: Some(view),
            file: file.clone(),
            offset: 0,
            hash: Sha256::new(),
            _metadata: reservation,
        })
    }

    pub fn reference(&self) -> &FileReference {
        &self.file
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub async fn next_chunk(&mut self, max_bytes: NonZeroUsize) -> Result<Option<FileChunk>> {
        let Some(view) = self.view.clone() else {
            return Ok(None);
        };
        let computation = view.computation()?;
        let work = async {
            let _activity = view.activity()?;
            let manager = view.manager();
            let _lifecycle = manager.inner.lifecycle.lock().await;
            let reservation = manager.reserve_read(max_bytes.get(), 1).await?;
            let (_pin, metadata) = view.pin()?;
            if self.offset == self.file.length {
                if <[u8; 32]>::from(self.hash.clone().finalize()) != self.file.sha256 {
                    return Err(corrupt("file digest mismatch"));
                }
                if let Some(active) = &computation {
                    active.check()?;
                }
                self.view = None;
                return Ok(None);
            }
            let bytes = manager
                .inner
                .bounded(manager.inner.primary.read_file(
                    &metadata,
                    &self.file,
                    self.offset,
                    max_bytes,
                ))
                .await?;
            let _live = view.pin()?;
            if bytes.is_empty()
                || bytes.capacity() > max_bytes.get()
                || bytes.len() as u64 > self.file.length - self.offset
            {
                return Err(corrupt(
                    "file chunk is empty or outside the immutable length",
                ));
            }
            let next = self.offset + bytes.len() as u64;
            let mut hash = self.hash.clone();
            hash.update(&bytes);
            if next == self.file.length
                && <[u8; 32]>::from(hash.clone().finalize()) != self.file.sha256
            {
                return Err(corrupt("file digest mismatch"));
            }
            if let Some(active) = &computation {
                active.check()?;
            }
            self.offset = next;
            self.hash = hash;
            if next == self.file.length {
                self.view = None;
            }
            Ok(Some(FileChunk {
                bytes,
                _reservation: reservation,
            }))
        };
        match &computation {
            Some(active) => tokio::time::timeout(active.timeout(), work)
                .await
                .map_err(|_| super::query::execution_timeout())?,
            None => work.await,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn corrupt(reason: &'static str) -> Error {
    Error::backend(
        "file read",
        BackendFailure::Corrupt,
        io::Error::new(io::ErrorKind::InvalidData, reason),
    )
}

/* === Tests === */
