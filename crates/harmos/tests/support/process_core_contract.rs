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
use harmos::operations::{
    OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireOperation,
};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Inspect;

/* === Enums === */
#[harmos::catalog]
pub enum Queries {
    Inspect(Inspect),
}

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Inspect {
    type Request = String;
    type Response = String;
    const ID: &'static str = "process.core.inspect";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Inspect {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
