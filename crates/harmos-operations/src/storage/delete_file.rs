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
use harmos_domain::operations::ReplaySafety;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(DeleteFile {
    mutation: Nested<MutationId> => 1,
    file: Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Delete A Staged File, Exactly Once Per Mutation.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.delete_file",
    response = StagedFile,
    policy = POLICY,
    visibility = Private,
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

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "runtime")]
mod execution {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::execution::operations::Operation;
    use crate::{Error, Result};
    use harmos_storage::StagedFile;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::DeleteFile {
        type Error = Error;

        async fn run(self, context: crate::execution::Context<'_, Self>) -> Result<crate::execution::operations::Response<StagedFile>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<StagedFile, Self::Error> =
                async move { storage.delete_file_once(&self.mutation, &self.file).await.map_err(Into::into) }.await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> crate::execution::operations::FailureKind {
            crate::storage::classify(error)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
