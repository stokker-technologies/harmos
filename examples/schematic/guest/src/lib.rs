/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

#![allow(unsafe_code, reason = "The Author Supplies One Named Component Registration Symbol")]

/* === Modules === */

mod operations;
#[cfg(feature = "benchmark")]
mod transfer;

/* === Re-Exports === */

/* === Imports === */

use harmos::wasm::Registration;
use harmos::{Context, OperationPolicy};
#[cfg(not(feature = "benchmark"))]
use operations::plugin;
#[cfg(feature = "benchmark")]
use transfer::plugin;

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Some(harmos::encoding::Duration { seconds: 10, nanos: 0 }),
    attempt_timeout: Some(harmos::encoding::Duration { seconds: 10, nanos: 0 }),
    cancellation_timeout: None,
    attempts: 1,
    backoff: None,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[unsafe(no_mangle)]
pub fn harmos_register() -> Registration {
    Registration::new(plugin()).capacity(8).lifecycle_policy(POLICY, POLICY).on_initialize(async |_: &Context, configuration: Vec<u8>| {
        serde_json::from_slice::<()>(&configuration).map_err(|error| harmos::Error::Invalid(error.to_string()))?;
        Ok(())
    })
}

/* === Private Functions === */

/* === Tests === */
