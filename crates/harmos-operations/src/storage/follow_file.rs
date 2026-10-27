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
use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::files::{FileCompletion, FileEvent, FileSource, FollowStart};
use harmos_domain::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowFileRequest {
    source: Nested<FileSource> => 1,
    start: Nested<FollowStart> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowFileRequest {
    pub source: FileSource,
    pub start: FollowStart,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_follow_file",
    request = FollowFileRequest,
    response = FileCompletion,
    policy = IO_POLICY,
    infer_request,
    output = FileEvent,
    visibility = Visibility::Shared,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "FollowFile"),
    grant = Self::grant_for)]
pub struct FollowFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FollowFile {
    fn grant_for(request: &FollowFileRequest) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for FollowFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.start.memory_size().saturating_sub(std::mem::size_of_val(&self.start)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_domain::model::storage::files::{FileCompletion, FileEvent};
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamOutput};

    use harmos_storage::StorageManager;

    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};

    impl Operation for super::FollowFile {
        type Request = <FollowFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <FollowFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<FileEvent>;
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_file", level = "trace", skip_all)]
        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileCompletion>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let grant = request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(FollowFile::ID, scope));
            if let Some(grant) = &grant {
                ctx.admit(grant).map_err(Error::Access)?;
            }
            let mut follower =
                self.storage.follow_file(harmos_storage::FileFollow { source: request.source.clone(), start: request.start }).await?;
            while let Some(event) = follower.next().await? {
                if let Some(grant) = &grant {
                    ctx.admit(grant).map_err(Error::Access)?;
                }
                let response = harmos_sdk::operations::Response::from_owner(event.owner());
                tokio::time::timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| crate::storage::output_timeout())??;
            }
            Ok(follower.completion()?.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
