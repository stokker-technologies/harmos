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
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry, DirectorySelection};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(ListDirectory { selection: Nested<DirectorySelection> => 1 });

#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.list_directory",
    response = DirectoryCompletion,
    policy = IO_POLICY,
    output = DirectoryEntry,
    visibility = Public,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "ListDirectory"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListDirectory {
    pub selection: DirectorySelection,
}

impl MemorySize for ListDirectory {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    use crate::{Error, Result};
    use harmos_domain::model::storage::DirectoryCompletion;
    use harmos_sdk::operations::{Operation, Response};

    impl Operation for super::ListDirectory {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.list_directory", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<DirectoryCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let output = context.streams();

            let mut reader = storage.list_directory(self.selection.clone()).await?;
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
