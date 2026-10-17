/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod payloads;

/* === Re-Exports === */
pub use payloads::{FollowingRow, Publication};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::{OperationDeclaration, OperationPolicy, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// A Sidecar Publishing One Ordered Batch Of Stream Rows To Its Host.
pub struct Publish;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Publish {
    type Request = Publication;
    type Response = ();
    const ID: &'static str = "host.publish";
    const POLICY: OperationPolicy = super::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl WireOperation for Publish {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
