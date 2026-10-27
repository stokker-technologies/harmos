/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// FollowProductionRequest Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, Unsigned, product};
use harmos_domain::model::storage::files::{FileBatch, FileProgress, StagedFile};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(FollowProductionRequest {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

#[harmos::operation(harmos_path = crate,
    id = "storage_follow_production",
    request = FollowProductionRequest,
    response = FileProgress,
    policy = IO_POLICY,
    infer_request,
    output = FileBatch,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "FollowProduction"),
    grant = Self::grant_for)]
pub struct FollowProduction {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}
impl FollowProduction {
    fn grant_for(request: &FollowProductionRequest) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.file.id.as_str().to_owned()))
    }
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowProductionRequest {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowProductionRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Follow An Existing Staged Production Without Reinterpreting Its Identity.

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamOutput};
    use harmos_storage::{FileBatch, FileProgress, StorageManager};

    impl Operation for super::FollowProduction {
        type Request = <FollowProduction as harmos_domain::OperationDeclaration>::Request;
        type Response = <FollowProduction as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<FileBatch>;

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_production", level = "trace", skip_all)]
        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileProgress>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let grant = harmos_domain::plugin::Grant::of(FollowProduction::ID, request.file.id.as_str().to_owned());
            ctx.admit(&grant).map_err(Error::Access)?;
            let mut follower = self.storage.follow_production(&request.file, request.offset).await?;
            let batch_bytes = self.storage.settings().response.batch_bytes;
            while let Some(batch) = follower.next_batch(batch_bytes).await? {
                ctx.admit(&grant).map_err(Error::Access)?;
                let response = harmos_sdk::operations::Response::from_owner(batch.owner());
                tokio::time::timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| crate::storage::output_timeout())??;
            }
            Ok(follower.progress().into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
