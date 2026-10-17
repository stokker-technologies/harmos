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
use crate::model::storage::{FileMetadata, FileProducer, MutationId};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(CreateFile {
    mutation: Nested<MutationId> => 1,
    metadata: Nested<FileMetadata> => 2,
});

crate::declare!(CreateFile {
    id: "storage.create_file",
    version: 1,
    policy: POLICY,
    response: FileProducer,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */
// Open A Producing File Session For New Metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFile {
    pub mutation: MutationId,
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for CreateFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
