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
use harmos::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

pub const POLICY: OperationPolicy =
    OperationPolicy { timeout: Duration::from_secs(1), attempt_timeout: Duration::from_secs(1), cancellation_timeout: std::time::Duration::ZERO, retries: RetryPolicy::Never };

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
