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
use crate::model::storage::codec::{Nested, Unsigned, variants};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

crate::model::storage::codec::product!(ByteRange {
    offset: crate::model::storage::codec::Unsigned => 1,
    limit: crate::model::storage::codec::Optional<crate::model::storage::codec::Unsigned> => 2,
});

variants!(FileSource {
    0, 1, "stored": Self::Stored(file) => { file: Nested<FileReference> => 1 } => Self::Stored(file);
    1, 2, "local": Self::Local(path) => { path: super::codec::LocalPath => 1 } => Self::Local(path);
});

crate::model::storage::codec::product!(FileHash {
    bytes_read: Unsigned => 1,
    sha256: super::codec::Digest => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ByteRange {
    pub offset: u64,
    pub limit: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileBatch {
    pub offset: u64,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileCompletion {
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileHash {
    pub bytes_read: u64,
    pub sha256: [u8; 32],
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileSource {
    Stored(FileReference),
    Local(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FollowStart {
    Beginning,
    End,
    At(u64),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileEvent {
    Data(FileBatch),
    Truncated { length: u64 },
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
