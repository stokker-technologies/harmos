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
use std::any::TypeId;
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{
    Context, Grpc, InputDeclaration, Operation, OperationDeclaration, OutputDeclaration,
    ReplaySafety, StreamInput, StreamOutput, Visibility, WireOperation,
};
use crate::schema::Described;
use crate::storage::StorageManager;
use crate::storage::writes::{
    DataBatch, WriteCompletion, WriteKind, WriteReceipt, WriteRequest, exchange,
};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct WriteStreams;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteStreams {
    type Request = WriteRequest;
    type Response = WriteCompletion;
    const ID: &'static str = "storage.write_streams";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (
            Some(TypeId::of::<DataBatch>()),
            Some(TypeId::of::<WriteReceipt>()),
        )
    }
}

impl InputDeclaration for WriteStreams {
    type Item = DataBatch;
}
impl OutputDeclaration for WriteStreams {
    type Item = WriteReceipt;
}

impl WireOperation for WriteStreams {
    const VERSION: u32 = 1;
    const INPUT: Option<crate::schema::declaration::Definition> = Some(DataBatch::DEFINITION);
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(WriteReceipt::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("WriteStreams"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.exchange::<Self>())
        }
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteStreams;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = (StreamInput<DataBatch>, StreamOutput<WriteReceipt>);

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        ctx: Self::Context<'a>,
        request: &mut WriteRequest,
        endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<WriteCompletion>> {
        let completion = exchange(
            &self.storage,
            &ctx,
            request,
            endpoints,
            WriteKind::Streams,
            WriteStreams::ID,
        )
        .await?;
        Ok(completion.into())
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
