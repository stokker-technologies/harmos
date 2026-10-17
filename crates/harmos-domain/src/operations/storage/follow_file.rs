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
use crate::model::storage::follow::File;
use crate::model::storage::{FileBatch, FileProgress};
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowFile {
    type Request = File;
    type Response = FileProgress;
    const ID: &'static str = "storage.follow_file";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBatch>()))
    }
}

impl OutputDeclaration for FollowFile {
    type Item = FileBatch;
}

impl WireOperation for FollowFile {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
