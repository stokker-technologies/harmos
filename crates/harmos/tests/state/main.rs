/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod managed;
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;
#[path = "../support/mod.rs"]
mod support;

#[path = "capture.rs"]
mod capture;

#[path = "consumers.rs"]
mod consumers;

#[path = "idempotency.rs"]
mod idempotency;

#[path = "ordering.rs"]
mod ordering;

#[path = "poison.rs"]
mod poison;

#[path = "records.rs"]
mod records;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
mod listeners;

#[path = "undo.rs"]
mod undo;

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
fn committed_history() {
    ordering::contract_scenario();
    idempotency::contract_scenario();
    records::contract_scenario();
    consumers::contract_scenario();
}

#[test]
fn state_ownership() {
    managed::contract_scenario();
    poison::contract_scenario();
}
