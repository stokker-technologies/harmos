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
use harmos_domain::model::storage::codec::{Bytes, Nested, Unsigned, product};
use harmos_domain::model::storage::{FileProducer, FileSession, MutationId};
use harmos_domain::operations::ReplaySafety;
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

product!(AppendFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
    offset: Unsigned => 3,
    bytes: Bytes => 4,
});

// Append Bytes At An Offset Of A Producing File Session.
#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.append_file",
    response = FileProducer,
    policy = POLICY,
    visibility = Host,
    replay = ReplaySafety::Idempotent)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFile {
    pub mutation: MutationId,
    pub session: FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

impl MemorySize for AppendFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
            .saturating_add(self.bytes.memory_size().saturating_sub(std::mem::size_of_val(&self.bytes)))
    }
}

#[cfg(feature = "runtime")]
mod execution {

    use harmos_sdk::operations::Operation;
    use harmos_storage::{FileChunk, FileProducer};

    use crate::{Error, Result};

    impl Operation for super::AppendFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.append_file", level = "trace", skip_all)]
        async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<FileProducer>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            let payload = FileChunk::new(std::mem::take(&mut self.bytes));
            let result: ::core::result::Result<FileProducer, Self::Error> =
                async move { storage.append_file_once(&self.mutation, &self.session, self.offset, payload).await.map_err(Into::into) }
                    .await;
            result.map(::core::convert::Into::into)
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}
