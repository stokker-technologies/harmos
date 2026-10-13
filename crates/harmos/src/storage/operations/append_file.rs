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

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};
use crate::storage::FileProducer;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(AppendFileRequest {
    mutation: crate::storage::codec::Nested<crate::storage::MutationId> => 1,
    session: crate::storage::codec::Nested<crate::storage::FileSession> => 2,
    offset: crate::storage::codec::Unsigned => 3,
    bytes: crate::storage::codec::Bytes => 4,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFileRequest {
    pub mutation: crate::storage::MutationId,
    pub session: crate::storage::FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

pub struct AppendFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for AppendFile {
    type Request = AppendFileRequest;
    type Response = FileProducer;
    const ID: &'static str = "storage.append_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for AppendFile {
    const VERSION: u32 = 1;
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = AppendFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();
    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut AppendFileRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProducer>> {
        let result: ::core::result::Result<FileProducer, Self::Error> = async move {
            self.storage
                .append_file_once(
                    &request.mutation,
                    &request.session,
                    request.offset,
                    &request.bytes,
                )
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

impl crate::MemorySize for AppendFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<crate::storage::MutationId>()
            + self.session.memory_size()
            - std::mem::size_of::<crate::storage::FileSession>()
            + self.offset.memory_size()
            - std::mem::size_of::<u64>()
            + self.bytes.memory_size()
            - std::mem::size_of::<Vec<u8>>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
