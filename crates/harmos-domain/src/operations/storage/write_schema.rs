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
use crate::model::storage::{MutationId, SchemaInput, StorageCursor};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(WriteSchemaRequest {
    mutation: crate::model::storage::codec::Nested<MutationId> => 1,
    schema: crate::model::storage::codec::Nested<SchemaInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSchemaRequest {
    pub mutation: MutationId,
    pub schema: SchemaInput,
}

pub struct WriteSchema;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteSchema {
    type Request = WriteSchemaRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.write_schema";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for WriteSchema {
    const VERSION: u32 = 2;
}

impl crate::MemorySize for WriteSchemaRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.schema.memory_size()
            - std::mem::size_of::<SchemaInput>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
