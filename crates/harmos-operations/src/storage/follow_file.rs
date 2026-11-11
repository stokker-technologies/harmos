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

#[cfg(feature = "runtime")]
use crate::execution::operations::access::GrantAccess;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(FollowFile {
    source: Nested<FileSource> => 1,
    start: Nested<FollowStart> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.follow_file",
    response = FileCompletion,
    policy = IO_POLICY,
    output = FileEvent,
    visibility = Public,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "FollowFile"),
    grant = Self::grant_for,
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowFile {
    pub source: FileSource,
    pub start: FollowStart,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FollowFile {
    fn grant_for(request: &FollowFile) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for FollowFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.start.memory_size().saturating_sub(std::mem::size_of_val(&self.start)))
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
    use super::*;
    use crate::execution::operations::{Operation, OperationDeclaration};
    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};
    use harmos_domain::model::storage::files::FileCompletion;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::FollowFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.follow_file", level = "trace", skip_all)]
        async fn run(self, context: crate::execution::Context<'_, Self>) -> Result<crate::execution::operations::Response<FileCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<crate::execution::operations::access::GrantAccess>();
            let output = context.streams();

            let host = grants;
            let ctx = &host.0;
            let grant = self.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(FollowFile::ID, scope));
            if let Some(grant) = &grant {
                ctx.admit(grant).map_err(Error::Access)?;
            }
            let mut follower = storage
                .follow_file(harmos_storage::FileFollow {
                    source: self.source.clone(),
                    start: self.start,
                })
                .await?;
            while let Some(event) = follower.next().await? {
                if let Some(grant) = &grant {
                    ctx.admit(grant).map_err(Error::Access)?;
                }
                let response = crate::execution::operations::Response::from_owner(event.owner());
                storage.clock().timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| crate::storage::output_timeout())??;
            }
            Ok(follower.completion()?.into())
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
