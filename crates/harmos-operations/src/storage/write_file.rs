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
use harmos_domain::model::storage::{FileBatch, FileMetadata, FileReference};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::GrantAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(WriteFile {
    metadata: Nested<FileMetadata> => 1,
});

/* === Type Aliases === */

/* === Structs === */

// Write One Finished File From Streamed Batches.
#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.write_file",
    response = FileReference,
    policy = IO_POLICY,
    input = FileBatch,
    visibility = Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteFile"),
    grant = Self::grant_for,
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFile {
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WriteFile {
    fn grant_for(request: &WriteFile) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.metadata.id.as_str().to_owned()))
    }
}

impl MemorySize for WriteFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports
    // Third Party Imports
    // Local Imports
    use crate::{Error, Result};
    use harmos_sdk::operations::{Operation, OperationDeclaration};
    use harmos_storage::FileReference;

    impl Operation for super::WriteFile {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileReference>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let input = context.streams();

            let host = grants;
            let ctx = &host.0;
            let grant = harmos_domain::plugin::Grant::of(WriteFile::ID, self.metadata.id.as_str().to_owned());
            ctx.admit(&grant).map_err(Error::Access)?;
            storage.retry_transient_persistence();
            let mut file = storage.create_file(self.metadata.clone()).await?;

            let mut offset = 0u64;
            while let Some(batch) = input.recv().await {
                let (batch, _claims) = batch.into_owned_request()?;
                ctx.admit(&grant).map_err(Error::Access)?;
                if batch.offset != offset || batch.bytes.is_empty() {
                    return Err(Error::InvalidInput {
                        field: "file batch",
                        reason: "nonempty batches must have contiguous byte offsets",
                    });
                }
                let next = offset.checked_add(batch.bytes.len() as u64).ok_or(Error::InvalidInput {
                    field: "file batch offset",
                    reason: "byte offset overflow",
                })?;
                file.write_all(batch.bytes).await?;
                offset = next;
            }

            ctx.admit(&grant).map_err(Error::Access)?;
            let reference = file.finish().await?;
            storage.await_durable(&reference.position).await?;
            Ok(reference.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
