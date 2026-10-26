/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// FollowProduction Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, Unsigned, product};
use crate::model::storage::files::{FileBatch, FileProgress, StagedFile};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(FollowProduction {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

crate::declare!(FollowProduction {
    id: "storage.follow_production",
    version: 1,
    policy: IO_POLICY,
    response: FileProgress,
    output: FileBatch,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Unknown,
    grpc: ("harmos.storage.v1", "Storage", "FollowProduction"),
    grant: |request: &FollowProduction| Some(crate::plugin::Grant::of("storage.follow_production", request.file.id.as_str().to_owned())),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowProduction {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowProduction {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
