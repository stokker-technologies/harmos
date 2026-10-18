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

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{FileBatch, FileMetadata, FileReference};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(WriteFile {
    metadata: Nested<FileMetadata> => 1,
});

crate::declare!(WriteFile {
    id: "storage.write_file",
    version: 1,
    policy: IO_POLICY,
    response: FileReference,
    input: FileBatch,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Unknown,
    grpc: ("harmos.storage.v1", "Storage", "WriteFile"),
    grant: |request: &WriteFile| Some(crate::plugin::Grant::of("storage.write_file", request.metadata.id.as_str().to_owned(),)),
});

/* === Type Aliases === */

/* === Structs === */
// Write One Finished File From Streamed Batches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFile {
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for WriteFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
