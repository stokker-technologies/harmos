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
// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::digest::common::hazmat::{SerializableState, SerializedState};
use sha2::{Digest, Sha256};
// Local Imports
use super::{FileMetadata, FileSession};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCheckpoint {
    pub metadata: FileMetadata,
    pub session: FileSession,
    pub length: u64,
    pub hash_format: u32,
    pub hash_state: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FileCheckpoint {
    pub(super) fn new(metadata: FileMetadata, session: FileSession, length: u64, hash: &Sha256) -> Self {
        Self {
            metadata,
            session,
            length,
            hash_format: 1,
            hash_state: hash.serialize().to_vec(),
        }
    }

    pub fn sha256(&self) -> Result<[u8; 32]> {
        Ok(self.restore_hash()?.finalize().into())
    }

    pub(super) fn restore_hash(&self) -> Result<Sha256> {
        if self.hash_format != 1
            || self.session.generation == 0
            || self.metadata.id != self.session.id
            || self.metadata.expected_length.is_some_and(|length| self.length > length)
        {
            return Err(invalid());
        }
        // Format 1: 32 State Bytes, Eight LE Block-Count Bytes, Then One
        // Buffer-Length Byte And Up To 63 Buffered Bytes (An Eager SHA-256 Block).
        if self.hash_state.len() != 104 {
            return Err(invalid());
        }
        let blocks = u64::from_le_bytes(self.hash_state[32..40].try_into().map_err(|_| invalid())?);
        let length = blocks.checked_mul(64).and_then(|length| length.checked_add(u64::from(self.hash_state[40])));
        if length != Some(self.length) {
            return Err(invalid());
        }
        let encoded = <&SerializedState<Sha256>>::try_from(self.hash_state.as_slice()).map_err(|_| invalid())?;
        Sha256::deserialize(encoded).map_err(|_| invalid())
    }
}

impl MemorySize for FileCheckpoint {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of::<FileMetadata>()))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of::<FileSession>()))
            .saturating_add(self.hash_state.capacity())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid() -> Error {
    Error::InvalidInput {
        field: "file checkpoint",
        reason: "invalid metadata, generation or hash state",
    }
}

/* === Tests === */
