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
use harmos_domain::model::storage::files::FileSource;
use harmos_domain::model::storage::{ByteRange, FileBatch, FileCompletion};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ReadFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Read A Bounded Byte Range Of A Stored Or Local File.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_read_file",
    request = ReadFileRequest,
    response = FileCompletion,
    policy = IO_POLICY,
    infer_request,
    output = FileBatch,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "ReadFile"),
    grant = Self::grant_for)]
pub struct ReadFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReadFile {
    fn grant_for(request: &ReadFileRequest) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for ReadFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
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
    use harmos_domain::model::storage::files::{FileBatch, FileCompletion};
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamOutput};
    use harmos_storage::StorageManager;

    impl Operation for super::ReadFile {
        type Request = <ReadFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <ReadFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<FileBatch>;

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.read_file", level = "trace", skip_all)]
        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileCompletion>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let grant = request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(ReadFile::ID, scope));
            if let Some(grant) = &grant {
                ctx.admit(grant).map_err(Error::Access)?;
            }
            let mut reader =
                self.storage.read_file(harmos_storage::FileRead { source: request.source.clone(), range: request.range }).await?;
            while let Some(batch) = reader.next().await? {
                if let Some(grant) = &grant {
                    ctx.admit(grant).map_err(Error::Access)?;
                }
                let response = harmos_sdk::operations::Response::from_owner(batch.owner());
                tokio::time::timeout(IO_TIMEOUT, output.send_owned(response)).await.map_err(|_| crate::storage::output_timeout())??;
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
