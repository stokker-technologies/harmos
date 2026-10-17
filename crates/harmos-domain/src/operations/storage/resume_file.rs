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
use crate::model::storage::FileProducer;
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ResumeFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ResumeFile {
    type Request = FileMutation;
    type Response = FileProducer;
    const ID: &'static str = "storage.resume_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for ResumeFile {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
