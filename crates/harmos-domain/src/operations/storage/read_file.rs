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
use crate::model::storage::{ByteRange, FileBatch, FileCompletion, FileReference};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(ReadFile {
    file: Nested<FileReference> => 1,
    range: Nested<ByteRange> => 2,
});

crate::declare!(ReadFile {
    id: "storage.read_file",
    version: 1,
    policy: IO_POLICY,
    response: FileCompletion,
    output: FileBatch,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Idempotent,
    grpc: ("harmos.storage.v1", "Storage", "ReadFile"),
    grant: |request: &ReadFile| Some(crate::plugin::Grant::of(
        "storage.read_file",
        request.file.metadata.id.as_str().to_owned(),
    )),
});

/* === Type Aliases === */

/* === Structs === */
// Read A Byte Range Of A Finished, Immutable File.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFile {
    pub file: FileReference,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for ReadFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.file
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.file)),
            )
            .saturating_add(
                self.range
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.range)),
            )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
