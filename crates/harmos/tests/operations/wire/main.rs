/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../support/app/minimal.rs"]
mod app;
#[path = "authored_lifecycle.rs"]
mod authored_lifecycle;
#[path = "../../support/operations_fixture.rs"]
mod fixture;
#[path = "../../support/runtime_probe.rs"]
mod probe;
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;
#[path = "../../support/mod.rs"]
mod support;
use fixture::*;
#[cfg(any(feature = "guest", feature = "sidecar"))]
#[path = "artifact.rs"]
mod artifact;
#[path = "authored.rs"]
mod authored;
#[path = "endpoints.rs"]
mod endpoints;
#[cfg(feature = "grpc")]
#[path = "grpc.rs"]
mod grpc;
#[cfg(feature = "grpc")]
#[path = "grpc_manager.rs"]
mod grpc_manager;
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
#[path = "host.rs"]
mod host;
#[path = "owned_wire.rs"]
mod owned_wire;
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
#[path = "plugin.rs"]
mod plugin;
#[path = "cases.rs"]
mod wire;

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
fn portable_calls() {
    authored::contract_scenario();
    owned_wire::contract_scenario();
    wire::contract_scenario();
    endpoints::contract_scenario();
}

#[test]
fn call_projections() {
    #[cfg(any(feature = "guest", feature = "sidecar"))]
    artifact::contract_scenario();
    #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
    host::contract_scenario();
    #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
    plugin::contract_scenario();
}

#[test]
fn grpc_calls() {
    #[cfg(feature = "grpc")]
    grpc::contract_scenario();
    #[cfg(feature = "grpc")]
    grpc_manager::contract_scenario();
}
