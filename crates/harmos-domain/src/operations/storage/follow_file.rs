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
use super::STREAM_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, Unsigned, product};
use crate::model::storage::{FileBatch, FileProgress, StagedFile};
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowFile {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

crate::declare!(FollowFile {
    id: "storage.follow_file",
    version: 1,
    policy: STREAM_POLICY,
    response: FileProgress,
    output: FileBatch,
    visibility: Visibility::Host,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowFile {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
