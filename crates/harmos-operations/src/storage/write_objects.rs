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

product!(WriteObjects {
    scope: Optional<Id> => 1,
});

/* === Type Aliases === */

/* === Structs === */

// Admit Streamed Arrow Batches Of Objects Into One Optional Scope.
#[harmos::operation(
    harmos_path = crate,
    resources = [harmos_storage::StorageManager<harmos_storage::ErasedCatalog>, harmos_sdk::operations::access::GrantAccess],
    codec = manual,
    id = "storage.write_objects",
    response = WriteCompletion,
    policy = IO_POLICY,
    input = DataBatch,
    output = WriteReceipt,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "WriteObjects"),
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteObjects {
    pub scope: Option<Identifier>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteObjects {
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
    use harmos_sdk::operations::{Operation, OperationDeclaration};
    use harmos_storage::writes::WriteCompletion;

    impl Operation for super::WriteObjects {
        type Error = Error;

        async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<WriteCompletion>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let grants = context.resource::<harmos_sdk::operations::access::GrantAccess>();
            let request = &mut self;
            let endpoints = context.streams();

            let host = grants;
            let ctx = &host.0;
            let completion = exchange(&storage, ctx, request.scope.as_ref(), endpoints, WriteKind::Objects, WriteObjects::ID).await?;
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
