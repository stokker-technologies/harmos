/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain, id = "provider.echo", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    }, visibility = Shared))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest, id = "provider.echo", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    }, visibility = Shared))]
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
