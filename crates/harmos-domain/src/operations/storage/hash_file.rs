/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// HashFile Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::files::{ByteRange, FileHash, FileSource};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(HashFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

crate::declare!(HashFile {
    id: "storage.hash_file",
    version: 1,
    policy: IO_POLICY,
    response: FileHash,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Idempotent,
    grpc: ("harmos.storage.v1", "Storage", "HashFile"),
    grant: |request: &HashFile| request.source.grant_scope().map(|scope| crate::plugin::Grant::of("storage.hash_file", scope)),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashFile {
    pub source: FileSource,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for HashFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
