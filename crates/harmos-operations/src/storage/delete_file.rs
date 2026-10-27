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
use harmos_domain::model::storage::{MutationId, StagedFile};
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(DeleteFile {
    mutation: Nested<MutationId> => 1,
    file: Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Delete A Staged File, Exactly Once Per Mutation.
#[harmos::operation(harmos_path = crate, resources = [harmos_storage::StorageManager<harmos_storage::ErasedCatalog>], codec = manual,
    id = "storage.delete_file",
    response = StagedFile,
    policy = POLICY,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFile {
    pub mutation: MutationId,
    pub file: StagedFile,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for DeleteFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;

    use harmos_storage::StagedFile;

    use crate::{Error, Result};

    impl Operation for super::DeleteFile {
        type Error = Error;

        async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StagedFile>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let request = &mut self;

            let result: ::core::result::Result<StagedFile, Self::Error> =
                async move { storage.delete_file_once(&request.mutation, &request.file).await.map_err(Into::into) }.await;
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
