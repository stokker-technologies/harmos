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
use harmos::operations::{OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireOperation};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1), attempt_timeout: Duration::from_secs(1), retries: RetryPolicy::Never, restart: RestartPolicy::Never
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Observe;

/* === Enums === */
#[harmos::catalog]
pub enum Operations { Observe(Observe) }

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Observe {
    type Request = harmos::operations::observers::Control;
    type Response = ();
    const ID: &'static str = "fixture.observe";
    const POLICY: OperationPolicy = harmos::operations::observers::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Observe { const VERSION: u32 = 1; }

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
