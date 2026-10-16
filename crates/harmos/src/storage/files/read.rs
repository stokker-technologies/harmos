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
// Third Party Imports
// Local Imports
use super::FileReference;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(ReadFile {
    file: crate::storage::codec::Nested<FileReference> => 1,
    range: crate::storage::codec::Nested<ByteRange> => 2,
});
crate::storage::codec::product!(ByteRange {
    offset: crate::storage::codec::Unsigned => 1,
    limit: crate::storage::codec::Optional<crate::storage::codec::Unsigned> => 2,
});
crate::storage::codec::product!(FileBatch {
    offset: crate::storage::codec::Unsigned => 1,
    bytes: crate::storage::codec::Bytes => 2,
});
crate::storage::codec::product!(FileCompletion {
    bytes: crate::storage::codec::Unsigned => 1,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFile {
    pub file: FileReference,
    pub range: ByteRange,
}
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl ByteRange {
    pub(crate) fn end(self, length: u64) -> Result<u64> {
        let remaining = length.checked_sub(self.offset).ok_or(Error::InvalidInput {
            field: "file byte range",
            reason: "offset exceeds the immutable file length",
        })?;
        self.offset
            .checked_add(self.limit.unwrap_or(remaining).min(remaining))
            .ok_or(Error::InvalidInput {
                field: "file byte range",
                reason: "byte range overflow",
            })
    }
}
impl MemorySize for ReadFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.file
                .memory_size()
                .saturating_sub(std::mem::size_of::<FileReference>()),
        )
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
