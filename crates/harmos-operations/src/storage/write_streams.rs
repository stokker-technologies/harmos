/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::IO_POLICY;
use harmos_domain::Identifier;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Id, Optional, product};
use harmos_domain::model::storage::{DataBatch, WriteCompletion, WriteReceipt};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::GrantAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(WriteStreams {
    scope: Optional<Id> => 1,
});

// Admit Streamed Arrow Batches Of Stream Rows Into One Optional Scope.
#[harmos::operation(
    harmos_path = crate,
    resources = [StorageManager<ErasedCatalog>, GrantAccess],
    codec = manual,
    id = "storage.write_streams",
    response = WriteCompletion,
    policy = IO_POLICY,
    input = DataBatch,
    output = WriteReceipt,
    visibility = Public,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteStreams"),
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteStreams {
    pub scope: Option<Identifier>,
}

impl MemorySize for WriteStreams {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of_val(&self.scope)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    use crate::storage::exchange::{WriteKind, exchange};

    use crate::{Error, Result};
    use harmos_sdk::operations::{Operation, OperationDeclaration};
    use harmos_storage::writes::WriteCompletion;

    impl Operation for super::WriteStreams {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<WriteCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let endpoints = context.streams();

            let host = grants;
            let ctx = &host.0;
            let completion = exchange(&storage, ctx, self.scope.as_ref(), endpoints, WriteKind::Streams, WriteStreams::ID).await?;
            Ok(completion.into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
