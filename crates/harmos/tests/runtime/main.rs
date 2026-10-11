/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/core_egress.rs"]
#[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
mod core_egress;
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../support/core_journal.rs"]
#[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
mod core_journal;

#[path = "../support/mod.rs"]
mod support;

#[path = "guest.rs"]
#[cfg(all(
    feature = "guest-host",
    feature = "guest",
    feature = "sidecar-host",
    feature = "sidecar"
))]
mod guest;

#[path = "plugin.rs"]
#[cfg(feature = "plugin")]
mod plugin;

#[path = "sidecar.rs"]
#[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
mod sidecar;

#[cfg(all(feature = "sidecar-host", feature = "sidecar"))]
mod observer_capabilities;

#[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
mod observer_demand;

#[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
mod observer_hosts;

#[cfg(all(feature = "sidecar-host", feature = "sidecar"))]
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

#[test]
fn observer_binding() {
    #[cfg(all(feature = "sidecar-host", feature = "sidecar"))]
    observer_capabilities::contract_scenario();
    #[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
    observer_demand::contract_scenario();
    #[cfg(all(feature = "sidecar-host", feature = "sidecar", feature = "guest"))]
    observer_hosts::contract_scenario();
}
