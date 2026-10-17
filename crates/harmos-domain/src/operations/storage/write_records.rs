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
// Third Party Imports
// Local Imports
use crate::model::storage::writes::{DataBatch, WriteCompletion, WriteReceipt, WriteRequest};
use crate::operations::{
    Grpc, InputDeclaration, OperationDeclaration, OutputDeclaration, ReplaySafety, Visibility,
    WireOperation,
};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct WriteRecords;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteRecords {
    type Request = WriteRequest;
    type Response = WriteCompletion;
    const ID: &'static str = "storage.write_records";
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

impl InputDeclaration for WriteRecords {
    type Item = DataBatch;
}

impl OutputDeclaration for WriteRecords {
    type Item = WriteReceipt;
}

impl WireOperation for WriteRecords {
    const VERSION: u32 = 1;
    const INPUT: Option<crate::schema::declaration::Definition> = Some(DataBatch::DEFINITION);
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(WriteReceipt::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("WriteRecords"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.exchange::<Self>())
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
