/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finished File Read Contract

use std::path::PathBuf;

use super::FileReference;
use crate::model::storage::codec::{Nested, Unsigned, coded, variants};
use crate::{Error, MemorySize, Result};

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
