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
// Standard Library Imports
use std::any::TypeId;
// Third Party Imports
// Local Imports
use crate::model::storage::query::{Query as Request, QueryCompletion, QueryOutput};
use crate::operations::{
    Grpc, OperationDeclaration, OutputDeclaration, ReplaySafety, Visibility, WireOperation,
};
use crate::plugin::Grant;
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Query;

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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
