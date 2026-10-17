/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/process_core_contract.rs"]
#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod process_core_contract;

#[path = "../support/mod.rs"]
mod support;

#[path = "guest.rs"]
#[cfg(all(feature = "plugin-guest", feature = "plugin-sidecar"))]
mod guest;

#[path = "plugin.rs"]
#[cfg(feature = "plugin")]
mod plugin;

#[path = "sidecar.rs"]
#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod sidecar;

#[cfg(feature = "plugin-sidecar")]
mod observer_host_accessor;

#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod observer_demand;

#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod observer_hosts;

#[cfg(feature = "plugin-sidecar")]
mod stream_consumption;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
