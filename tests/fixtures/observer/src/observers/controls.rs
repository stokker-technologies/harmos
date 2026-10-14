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
use harmos::operations::observers::{Control, POLICY};
use harmos::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The one Host control that runs each observer until the host cancels it.
pub struct Observe;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Observe {
    type Request = Control;
    type Response = ();
    const ID: &'static str = "standalone-bridge.observers.observe";
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const POLICY: harmos::operations::OperationPolicy = POLICY;
}

impl WireOperation for Observe {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
