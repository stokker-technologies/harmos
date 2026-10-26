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
use crate::model::storage::files::{FileCompletion, FileEvent, FileSource, FollowStart};
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowFile {
    source: Nested<FileSource> => 1,
    start: Nested<FollowStart> => 2,
});

crate::declare!(FollowFile {
    id: "storage.follow_file",
    version: 1,
    policy: IO_POLICY,
    response: FileCompletion,
    output: FileEvent,
    visibility: Visibility::Shared,
    grpc: ("harmos.storage.v1", "Storage", "FollowFile"),
    grant: |request: &FollowFile| request.source.grant_scope().map(|scope| crate::plugin::Grant::of("storage.follow_file", scope)),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowFile {
    pub source: FileSource,
    pub start: FollowStart,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.start.memory_size().saturating_sub(std::mem::size_of_val(&self.start)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
