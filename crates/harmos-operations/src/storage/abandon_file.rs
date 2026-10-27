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
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::{FileProgress, FileSession, MutationId};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(AbandonFileRequest {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Abandon A Producing File Session, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbandonFileRequest {
    pub mutation: MutationId,
    pub session: FileSession,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_abandon_file",
    request = AbandonFileRequest,
    response = FileProgress,
    policy = POLICY,
    infer_request,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
pub struct AbandonFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for AbandonFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::FileProgress;

    use harmos_storage::StorageManager;

    use crate::{Error, Result};

    impl Operation for super::AbandonFile {
        type Request = <AbandonFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <AbandonFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.abandon_file", level = "trace", skip_all)]
        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileProgress>> {
            let result: ::core::result::Result<FileProgress, Self::Error> =
                async move { self.storage.abandon_file_once(&request.mutation, &request.session).await.map_err(Into::into) }.await;
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
