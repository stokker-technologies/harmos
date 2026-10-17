/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod codec;
pub mod read;

/* === Re-Exports === */
pub use read::{ByteRange, FileBatch, FileCompletion, ReadFile};

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::model::storage::{StorageCursor, StoreHistory};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileMetadata {
    pub id: Identifier,
    pub scope: Option<Identifier>,
    pub media_type: String,
    pub expected_length: Option<u64>,
    pub expected_sha256: Option<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedFile {
    pub history: StoreHistory,
    pub id: Identifier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileReference {
    pub position: StorageCursor,
    pub metadata: FileMetadata,
    pub length: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSession {
    pub history: StoreHistory,
    pub id: Identifier,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileProgress {
    pub length: u64,
    pub phase: FilePhase,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u64)]
pub enum FilePhase {
    Producing = 0,
    Finished = 1,
    Abandoned = 2,
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for FileMetadata {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.id
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            )
            .saturating_add(
                self.scope
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Option<Identifier>>()),
            )
            .saturating_add(self.media_type.capacity())
    }
}

impl MemorySize for FileReference {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.position
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
            .saturating_add(
                self.metadata
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<FileMetadata>()),
            )
    }
}

impl MemorySize for FileSession {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.history
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StoreHistory>()),
            )
            .saturating_add(
                self.id
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            )
    }
}

impl MemorySize for StagedFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.history
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StoreHistory>()),
            )
            .saturating_add(
                self.id
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            )
    }
}

impl MemorySize for FileProgress {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
