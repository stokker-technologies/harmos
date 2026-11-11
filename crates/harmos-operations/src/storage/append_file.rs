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
use harmos_domain::model::storage::codec::{Bytes, Nested, Unsigned, product};
use harmos_domain::model::storage::{FileProducer, FileSession, MutationId};
use harmos_domain::operations::ReplaySafety;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(AppendFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
    offset: Unsigned => 3,
    bytes: Bytes => 4,
});

/* === Type Aliases === */

/* === Structs === */

// Append Bytes At An Offset Of A Producing File Session.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.append_file",
    response = FileProducer,
    policy = POLICY,
    visibility = Private,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFile {
    pub mutation: MutationId,
    pub session: FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for AppendFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
            .saturating_add(self.bytes.memory_size().saturating_sub(std::mem::size_of_val(&self.bytes)))
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
    use harmos_storage::{FileChunk, FileProducer};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::AppendFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.append_file", level = "trace", skip_all)]
        async fn run(
            mut self, context: crate::execution::Context<'_, Self>,
        ) -> Result<crate::execution::operations::Response<FileProducer>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let payload = FileChunk::new(std::mem::take(&mut self.bytes));
            let result: ::core::result::Result<FileProducer, Self::Error> =
                async move { storage.append_file_once(&self.mutation, &self.session, self.offset, payload).await.map_err(Into::into) }
                    .await;
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
