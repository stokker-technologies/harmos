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

product!(WriteSchemaRequest {
    mutation: Nested<MutationId> => 1,
    schema: Nested<SchemaInput> => 2,
});

pub struct WriteSchema;

impl crate::operations::OperationDeclaration for WriteSchema {
    type Request = WriteSchemaRequest;
    type Response = StorageCursor;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_write_schema";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for WriteSchemaRequest {
    type Operation = WriteSchema;
}

/* === Type Aliases === */

/* === Structs === */

// Register One Payload Schema, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSchemaRequest {
    pub mutation: MutationId,
    pub schema: SchemaInput,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteSchemaRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of_val(&self.schema)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
