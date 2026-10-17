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
use crate::model::storage::files::{FileBatch, FileCompletion, ReadFile as ReadRequest};
use crate::operations::{
    Grpc, OperationDeclaration, OutputDeclaration, ReplaySafety, Visibility, WireOperation,
};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ReadFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ReadFile {
    type Request = ReadRequest;
    type Response = FileCompletion;
    const ID: &'static str = "storage.read_file";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    fn grant(request: &ReadRequest) -> Option<crate::plugin::Grant> {
        Some(crate::plugin::Grant::of(
            Self::ID,
            request.file.metadata.id.as_str().to_owned(),
        ))
    }
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBatch>()))
    }
}

impl OutputDeclaration for ReadFile {
    type Item = FileBatch;
}

impl WireOperation for ReadFile {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("ReadFile"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> { Some(visitor.download::<Self>()) }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
