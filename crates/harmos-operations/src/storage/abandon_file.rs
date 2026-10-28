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
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::{FileProgress, FileSession, MutationId};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(AbandonFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

// Abandon A Producing File Session, Exactly Once Per Mutation.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.abandon_file",
    response = FileProgress,
    policy = POLICY,
    visibility = Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbandonFile {
    pub mutation: MutationId,
    pub session: FileSession,
}

impl MemorySize for AbandonFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    use harmos_sdk::operations::Operation;
    use harmos_storage::FileProgress;

    use crate::{Error, Result};

    impl Operation for super::AbandonFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.abandon_file", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileProgress>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<FileProgress, Self::Error> =
                async move { storage.abandon_file_once(&self.mutation, &self.session).await.map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
