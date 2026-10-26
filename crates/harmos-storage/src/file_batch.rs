/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native File Batches Retain Their Allocation Charge Through Every Shared View.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::arrow::RetainedMemory;
use crate::{Error, FileBatch, MemorySize, ReadBatch, Result};
use harmos_domain::model::storage::files::FileEvent;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct FileReadBatch {
    owner: ReadBatch<FileBatch>,
}

#[derive(Clone)]
pub struct FileReadSlice {
    owner: FileReadBatch,
    range: Range<usize>,
}

pub type FileReadEvent = ReadBatch<FileEvent>;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FileReadEvent {
    pub fn event(&self) -> &FileEvent {
        self.as_ref()
    }

    pub fn data(&self) -> Option<&[u8]> {
        match self.event() {
            FileEvent::Data(batch) => Some(&batch.bytes),
            _ => None,
        }
    }

    pub fn offset(&self) -> Option<u64> {
        match self.event() {
            FileEvent::Data(batch) => Some(batch.offset),
            _ => None,
        }
    }
}

impl FileReadBatch {
    pub(crate) fn retained(batch: FileBatch, memory: RetainedMemory) -> Self {
        Self { owner: ReadBatch::retained(batch, memory) }
    }

    /// Moves a unique file batch through an admitted projection.
    /// Shared batches or slices refuse; allocation and escape rules match `ReadBatch::try_map`.
    #[tracing::instrument(name = "storage.file_batch.try_map", level = "trace", skip_all, fields(additional_peak = additional_peak, outcome = "cancelled"))]
    pub fn try_map<U, E>(
        self, additional_peak: usize, map: impl FnOnce(FileBatch) -> std::result::Result<U, E>,
    ) -> std::result::Result<ReadBatch<U>, E>
    where
        E: From<Error>,
    {
        let result: std::result::Result<ReadBatch<U>, E> = self.owner.try_map(additional_peak, map);
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn offset(&self) -> u64 {
        self.owner.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.owner.bytes
    }

    pub fn owner(&self) -> Arc<impl Borrow<FileBatch> + Send + Sync + 'static> {
        self.owner.owner()
    }

    pub fn slice(&self, range: Range<usize>) -> FileReadSlice {
        assert!(range.start <= range.end && range.end <= self.bytes().len(), "file slice is outside its batch");
        FileReadSlice { owner: self.clone(), range }
    }
}

impl Borrow<FileBatch> for FileReadBatch {
    fn borrow(&self) -> &FileBatch {
        self.owner.as_ref()
    }
}

impl FileReadSlice {
    pub fn offset(&self) -> u64 {
        self.owner.offset() + self.range.start as u64
    }

    pub fn bytes(&self) -> &[u8] {
        &self.owner.bytes()[self.range.clone()]
    }
}

impl MemorySize for FileReadBatch {
    fn memory_size(&self) -> usize {
        self.owner.memory_size().saturating_add(std::mem::size_of::<Self>())
    }
}

impl std::fmt::Debug for FileReadBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.owner.fmt(formatter)
    }
}

/* === Public Functions === */

pub(crate) fn batch_limit(ceiling: usize, offset: u64, remaining: u64, event: bool) -> Result<NonZeroUsize> {
    let mut count = ceiling.min(usize::try_from(remaining).unwrap_or(usize::MAX));
    loop {
        let batch = 2usize.saturating_add(varint(offset)).saturating_add(varint(count as u64)).saturating_add(count);
        let framed = if event { batch.saturating_add(1 + varint(batch as u64)) } else { batch };
        if framed <= ceiling {
            return NonZeroUsize::new(count).ok_or(Error::ResourceExhausted { resource: "file response batch bytes" });
        }
        count = count
            .checked_sub(framed - ceiling)
            .filter(|count| *count != 0)
            .ok_or(Error::ResourceExhausted { resource: "file response batch bytes" })?;
    }
}

/* === Private Functions === */

fn varint(mut value: u64) -> usize {
    let mut bytes = 1;
    while value >= 128 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}

/* === Tests === */
