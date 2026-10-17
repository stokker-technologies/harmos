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
use harmos_runtime::operations::{OperationPolicy, RestartPolicy, RetryPolicy, Visibility};

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
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "process.core.inspect",
    version = 1,
    policy = POLICY,
    response = String,
    visibility = Visibility::Host,
)]
#[derive(Clone, PartialEq)]
pub struct Inspect {
    pub name: String,
}

/* === Enums === */
#[harmos_runtime::catalog]
pub enum Queries {
    Inspect(Inspect),
}

/* === Traits === */

/* === Implementations === */
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
