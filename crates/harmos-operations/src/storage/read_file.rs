/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::files::FileSource;
use harmos_domain::model::storage::{ByteRange, FileBatch, FileCompletion};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::GrantAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(ReadFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

// Read A Bounded Byte Range Of A Stored Or Local File.
#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.read_file",
    response = FileCompletion,
    policy = IO_POLICY,
    output = FileBatch,
    visibility = Public,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "ReadFile"),
    grant = Self::grant_for,
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFile {
    pub source: FileSource,
    pub range: ByteRange,
}

impl ReadFile {
    fn grant_for(request: &ReadFile) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for ReadFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    use crate::storage::IO_TIMEOUT;
    use crate::{Error, Result};
    use harmos_domain::model::storage::files::FileCompletion;
    use harmos_sdk::operations::{Operation, OperationDeclaration};

    impl Operation for super::ReadFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.read_file", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let output = context.streams();

            let host = grants;
            let ctx = &host.0;
            let grant = self.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(ReadFile::ID, scope));
            if let Some(grant) = &grant {
                ctx.admit(grant).map_err(Error::Access)?;
            }
            let mut reader = storage
                .read_file(harmos_storage::FileRead {
                    source: self.source.clone(),
                    range: self.range,
                })
                .await?;
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
