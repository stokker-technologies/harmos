/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finished File Reads

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::backend::ScanPage;
use super::capacity::{Held, Pool, Refusal, Reservation};
use super::manager::{ENTRY_OVERHEAD, Inner, Operation, encoded_size};
use super::query::FileSelection;
use super::snapshot::Snapshot;
use super::{FileBatch, FileCompletion, FileReference, StorageManager};
use crate::{BackendFailure, Error, Result};
use harmos_domain::operations::storage::ReadFile;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct FileReader {
    inner: Arc<Inner>,
    held: Held,
    snapshot: Option<Snapshot>,
    file: FileReference,
    offset: u64,
    end: u64,
    bytes: u64,
    verify: bool,
    hash: Sha256,
    _metadata: Reservation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn read_file(&self, request: ReadFile) -> Result<FileReader> {
        let ReadFile { file, range } = request;
        let end = range.end(file.length)?;
        self.inner.primary.history().check(&file.position.history)?;

        // Accepted References Not Yet Saved Must Match The Queued Publication.
        let (saved, pending) = {
            let state = self.inner.state.lock().unwrap();
            if !state.status.accepted.covers(&file.position)? {
                return Err(invalid("file has not been admitted"));
            }
            let pending = state.queue.iter().find_map(|entry| match &entry.operation {
                Operation::File(actual) if actual.position == file.position => Some(actual == &file),
                _ => None,
            });
            (state.status.saved.clone(), pending)
        };
        if pending == Some(false) {
            return Err(invalid("immutable metadata does not match a finished file"));
        }
        let held = Held::default();
        let metadata =
            Reservation::wait(&self.inner, Pool::Read, &held, encoded_size(&file)?.saturating_mul(2).saturating_add(ENTRY_OVERHEAD), None)
                .await
                .map_err(Refusal::into_error)?;
        let snapshot = self.snapshot(&saved).await?;

        // Saved References Must Be Visible In The Pinned Image.
        if pending.is_none() {
            let selection = FileSelection { scope: file.metadata.scope.clone(), item: Some(file.metadata.id.clone()) };
            let page = ScanPage {
                after: file.position.sequence.saturating_sub(1),
                rows: NonZeroUsize::MIN,
                bytes: NonZeroUsize::new(encoded_size(&file)?.max(1)).unwrap(),
            };
            let (stored, _) = snapshot.scan::<FileReference>(&selection, page).await?;
            if stored.first() != Some(&file) {
                return Err(invalid("immutable metadata does not match the stored file"));
            }
        }
        Ok(FileReader {
            inner: Arc::clone(&self.inner),
            held,
            snapshot: Some(snapshot),
            offset: range.offset,
            verify: range.offset == 0 && end == file.length,
            end,
            file,
            bytes: 0,
            hash: Sha256::new(),
            _metadata: metadata,
        })
    }
}

impl FileReader {
    pub fn reference(&self) -> &FileReference {
        &self.file
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub async fn next(&mut self) -> Result<Option<FileBatch>> {
        let Some(snapshot) = &self.snapshot else {
            return Ok(None);
        };
        if self.offset == self.end {
            // An Empty Whole File Still Proves Its Digest.
            if self.verify && self.end == 0 && <[u8; 32]>::from(self.hash.clone().finalize()) != self.file.sha256 {
                return Err(corrupt("file digest mismatch"));
            }
            self.snapshot = None;
            return Ok(None);
        }

        // The Response Envelope Shares The Batch Byte Ceiling.
        let ceiling = self.inner.settings.response.batch_bytes.get();
        let overhead = 2 + varint_bytes(self.offset) + varint_bytes(ceiling as u64);
        let remaining = usize::try_from(self.end - self.offset).unwrap_or(usize::MAX);
        let maximum = ceiling
            .checked_sub(overhead)
            .map(|bytes| bytes.min(remaining))
            .and_then(NonZeroUsize::new)
            .ok_or(Error::ResourceExhausted { resource: "file response batch bytes" })?;
        let charge = maximum.get().saturating_mul(2).saturating_add(std::mem::size_of::<FileBatch>() + ENTRY_OVERHEAD);
        let _transfer = Reservation::wait(&self.inner, Pool::Read, &self.held, charge, None).await.map_err(Refusal::into_error)?;
        let bytes = snapshot.read_file(&self.file, self.offset, maximum).await?;
        if bytes.is_empty() || bytes.len() > maximum.get() || bytes.len() as u64 > self.end - self.offset {
            return Err(corrupt("file batch is empty or outside the immutable length"));
        }
        // Verify The Whole-File Digest Before Delivering Its Final Batch.
        let mut hash = self.hash.clone();
        hash.update(&bytes);
        let next = self.offset + bytes.len() as u64;
        if self.verify && next == self.end && <[u8; 32]>::from(hash.clone().finalize()) != self.file.sha256 {
            return Err(corrupt("file digest mismatch"));
        }
        let offset = self.offset;
        self.hash = hash;
        self.offset = next;
        self.bytes += bytes.len() as u64;
        Ok(Some(FileBatch { offset, bytes }))
    }

    pub fn completion(&self) -> Result<FileCompletion> {
        if self.offset != self.end || self.snapshot.is_some() {
            return Err(Error::NotReady { domain: "file read completion" });
        }
        Ok(FileCompletion { bytes: self.bytes })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn varint_bytes(mut value: u64) -> usize {
    let mut bytes = 1;
    while value >= 128 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "file reference", reason }
}

fn corrupt(reason: &'static str) -> Error {
    Error::backend("file read", BackendFailure::Corrupt, io::Error::new(io::ErrorKind::InvalidData, reason))
}

/* === Tests === */
