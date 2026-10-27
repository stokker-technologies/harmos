/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Provider Operation Contract And Execution.

/* === --- === Definition Module === --- === */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Duration, Echoed, OperationPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "provider.echo", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    }, visibility = Shared)]
#[derive(Clone, PartialEq)]

pub struct ProviderEchoOperation {
    pub say: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
