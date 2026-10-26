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
use crate::model::storage::{DirectoryCompletion, DirectoryEntry, DirectorySelection};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ListDirectory { selection: Nested<DirectorySelection> => 1 });
crate::declare!(ListDirectory {
    id: "storage.list_directory",
    version: 1,
    policy: IO_POLICY,
    response: DirectoryCompletion,
    output: DirectoryEntry,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Idempotent,
    grpc: ("harmos.storage.v1", "Storage", "ListDirectory"),
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListDirectory {
    pub selection: DirectorySelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for ListDirectory {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
