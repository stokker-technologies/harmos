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
use crate::model::storage::{FileProgress, FileSession, MutationId};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(AbandonFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

crate::declare!(AbandonFile {
    id: "storage.abandon_file",
    version: 1,
    policy: POLICY,
    response: FileProgress,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */
// Abandon A Producing File Session, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbandonFile {
    pub mutation: MutationId,
    pub session: FileSession,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for AbandonFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
