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
use harmos_domain::operations::ReplaySafety;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(WriteSchema {
    mutation: Nested<MutationId> => 1,
    schema: Nested<SchemaInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Register One Payload Schema, Exactly Once Per Mutation.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.write_schema",
    response = StorageCursor,
    policy = POLICY,
    visibility = Private,
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

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "runtime")]
mod execution {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::execution::operations::Operation;
    use crate::{Error, Result};
    use harmos_storage::StorageCursor;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::WriteSchema {
        type Error = Error;

        async fn run(self, context: crate::execution::Context<'_, Self>) -> Result<crate::execution::operations::Response<StorageCursor>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
                storage.write_schema_once(&self.mutation, self.schema.key.clone(), self.schema.graph_owner()).await.map_err(Into::into)
            }
            .await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> crate::execution::operations::FailureKind {
            crate::storage::classify(error)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
