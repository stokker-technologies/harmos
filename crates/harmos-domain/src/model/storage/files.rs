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

pub use read::{ByteRange, CaptureConsistency, FileBatch, FileCompletion, FileEvent, FileHash, FileSource, FollowStart};

/* === Imports === */

use harmos_protocol::model::FileMetadata as WireFileMetadata;
use harmos_protocol::model::FileReference as WireFileReference;

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
            .saturating_add(self.id.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
            .saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of::<Option<Identifier>>()))
            .saturating_add(self.media_type.capacity())
    }
}

impl MemorySize for FileReference {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.position.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of::<FileMetadata>()))
    }
}

impl MemorySize for FileSession {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
            .saturating_add(self.id.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
    }
}

impl MemorySize for StagedFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
            .saturating_add(self.id.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
    }
}

impl MemorySize for FileProgress {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<FileMetadata> for WireFileMetadata {
    fn from(value: FileMetadata) -> Self {
        Self {
            id: value.id.to_string(),
            scope: value.scope.map(|id| id.to_string()),
            media_type: value.media_type,
            expected_length: value.expected_length,
            expected_sha256: value.expected_sha256.map(|bytes| bytes.to_vec()),
        }
    }
}
impl TryFrom<WireFileMetadata> for FileMetadata {
    type Error = crate::Error;
    fn try_from(value: WireFileMetadata) -> crate::Result<Self> {
        Ok(Self {
            id: Identifier::new(value.id)?,
            scope: value.scope.map(Identifier::new).transpose()?,
            media_type: value.media_type,
            expected_length: value.expected_length,
            expected_sha256: value
                .expected_sha256
                .map(|bytes| {
                    bytes.try_into().map_err(|_| crate::Error::InvalidInput {
                        field: "expected_sha256",
                        reason: "requires exactly 32 bytes",
                    })
                })
                .transpose()?,
        })
    }
}

impl From<FileReference> for WireFileReference {
    fn from(value: FileReference) -> Self {
        Self {
            position: Some(value.position.into()),
            metadata: Some(value.metadata.into()),
            length: value.length,
            sha256: value.sha256.to_vec(),
        }
    }
}

impl TryFrom<WireFileReference> for FileReference {
    type Error = crate::Error;
    fn try_from(value: WireFileReference) -> crate::Result<Self> {
        Ok(Self {
            position: value.position.ok_or(crate::Error::InvalidInput { field: "position", reason: "required" })?.try_into()?,
            metadata: value.metadata.ok_or(crate::Error::InvalidInput { field: "metadata", reason: "required" })?.try_into()?,
            length: value.length,
            sha256: value.sha256.try_into().map_err(|_| crate::Error::InvalidInput {
                field: "sha256",
                reason: "requires exactly 32 bytes",
            })?,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
