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
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(AppendFileRequest {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
    offset: Unsigned => 3,
    bytes: Bytes => 4,
});

/* === Type Aliases === */

/* === Structs === */

// Append Bytes At An Offset Of A Producing File Session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFileRequest {
    pub mutation: MutationId,
    pub session: FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_append_file",
    request = AppendFileRequest,
    response = FileProducer,
    policy = POLICY,
    infer_request,
    visibility = Visibility::Host,
    replay = ReplaySafety::Idempotent)]
pub struct AppendFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
    #[cfg(feature = "runtime")]
    payload: Option<harmos_storage::FileChunk>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for AppendFileRequest {
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
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;
    use harmos_storage::{FileChunk, FileProducer};

    use harmos_storage::StorageManager;

    use crate::{Error, Result};

    impl Operation for super::AppendFile {
        type Request = <AppendFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <AppendFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();
        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage, payload: None })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.append_file", level = "trace", skip_all)]
        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
        ) -> Result<harmos_sdk::operations::Response<FileProducer>> {
            if self.payload.is_none() {
                let bytes = std::mem::take(&mut request.bytes);
                self.payload = Some(FileChunk::new(bytes));
            }
            let payload = self.payload.as_ref().expect("retained file payload").clone();
            let result: ::core::result::Result<FileProducer, Self::Error> = async move {
                self.storage.append_file_once(&request.mutation, &request.session, request.offset, payload).await.map_err(Into::into)
            }
            .await;
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
