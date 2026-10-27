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
use harmos_domain::Identifier;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Id, Optional, product};
use harmos_domain::model::storage::{DataBatch, WriteCompletion, WriteReceipt};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WriteRecordsRequest {
    scope: Optional<Id> => 1,
});

/* === Type Aliases === */

/* === Structs === */

// Admit Streamed Arrow Batches Of Records Into One Optional Scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteRecordsRequest {
    pub scope: Option<Identifier>,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_write_records",
    request = WriteRecordsRequest,
    response = WriteCompletion,
    policy = IO_POLICY,
    infer_request,
    input = DataBatch,
    output = WriteReceipt,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteRecords"))]
pub struct WriteRecords {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteRecordsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of_val(&self.scope)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    use crate::storage::exchange::{WriteKind, exchange};

    // Standard Library Imports
    // Third Party Imports
    // Local Imports
    use crate::{Error, Result};
    use harmos_sdk::operations::{Operation, OperationDeclaration, StreamInput, StreamOutput};
    use harmos_storage::StorageManager;
    use harmos_storage::writes::{DataBatch, WriteCompletion, WriteReceipt};

    impl Operation for super::WriteRecords {
        type Request = <WriteRecords as harmos_domain::OperationDeclaration>::Request;
        type Response = <WriteRecords as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = (StreamInput<DataBatch>, StreamOutput<WriteReceipt>);

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        async fn run(
            &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<WriteCompletion>> {
            let host = ctx.dependency::<harmos_sdk::operations::access::GrantAccess>()?;
            let ctx = &host.0;
            let completion = exchange(&self.storage, ctx, request.scope.as_ref(), endpoints, WriteKind::Records, WriteRecords::ID).await?;
            Ok(completion.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
