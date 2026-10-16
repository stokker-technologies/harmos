/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finite Storage Query Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
use crate::operations::{
    Context, Grpc, Operation, OperationDeclaration, OutputDeclaration, ReplaySafety, StreamOutput,
    Visibility, WireOperation,
};
use crate::schema::Described;
use crate::storage::StorageManager;
use crate::storage::config::IO_TIMEOUT;
use crate::storage::query::{Query as Request, QueryCompletion, QueryOutput};
use crate::{Error, MemorySize, Result};
use harmos_contract::plugin::Grant;
use std::{any::TypeId, sync::Arc};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Query;
pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Query {
    type Request = Request;
    type Response = QueryCompletion;
    const ID: &'static str = "storage.query";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    fn grant(request: &Request) -> Option<Grant> {
        Some(Grant::of(Self::ID, request.source.grant_scope()))
    }
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<QueryOutput>()))
    }
}
impl OutputDeclaration for Query {
    type Item = QueryOutput;
}
impl WireOperation for Query {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("Query"));
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(QueryOutput::DEFINITION);
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.download::<Self>())
        }
    }
}
impl<C: 'static> Operation for Handler<C> {
    type Declaration = Query;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<QueryOutput>;
    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }
    async fn run<'a>(
        &mut self,
        ctx: Self::Context<'a>,
        request: &mut Request,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<QueryCompletion>> {
        let grant = Grant::of(Query::ID, request.source.grant_scope());
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self.storage.query(request.clone()).await?;
        while let Some(item) = reader.next().await? {
            ctx.admit_stream_grant(&grant)?;
            tokio::time::timeout(
                IO_TIMEOUT,
                output.send_with(item.memory_size(), || Ok(item)),
            )
            .await
            .map_err(|_| super::output_timeout())??;
        }
        Ok(reader.completion()?.into())
    }
    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
