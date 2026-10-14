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

use crate::storage::StorageManager;
use crate::storage::{MutationId, StorageCursor, StreamInput};

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(WriteStreamRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    rows: crate::storage::codec::Repeated<StreamInput> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteStreamRequest {
    pub mutation: MutationId,
    pub rows: Vec<StreamInput>,
}
pub struct WriteStream;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteStream {
    type Request = WriteStreamRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.write_stream";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for WriteStream {
    const VERSION: u32 = 1;
}
impl crate::MemorySize for WriteStreamRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.rows.memory_size()
            - std::mem::size_of::<Vec<StreamInput>>()
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteStream;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();
    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut WriteStreamRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            let values = request.rows.iter().map(StreamInput::borrowed);
            self.storage
                .admit_stream_values(&request.mutation, values)
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
