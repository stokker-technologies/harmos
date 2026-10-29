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
use harmos_domain::model::storage::codec::{Flag, Nested, product};
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEvent, DirectorySelection};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(WatchDirectory { selection: Nested<DirectorySelection> => 1, include_existing: Flag => 2 });

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.watch_directory",
    response = DirectoryCompletion,
    policy = IO_POLICY,
    output = DirectoryEvent,
    visibility = Shared,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WatchDirectory"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchDirectory {
    pub selection: DirectorySelection,
    pub include_existing: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WatchDirectory {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports
    // Third Party Imports
    // Local Imports
    use crate::{Error, Result};
    use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEvent};
    use harmos_sdk::operations::{Operation, Response};

    impl Operation for super::WatchDirectory {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.watch_directory", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<DirectoryCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let output = context.streams();

            let mut reader = storage.watch_directory(self.selection.clone(), self.include_existing).await?;
            let mut entries = 0_u64;
            while let Some(item) = reader.next().await? {
                if matches!(&*item, DirectoryEvent::Existing(_) | DirectoryEvent::Created(_) | DirectoryEvent::Modified(_)) {
                    entries = entries.checked_add(1).ok_or(Error::ResourceExhausted { resource: "directory observation count" })?;
                }
                let response = Response::from_owner(item.owner());
                tokio::time::timeout(crate::storage::IO_TIMEOUT, output.send_owned(response))
                    .await
                    .map_err(|_| crate::storage::output_timeout())??;
            }
            Ok(DirectoryCompletion { entries }.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
