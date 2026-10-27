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
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::{MutationId, SchemaInput, StorageCursor};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WriteSchema {
    mutation: Nested<MutationId> => 1,
    schema: Nested<SchemaInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Register One Payload Schema, Exactly Once Per Mutation.
#[harmos::operation(harmos_path = crate, resources = [harmos_storage::StorageManager<harmos_storage::ErasedCatalog>], codec = manual,
    id = "storage.write_schema",
    response = StorageCursor,
    policy = POLICY,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
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
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of_val(&self.schema)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;

    use harmos_storage::StorageCursor;

    use crate::{Error, Result};

    impl Operation for super::WriteSchema {
        type Error = Error;

        async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StorageCursor>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let request = &mut self;

            let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
                storage
                    .write_schema_once(&request.mutation, request.schema.key.clone(), request.schema.graph_owner())
                    .await
                    .map_err(Into::into)
            }
            .await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
