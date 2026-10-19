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
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy};

/* === Constants === */

pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1), attempt_timeout: Duration::from_secs(1), retries: RetryPolicy::Never, restart: RestartPolicy::Never
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */


/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
