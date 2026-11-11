/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finished File Read Contract

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
// Third Party Imports
// Local Imports
use super::FileReference;
use crate::model::storage::codec::{Nested, Unsigned, coded, variants};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

crate::model::storage::codec::product!(ByteRange {
    offset: crate::model::storage::codec::Unsigned => 1,
    limit: crate::model::storage::codec::Optional<crate::model::storage::codec::Unsigned> => 2,
});
crate::model::storage::codec::product!(FileBatch {
    offset: crate::model::storage::codec::Unsigned => 1,
    bytes: crate::model::storage::codec::Bytes => 2,
});
crate::model::storage::codec::product!(FileCompletion {
    bytes: crate::model::storage::codec::Unsigned => 1,
});
variants!(FileSource {
    0, 1, "stored": Self::Stored(file) => { file: Nested<FileReference> => 1 } => Self::Stored(file);
    1, 2, "local": Self::Local(path) => { path: super::codec::LocalPath => 1 } => Self::Local(path);
});
variants!(FollowStart {
    0, 1, "beginning": Self::Beginning => {} => Self::Beginning;
    1, 2, "end": Self::End => {} => Self::End;
    2, 3, "at": Self::At(offset) => { offset: Unsigned => 1 } => Self::At(offset);
});
variants!(FileEvent {
    0, 1, "data": Self::Data(batch) => { batch: Nested<FileBatch> => 1 } => Self::Data(batch);
    1, 2, "truncated": Self::Truncated { length } => { length: Unsigned => 1 } => Self::Truncated { length };
    2, 3, "replaced": Self::Replaced => {} => Self::Replaced;
});
coded!(CaptureConsistency { Bounded, Stable });
crate::model::storage::codec::product!(FileHash {
    bytes_read: Unsigned => 1,
    sha256: super::codec::Digest => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange {
    pub offset: u64,
    pub limit: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileBatch {
    pub offset: u64,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCompletion {
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileHash {
    pub bytes_read: u64,
    pub sha256: [u8; 32],
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSource {
    Stored(FileReference),
    Local(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowStart {
    Beginning,
    End,
    At(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileEvent {
    Data(FileBatch),
    Truncated { length: u64 },
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureConsistency {
    Bounded,
    Stable,
}

/* === Traits === */

/* === Implementations === */

impl FileSource {
    pub fn grant_scope(&self) -> Option<String> {
        match self {
            Self::Stored(file) => Some(file.metadata.id.as_str().to_owned()),
            Self::Local(_) => None,
        }
    }
}

impl MemorySize for FileSource {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Stored(file) => file.memory_size().saturating_sub(std::mem::size_of_val(file)),
                Self::Local(path) => path.capacity(),
            }
    }
}

impl MemorySize for FollowStart {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl MemorySize for FileEvent {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Data(batch) => batch.memory_size().saturating_sub(std::mem::size_of_val(batch)),
                Self::Truncated { .. } | Self::Replaced => 0,
            }
    }
}

impl MemorySize for CaptureConsistency {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl MemorySize for FileHash {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl ByteRange {
    pub fn end(self, length: u64) -> Result<u64> {
        let remaining = length.checked_sub(self.offset).ok_or(Error::InvalidInput {
            field: "file byte range",
            reason: "offset exceeds the immutable file length",
        })?;
        self.offset.checked_add(self.limit.unwrap_or(remaining).min(remaining)).ok_or(Error::InvalidInput {
            field: "file byte range",
            reason: "byte range overflow",
        })
    }
}
impl MemorySize for ByteRange {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl MemorySize for FileBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.bytes.capacity())
    }
}
impl MemorySize for FileCompletion {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<ByteRange> for harmos_protocol::storage::ByteRange {
    fn from(value: ByteRange) -> Self {
        Self { offset: value.offset, limit: value.limit }
    }
}
impl From<harmos_protocol::storage::ByteRange> for ByteRange {
    fn from(value: harmos_protocol::storage::ByteRange) -> Self {
        Self { offset: value.offset, limit: value.limit }
    }
}
impl TryFrom<FileSource> for harmos_protocol::storage::FileSource {
    type Error = crate::Error;
    fn try_from(value: FileSource) -> crate::Result<Self> {
        use harmos_protocol::storage::file_source::Source;
        Ok(Self {
            source: Some(match value {
                FileSource::Stored(file) => Source::Stored(file.into()),
                FileSource::Local(path) => Source::Local(path.into_os_string().into_string().map_err(|_| crate::Error::InvalidInput {
                    field: "local path",
                    reason: "transport requires UTF-8",
                })?),
            }),
        })
    }
}
impl TryFrom<harmos_protocol::storage::FileSource> for FileSource {
    type Error = crate::Error;
    fn try_from(value: harmos_protocol::storage::FileSource) -> crate::Result<Self> {
        use harmos_protocol::storage::file_source::Source;
        match value.source.ok_or(crate::Error::InvalidInput { field: "source", reason: "required" })? {
            Source::Stored(file) => Ok(Self::Stored(file.try_into()?)),
            Source::Local(path) => Ok(Self::Local(path.into())),
        }
    }
}
impl From<FileBatch> for harmos_protocol::storage::FileBatch {
    fn from(value: FileBatch) -> Self {
        Self { offset: value.offset, bytes: value.bytes }
    }
}
impl From<FileCompletion> for harmos_protocol::storage::FileCompletion {
    fn from(value: FileCompletion) -> Self {
        Self { bytes: value.bytes }
    }
}
impl From<FileHash> for harmos_protocol::storage::FileHash {
    fn from(value: FileHash) -> Self {
        Self {
            bytes_read: value.bytes_read,
            sha256: value.sha256.to_vec(),
        }
    }
}
impl TryFrom<harmos_protocol::storage::FollowStart> for FollowStart {
    type Error = crate::Error;
    fn try_from(value: harmos_protocol::storage::FollowStart) -> crate::Result<Self> {
        use harmos_protocol::storage::follow_start::Start;
        Ok(match value.start.ok_or(crate::Error::InvalidInput { field: "start", reason: "required" })? {
            Start::Beginning(_) => Self::Beginning,
            Start::End(_) => Self::End,
            Start::At(offset) => Self::At(offset),
        })
    }
}
impl From<FileEvent> for harmos_protocol::storage::FileEvent {
    fn from(value: FileEvent) -> Self {
        use harmos_protocol::storage::{Empty, file_event::Event};
        Self {
            event: Some(match value {
                FileEvent::Data(batch) => Event::Data(batch.into()),
                FileEvent::Truncated { length } => Event::Truncated(length),
                FileEvent::Replaced => Event::Replaced(Empty {}),
            }),
        }
    }
}
impl TryFrom<i32> for CaptureConsistency {
    type Error = crate::Error;
    fn try_from(value: i32) -> crate::Result<Self> {
        match harmos_protocol::storage::CaptureConsistency::try_from(value) {
            Ok(harmos_protocol::storage::CaptureConsistency::Bounded) => Ok(Self::Bounded),
            Ok(harmos_protocol::storage::CaptureConsistency::Stable) => Ok(Self::Stable),
            Err(_) => Err(crate::Error::InvalidInput {
                field: "consistency",
                reason: "unknown value",
            }),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
