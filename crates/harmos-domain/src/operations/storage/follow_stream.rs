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
use crate::model::storage::follow::Stream;
use crate::model::storage::{StorageCursor, StoredRow};
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowStream;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowStream {
    type Request = Stream;
    type Response = StorageCursor;
    const ID: &'static str = "storage.follow_stream";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<StoredRow>()))
    }
}

impl OutputDeclaration for FollowStream {
    type Item = StoredRow;
}

impl WireOperation for FollowStream {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(StoredRow::DEFINITION);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
