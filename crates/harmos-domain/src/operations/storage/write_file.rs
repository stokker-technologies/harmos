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
use crate::model::storage::files::FileBatch;
use crate::model::storage::{FileMetadata, FileReference};
use crate::operations::{
    Grpc, InputDeclaration, OperationDeclaration, ReplaySafety, Visibility, WireOperation,
};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct WriteFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteFile {
    type Request = FileMetadata;
    type Response = FileReference;
    const ID: &'static str = "storage.write_file";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;

    fn grant(request: &FileMetadata) -> Option<crate::plugin::Grant> {
        Some(crate::plugin::Grant::of(
            Self::ID,
            request.id.as_str().to_owned(),
        ))
    }

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (Some(TypeId::of::<FileBatch>()), None)
    }
}

impl InputDeclaration for WriteFile {
    type Item = FileBatch;
}

impl WireOperation for WriteFile {
    const VERSION: u32 = 1;
    const INPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("WriteFile"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.upload::<Self>())
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
