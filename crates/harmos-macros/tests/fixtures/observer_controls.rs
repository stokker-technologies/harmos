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
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::operations::{OperationDeclaration, OperationPolicy, RetryPolicy, Visibility, WireOperation};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1), attempt_timeout: Duration::from_secs(1), retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Start;

pub struct Stop;

/* === Enums === */
#[harmos::catalog]
pub enum Operations { Start(Start), Stop(Stop) }

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Start {
    type Request = harmos::operations::observers::Control;
    type Response = ();
    const ID: &'static str = "fixture.start";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Start { const VERSION: u32 = 1; }

impl OperationDeclaration for Stop {
    type Request = harmos::operations::observers::Control;
    type Response = harmos::operations::observers::Status;
    const ID: &'static str = "fixture.stop";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Stop { const VERSION: u32 = 1; }

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
