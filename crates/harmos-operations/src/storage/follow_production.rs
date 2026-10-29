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
use harmos_domain::model::storage::codec::{Nested, Unsigned, product};
use harmos_domain::model::storage::files::{FileBatch, FileProgress, StagedFile};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::GrantAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(FollowProduction {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Follow An Existing Staged Production Without Reinterpreting Its Identity.
#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.follow_production",
    response = FileProgress,
    policy = IO_POLICY,
    output = FileBatch,
    visibility = Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "FollowProduction"),
    grant = Self::grant_for,
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowProduction {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FollowProduction {
    fn grant_for(request: &FollowProduction) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.file.id.as_str().to_owned()))
    }
}

impl MemorySize for FollowProduction {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};
    use harmos_sdk::operations::{Operation, OperationDeclaration};
    use harmos_storage::FileProgress;

    impl Operation for super::FollowProduction {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_production", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileProgress>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let output = context.streams();

            let host = grants;
            let ctx = &host.0;
            let grant = harmos_domain::plugin::Grant::of(FollowProduction::ID, self.file.id.as_str().to_owned());
            ctx.admit(&grant).map_err(Error::Access)?;
            let mut follower = storage.follow_production(&self.file, self.offset).await?;
            let batch_bytes = storage.settings().response.batch_bytes;
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
