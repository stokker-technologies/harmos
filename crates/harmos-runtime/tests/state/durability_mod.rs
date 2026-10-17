/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/durability.rs"]
mod support;

#[path = "durability_atomic_commit.rs"]
mod atomic_commit;

#[path = "durability_delivery.rs"]
mod delivery;

#[path = "durability_evidence.rs"]
mod evidence;

#[path = "durability_evolution.rs"]
mod evolution;

#[path = "durability_history.rs"]
mod history;

#[path = "durability_recovery.rs"]
mod recovery;

#[path = "durability_retention.rs"]
mod retention;

#[path = "durability_wire.rs"]
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
fn durable_commit() {
    atomic_commit::contract_scenario();
    delivery::contract_scenario();
    evidence::contract_scenario();
}

#[test]
fn recovery_history() {
    recovery::contract_scenario();
    history::contract_scenario();
    retention::contract_scenario();
}

#[test]
fn schema_evolution() {
    evolution::contract_scenario();
    wire::contract_scenario();
}
