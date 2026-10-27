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
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WriteFileRequest {
    metadata: Nested<FileMetadata> => 1,
});

/* === Type Aliases === */

/* === Structs === */

// Write One Finished File From Streamed Batches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFileRequest {
    pub metadata: FileMetadata,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_write_file",
    request = WriteFileRequest,
    response = FileReference,
    policy = IO_POLICY,
    infer_request,
    input = FileBatch,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteFile"),
    grant = Self::grant_for)]
pub struct WriteFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WriteFile {
    fn grant_for(request: &WriteFileRequest) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, request.metadata.id.as_str().to_owned()))
    }
}

impl MemorySize for WriteFileRequest {
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
    use harmos_domain::model::storage::files::FileBatch;
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamInput};
    use harmos_storage::{FileReference, StorageManager};

    impl Operation for super::WriteFile {
        type Request = <WriteFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <WriteFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamInput<FileBatch>;

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, input: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileReference>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let grant = harmos_domain::plugin::Grant::of(WriteFile::ID, request.metadata.id.as_str().to_owned());
            ctx.admit(&grant).map_err(Error::Access)?;
            self.storage.retry_transient_persistence();
            let mut file = self.storage.create_file(request.metadata.clone()).await?;

            let mut offset = 0u64;
            while let Some(batch) = input.recv().await {
                let (batch, _claims) = batch.into_owned_request()?;
                ctx.admit(&grant).map_err(Error::Access)?;
                if batch.offset != offset || batch.bytes.is_empty() {
                    return Err(Error::InvalidInput { field: "file batch", reason: "nonempty batches must have contiguous byte offsets" });
                }
                let next = offset
                    .checked_add(batch.bytes.len() as u64)
                    .ok_or(Error::InvalidInput { field: "file batch offset", reason: "byte offset overflow" })?;
                file.write_all(batch.bytes).await?;
                offset = next;
            }

            ctx.admit(&grant).map_err(Error::Access)?;
            let reference = file.finish().await?;
            self.storage.await_durable(&reference.position).await?;
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
