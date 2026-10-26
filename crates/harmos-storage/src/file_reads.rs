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
use super::{ByteRange, FileBatch, FileCompletion, FileReference, StorageManager};
use crate::FileReadBatch;
use crate::arrow::RetainedMemory;
use crate::local::LocalReader;
use crate::{BackendFailure, Error, MemorySize, Result};
use harmos_domain::model::storage::files::{FileHash, FileSource};
use harmos_domain::operations::storage::ReadFile;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct FileReader {
    context: crate::execution::Context,
    source: Source,
}

enum Source {
    Stored(Box<StoredReader>),
    Local(LocalReader),
}

struct StoredReader {
    inner: Arc<Inner>,
    held: Held,
    snapshot: Option<Snapshot>,
    file: FileReference,
    offset: u64,
    end: u64,
    bytes: u64,
    verify: bool,
    hash: Sha256,
    _metadata: RetainedMemory,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.file_reads.read_file", level = "debug", skip_all)]
    pub async fn read_file(&self, request: ReadFile) -> Result<FileReader> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.read_file", outcome = "cancelled"));
        context.result(span, async { self.read_file_with_held(request, Held::default()).await }).await
    }

    pub(crate) async fn read_file_with_held(&self, request: ReadFile, held: Held) -> Result<FileReader> {
        let source = match request.source {
            FileSource::Stored(file) => Source::Stored(Box::new(self.read_stored_file(file, request.range, held).await?)),
            FileSource::Local(path) => Source::Local(LocalReader::open(&self.inner, path, request.range, held).await?),
        };
        Ok(FileReader { context: crate::execution::Context::current(), source })
    }

    async fn read_stored_file(&self, file: FileReference, range: ByteRange, held: Held) -> Result<StoredReader> {
        let end = range.end(file.length)?;
        self.inner.primary.history().check(&file.position.history)?;

        // Accepted References Not Yet Saved Must Match The Queued Publication.
        let (saved, pending) = {
            let state = self.inner.state.lock().unwrap();
            if !state.status.accepted.covers(&file.position)? {
                return Err(invalid("file has not been admitted"));
            }
            let pending = state.queue.iter().find_map(|entry| match &entry.operation {
                Operation::File(actual) if actual.as_slice()[0].position == file.position => Some(actual.as_slice()[0] == file),
                _ => None,
            });
            (state.status.saved.clone(), pending)
        };
        if pending == Some(false) {
            return Err(invalid("immutable metadata does not match a finished file"));
        }
        let metadata = Reservation::wait(
            &self.inner,
            Pool::Read,
            &held,
            file.memory_size().saturating_mul(3).saturating_add(std::mem::size_of::<StoredReader>() + ENTRY_OVERHEAD),
            None,
        )
        .await
        .map_err(Refusal::into_error)?
        .retain();
        let snapshot = self.snapshot(&saved, &held).await?;

        // Saved References Must Be Visible In The Pinned Image.
        if pending.is_none() {
            let selection = FileSelection { scope: file.metadata.scope.clone(), item: Some(file.metadata.id.clone()) };
            let page = ScanPage {
                after: file.position.sequence.saturating_sub(1),
                rows: NonZeroUsize::MIN,
                bytes: NonZeroUsize::new(encoded_size(&file)?.max(1)).unwrap(),
            };
            let (stored, _) = snapshot.scan::<FileReference>(&selection, page, metadata.clone()).await?;
            if stored.first() != Some(&file) {
                return Err(invalid("immutable metadata does not match the stored file"));
            }
        }
        Ok(StoredReader {
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
    pub(crate) fn stored_hash(&self) -> Result<Option<FileHash>> {
        self.completion()?;
        Ok(match &self.source {
            Source::Stored(reader) => Some(FileHash { bytes_read: reader.bytes, sha256: reader.hash.clone().finalize().into() }),
            Source::Local(_) => None,
        })
    }

    pub(crate) fn is_local(&self) -> bool {
        matches!(self.source, Source::Local(_))
    }

    pub fn offset(&self) -> u64 {
        match &self.source {
            Source::Stored(reader) => reader.offset,
            Source::Local(reader) => reader.offset(),
        }
    }

    pub async fn next(&mut self) -> Result<Option<FileReadBatch>> {
        Ok(self.next_raw(false).await?.map(|(batch, memory)| FileReadBatch::retained(batch, memory)))
    }

    pub(crate) async fn next_raw(&mut self, event: bool) -> Result<Option<(FileBatch, RetainedMemory)>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page", outcome = "cancelled"));
        let future = async {
            match &mut self.source {
                Source::Stored(reader) => reader.next(event).await,
                Source::Local(reader) => reader.next(event).await,
            }
        };
        context.result(span, future).await
    }

    pub(crate) async fn check_unchanged(&self) -> Result<()> {
        match &self.source {
            Source::Stored(_) => Ok(()),
            Source::Local(reader) => reader.check_unchanged().await,
        }
    }

    pub fn completion(&self) -> Result<FileCompletion> {
        match &self.source {
            Source::Stored(reader) => reader.completion(),
            Source::Local(reader) => reader.completion(),
        }
    }
}

impl StoredReader {
    async fn next(&mut self, event: bool) -> Result<Option<(FileBatch, RetainedMemory)>> {
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
        let maximum = crate::file_batch::batch_limit(ceiling, self.offset, self.end - self.offset, event)?;
        let charge = maximum.get().saturating_mul(2).saturating_add(std::mem::size_of::<FileBatch>() + ENTRY_OVERHEAD);
        let transfer = Reservation::wait(&self.inner, Pool::Read, &self.held, charge, None).await.map_err(Refusal::into_error)?.retain();
        let bytes = snapshot.read_file(&self.file, self.offset, maximum, transfer.clone()).await?;
        if bytes.is_empty() || bytes.capacity() > maximum.get() || bytes.len() as u64 > self.end - self.offset {
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
        Ok(Some((FileBatch { offset, bytes }, transfer)))
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

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "file reference", reason }
}

fn corrupt(reason: &'static str) -> Error {
    Error::backend("file read", BackendFailure::Corrupt, io::Error::new(io::ErrorKind::InvalidData, reason))
}

/* === Tests === */
