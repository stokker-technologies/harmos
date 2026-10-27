/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// HashFileRequest Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::files::{ByteRange, FileHash, FileSource};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(HashFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

#[harmos::operation(harmos_path = crate,
    id = "storage_hash_file",
    request = HashFileRequest,
    response = FileHash,
    policy = IO_POLICY,
    infer_request,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "HashFile"),
    grant = Self::grant_for)]
pub struct HashFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}
impl HashFile {
    fn grant_for(request: &HashFileRequest) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for HashFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // HashFile Runtime Handler.

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::{Error, Result};
    use harmos_domain::model::storage::files::FileHash;
    use harmos_sdk::operations::Operation;
    use harmos_storage::StorageManager;

    impl Operation for super::HashFile {
        type Request = <HashFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <HashFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.hash_file", level = "trace", skip_all)]
        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut (),
        ) -> Result<harmos_sdk::operations::Response<FileHash>> {
            Ok(self.storage.hash_file(harmos_storage::FileRead { source: request.source.clone(), range: request.range }).await?.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
