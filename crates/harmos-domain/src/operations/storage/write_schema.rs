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
use crate::model::storage::{MutationId, SchemaInput, StorageCursor};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(WriteSchema {
    mutation: Nested<MutationId> => 1,
    schema: Nested<SchemaInput> => 2,
});

crate::declare!(WriteSchema {
    id: "storage.write_schema",
    version: 1,
    policy: POLICY,
    response: StorageCursor,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */
// Register One Payload Schema, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSchema {
    pub mutation: MutationId,
    pub schema: SchemaInput,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for WriteSchema {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.mutation
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.mutation)),
            )
            .saturating_add(
                self.schema
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.schema)),
            )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
