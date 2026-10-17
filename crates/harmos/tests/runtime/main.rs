/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../harmos-guest/examples/operation_component/contract.rs"]
#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod operation_component_contract;
#[path = "../support/process_core_contract.rs"]
#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod process_core_contract;
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

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

#[test]
fn observer_binding() {
    #[cfg(feature = "plugin-sidecar")]
    observer_host_accessor::contract_scenario();
    #[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
    observer_demand::contract_scenario();
    #[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
    observer_hosts::contract_scenario();
}
