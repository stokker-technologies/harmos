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
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry, DirectorySelection};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ListDirectoryRequest { selection: Nested<DirectorySelection> => 1 });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListDirectoryRequest {
    pub selection: DirectorySelection,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_list_directory",
    request = ListDirectoryRequest,
    response = DirectoryCompletion,
    policy = IO_POLICY,
    infer_request,
    output = DirectoryEntry,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "ListDirectory"))]
pub struct ListDirectory {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for ListDirectoryRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Finite Storage Query Operation

    // Standard Library Imports
    // Third Party Imports
    // Local Imports
    use crate::{Error, Result};
    use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry};
    use harmos_sdk::operations::{Operation, Response, StreamOutput};
    use harmos_storage::StorageManager;

    impl Operation for super::ListDirectory {
        type Request = <ListDirectory as harmos_domain::OperationDeclaration>::Request;
        type Response = <ListDirectory as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = StreamOutput<DirectoryEntry>;

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.list_directory", level = "trace", skip_all)]
        async fn run(
            &mut self, _ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
        ) -> Result<Response<DirectoryCompletion>> {
            let mut reader = self.storage.list_directory(request.selection.clone()).await?;
            while let Some(item) = reader.next().await? {
                let response = Response::from_owner(item.owner());
                tokio::time::timeout(crate::storage::IO_TIMEOUT, output.send_owned(response))
                    .await
                    .map_err(|_| crate::storage::output_timeout())??;
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
