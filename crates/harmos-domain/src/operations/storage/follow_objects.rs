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
use crate::model::storage::follow::Objects;
use crate::model::storage::{ObjectEntry, StorageCursor};
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowObjects;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowObjects {
    type Request = Objects;
    type Response = StorageCursor;
    const ID: &'static str = "storage.follow_objects";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<ObjectEntry>()))
    }
}

impl OutputDeclaration for FollowObjects {
    type Item = ObjectEntry;
}

impl WireOperation for FollowObjects {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(ObjectEntry::DEFINITION);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
