/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// CaptureFile Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Code, Nested, product};
use crate::model::storage::files::{ByteRange, CaptureConsistency, FileMetadata, FileReference, FileSource};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(CaptureFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
    metadata: Nested<FileMetadata> => 3,
    consistency: Code<CaptureConsistency> => 4,
});

crate::declare!(CaptureFile {
    id: "storage.capture_file",
    version: 1,
    policy: IO_POLICY,
    response: FileReference,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Unknown,
    grpc: ("harmos.storage.v1", "Storage", "CaptureFile"),
    grant: |request: &CaptureFile| request.source.grant_scope().map(|scope| crate::plugin::Grant::of("storage.capture_file", scope)),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFile {
    pub source: FileSource,
    pub range: ByteRange,
    pub metadata: FileMetadata,
    pub consistency: CaptureConsistency,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for CaptureFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
            .saturating_add(self.consistency.memory_size().saturating_sub(std::mem::size_of_val(&self.consistency)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
