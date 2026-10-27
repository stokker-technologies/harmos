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

product!(WriteStreamsRequest {
    scope: Optional<Id> => 1,
});

#[harmos::operation(harmos_path = crate,
    id = "storage_write_streams",
    request = WriteStreamsRequest,
    response = WriteCompletion,
    policy = IO_POLICY,
    infer_request,
    input = DataBatch,
    output = WriteReceipt,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteStreams"))]
pub struct WriteStreams {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Type Aliases === */

/* === Structs === */

// Admit Streamed Arrow Batches Of Stream Rows Into One Optional Scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteStreamsRequest {
    pub scope: Option<Identifier>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteStreamsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of_val(&self.scope)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

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

    impl Operation for super::WriteStreams {
        type Request = <WriteStreams as harmos_domain::OperationDeclaration>::Request;
        type Response = <WriteStreams as harmos_domain::OperationDeclaration>::Response;
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
            let completion = exchange(&self.storage, &ctx, request.scope.as_ref(), endpoints, WriteKind::Streams, WriteStreams::ID).await?;
            Ok(completion.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
