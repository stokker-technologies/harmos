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
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(StorageStatusRequest {});

/* === Type Aliases === */

/* === Structs === */

// Read The Storage Frontiers, Mutation Window And File Batch Size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageStatusRequest {}

#[harmos::operation(harmos_path = crate,
    id = "storage_status",
    request = StorageStatusRequest,
    response = StorageInfo,
    policy = POLICY,
    infer_request,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
pub struct StorageStatus {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StorageStatusRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::StorageInfo;

    use harmos_storage::StorageManager;

    use crate::{Error, Result};

    impl Operation for super::StorageStatus {
        type Request = <StorageStatus as harmos_domain::OperationDeclaration>::Request;
        type Response = <StorageStatus as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        async fn run(
            &mut self, _: harmos_sdk::OperationContext, _request: &mut Self::Request, _: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<StorageInfo>> {
            let result: ::core::result::Result<StorageInfo, Self::Error> = async move { self.storage.info().map_err(Into::into) }.await;
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
