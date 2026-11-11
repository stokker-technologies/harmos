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
use harmos_domain::model::storage::StorageInfo;
use harmos_domain::model::storage::codec::product;
use harmos_domain::operations::ReplaySafety;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(StorageStatus {});

/* === Type Aliases === */

/* === Structs === */

// Read The Storage Frontiers, Mutation Window And File Batch Size.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.status",
    response = StorageInfo,
    policy = POLICY,
    visibility = Private,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageStatus {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StorageStatus {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "runtime")]
mod execution {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::execution::operations::Operation;
    use crate::{Error, Result};
    use harmos_storage::StorageInfo;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::StorageStatus {
        type Error = Error;

        async fn run(self, context: crate::execution::Context<'_, Self>) -> Result<crate::execution::operations::Response<StorageInfo>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<StorageInfo, Self::Error> = async move { storage.info().map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> crate::execution::operations::FailureKind {
            crate::storage::classify(error)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
