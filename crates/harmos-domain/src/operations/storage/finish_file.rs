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
// Third Party Imports
// Local Imports
use super::FileMutation;
use crate::model::storage::FileReference;
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FinishFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FinishFile {
    type Request = FileMutation;
    type Response = FileReference;
    const ID: &'static str = "storage.finish_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for FinishFile {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
