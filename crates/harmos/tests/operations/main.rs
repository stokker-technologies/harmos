/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/app/minimal.rs"]
mod app;
#[path = "../support/operations_fixture.rs"]
mod fixture;
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;
#[path = "../support/mod.rs"]
mod support;
use fixture::*;
#[path = "../operations/admission.rs"]
mod admission;
#[path = "../operations/causation.rs"]
mod causation;
#[path = "../operations/children.rs"]
mod children;
#[path = "../operations/clock.rs"]
mod clock;
#[path = "../operations/execution.rs"]
mod execution;
#[path = "../operations/finalization.rs"]
mod finalization;
#[path = "../operations/host.rs"]
mod host;
#[path = "../operations/lanes.rs"]
mod lanes;
#[path = "../operations/middleware.rs"]
mod middleware;
#[path = "../operations/outcomes.rs"]
mod outcomes;
#[path = "../operations/policy.rs"]
mod policy;
#[path = "../operations/resources.rs"]
mod resources;
#[path = "../operations/scoped.rs"]
mod scoped;
#[path = "../operations/shutdown.rs"]
mod shutdown;
#[allow(dead_code, unused_imports)]
#[path = "../support/operations_wire.rs"]
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
fn admitted_operations() {
    admission::contract_scenario();
    lanes::contract_scenario();
    middleware::contract_scenario();
    policy::contract_scenario();
    resources::contract_scenario();
}

#[test]
fn nested_operations() {
    causation::contract_scenario();
    children::contract_scenario();
    clock::contract_scenario();
    scoped::contract_scenario();
    outcomes::contract_scenario();
}

#[test]
fn operation_settlement() {
    execution::contract_scenario();
    finalization::contract_scenario();
    shutdown::contract_scenario();
}

#[test]
fn host() {
    host::contract_scenario();
}
