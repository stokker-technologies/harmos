/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Declarative Storage QueryRequest: A Source At A Full Cursor, Ordered Transforms
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
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(QueryRequest {
    at: Nested<StorageCursor> => 1,
    source: Nested<Source> => 2,
    transforms: Repeated<Transform> => 3,
    slice: Nested<RowSlice> => 4,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct QueryRequest {
    pub at: StorageCursor,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub slice: RowSlice,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_query",
    request = QueryRequest,
    response = QueryCompletion,
    policy = IO_POLICY,
    infer_request,
    output = QueryOutput,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "Query"),
    grant = Self::grant_for)]
pub struct Query {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Query {
    fn grant_for(request: &QueryRequest) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.source.grant_scope()))
    }
}

impl MemorySize for QueryRequest {
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
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamOutput};
    use harmos_storage::StorageManager;
    use harmos_storage::query::{QueryCompletion, QueryOutput};

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    impl Operation for super::Query {
        type Request = <Query as harmos_domain::OperationDeclaration>::Request;
        type Response = <Query as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<QueryOutput>;
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }
        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<QueryCompletion>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let grant = Grant::of(Query::ID, request.source.grant_scope());
            ctx.admit(&grant).map_err(Error::Access)?;
            let mut reader = self
                .storage
                .query(harmos_storage::Query {
                    at: request.at.clone(),
                    source: request.source.clone(),
                    transforms: request.transforms.clone(),
                    slice: request.slice,
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
