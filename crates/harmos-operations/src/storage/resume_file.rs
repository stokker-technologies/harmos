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
use harmos_domain::model::storage::{FileProducer, FileSession, MutationId};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ResumeFileRequest {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

#[harmos::operation(harmos_path = crate,
    id = "storage_resume_file",
    request = ResumeFileRequest,
    response = FileProducer,
    policy = POLICY,
    infer_request,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
pub struct ResumeFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Type Aliases === */

/* === Structs === */

// Resume A Producing File Session After Reconnection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeFileRequest {
    pub mutation: MutationId,
    pub session: FileSession,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for ResumeFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::FileProducer;

    use harmos_storage::StorageManager;

    use crate::{Error, Result};

    impl Operation for super::ResumeFile {
        type Request = <ResumeFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <ResumeFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.resume_file", level = "trace", skip_all)]
        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileProducer>> {
            let result: ::core::result::Result<FileProducer, Self::Error> =
                async move { self.storage.resume_file_once(&request.mutation, &request.session).await.map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
