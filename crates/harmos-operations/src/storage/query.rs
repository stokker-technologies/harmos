/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Declarative Storage Query: A Source At A Full Cursor, Ordered Transforms
// And A Final Row Slice. Results Cross The Wire As Arrow IPC.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::StorageCursor;
use harmos_domain::model::storage::codec::{Nested, Repeated, product};
use harmos_domain::model::storage::query::transform::Transform;
use harmos_domain::model::storage::query::{QueryCompletion, QueryOutput, RowSlice, Source};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::GrantAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(Query {
    at: Nested<StorageCursor> => 1,
    source: Nested<Source> => 2,
    transforms: Repeated<Transform> => 3,
    slice: Nested<RowSlice> => 4,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.query",
    response = QueryCompletion,
    policy = IO_POLICY,
    output = QueryOutput,
    visibility = Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "Query"),
    grant = Self::grant_for,
)]
#[derive(Clone, Debug)]
pub struct Query {
    pub at: StorageCursor,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub slice: RowSlice,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Query {
    fn grant_for(request: &Query) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.source.grant_scope()))
    }
}

impl MemorySize for Query {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.at.memory_size().saturating_sub(std::mem::size_of_val(&self.at)))
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.transforms.memory_size().saturating_sub(std::mem::size_of_val(&self.transforms)))
            .saturating_add(self.slice.memory_size().saturating_sub(std::mem::size_of_val(&self.slice)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Finite Storage Query Operation

    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};
    use harmos_domain::plugin::Grant;
    use harmos_sdk::operations::{Operation, OperationDeclaration};
    use harmos_storage::query::QueryCompletion;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    impl Operation for super::Query {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<QueryCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let output = context.streams();

            let host = grants;
            let ctx = &host.0;
            let grant = Grant::of(Query::ID, self.source.grant_scope());
            ctx.admit(&grant).map_err(Error::Access)?;
            let mut reader = storage
                .query(harmos_storage::Query {
                    at: self.at.clone(),
                    source: self.source.clone(),
                    transforms: self.transforms.clone(),
                    slice: self.slice,
                })
                .await?;
            while let Some(item) = reader.next().await? {
                ctx.admit(&grant).map_err(Error::Access)?;
                let bytes = item.output_memory_size();
                tokio::time::timeout(IO_TIMEOUT, output.send_with(bytes, || Ok(item.output())))
                    .await
                    .map_err(|_| crate::storage::output_timeout())??;
            }
            Ok(reader.completion()?.into())
        }
        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
