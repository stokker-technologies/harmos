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
use super::POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{MutationId, StagedFile};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(DeleteFile {
    mutation: Nested<MutationId> => 1,
    file: Nested<StagedFile> => 2,
});

crate::declare!(DeleteFile {
    id: "storage.delete_file",
    version: 1,
    policy: POLICY,
    response: StagedFile,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */
// Delete A Staged File, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFile {
    pub mutation: MutationId,
    pub file: StagedFile,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for DeleteFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
