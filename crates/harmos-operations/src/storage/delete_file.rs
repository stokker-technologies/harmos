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
use super::POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::{MutationId, StagedFile};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(DeleteFileRequest {
    mutation: Nested<MutationId> => 1,
    file: Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Delete A Staged File, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub mutation: MutationId,
    pub file: StagedFile,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_delete_file",
    request = DeleteFileRequest,
    response = StagedFile,
    policy = POLICY,
    infer_request,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
pub struct DeleteFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for DeleteFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;

    use harmos_storage::StagedFile;
    use harmos_storage::StorageManager;

    use crate::{Error, Result};

    impl Operation for super::DeleteFile {
        type Request = <DeleteFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <DeleteFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<StagedFile>> {
            let result: ::core::result::Result<StagedFile, Self::Error> =
                async move { self.storage.delete_file_once(&request.mutation, &request.file).await.map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
