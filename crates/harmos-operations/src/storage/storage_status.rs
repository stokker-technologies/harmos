/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::StorageInfo;
use harmos_domain::model::storage::codec::product;
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(StorageStatus {});

// Read The Storage Frontiers, Mutation Window And File Batch Size.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.status",
    response = StorageInfo,
    policy = POLICY,
    visibility = Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageStatus {}

impl MemorySize for StorageStatus {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(feature = "runtime")]
mod execution {

    use harmos_sdk::operations::Operation;
    use harmos_storage::StorageInfo;

    use crate::{Error, Result};

    impl Operation for super::StorageStatus {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StorageInfo>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<StorageInfo, Self::Error> = async move { storage.info().map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
