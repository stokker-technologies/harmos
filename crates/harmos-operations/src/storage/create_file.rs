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
use harmos_domain::model::storage::{FileMetadata, FileProducer, MutationId};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(CreateFile {
    mutation: Nested<MutationId> => 1,
    metadata: Nested<FileMetadata> => 2,
});

/* === Type Aliases === */

/* === Structs === */

// Open A Producing File Session For New Metadata.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.create_file",
    response = FileProducer,
    policy = POLICY,
    visibility = Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFile {
    pub mutation: MutationId,
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for CreateFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::FileProducer;

    use crate::{Error, Result};

    impl Operation for super::CreateFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.create_file", level = "trace", skip_all)]
        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileProducer>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let result: ::core::result::Result<FileProducer, Self::Error> =
                async move { storage.create_file_once(&self.mutation, &self.metadata).await.map_err(Into::into) }.await;
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
