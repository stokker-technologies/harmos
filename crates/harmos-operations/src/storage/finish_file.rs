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
use harmos_domain::model::storage::{FileReference, FileSession, MutationId};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(FinishFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Finish A Producing File Session Into An Immutable Reference.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.finish_file",
    response = FileReference,
    policy = POLICY,
    visibility = Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishFile {
    pub mutation: MutationId,
    pub session: FileSession,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FinishFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::FileReference;

    use crate::{Error, Result};

    impl Operation for super::FinishFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.finish_file", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileReference>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<FileReference, Self::Error> =
                async move { storage.finish_file_once(&self.mutation, &self.session).await.map_err(Into::into) }.await;
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
