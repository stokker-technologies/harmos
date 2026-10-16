/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../support/storage.rs"]
mod backend;
#[path = "../../support/storage_fixture.rs"]
mod fixture;
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;
#[path = "../../support/mod.rs"]
mod support;
use fixture::*;
#[path = "boot.rs"]
mod boot;
#[path = "capture.rs"]
mod capture;
#[path = "composition.rs"]
mod composition;
mod files;
#[path = "managed.rs"]
mod managed;
#[allow(dead_code, unused_imports)]
#[path = "../../support/storage_operations.rs"]
mod operations;
#[path = "reconcile.rs"]
mod reconcile;
#[path = "snapshots.rs"]
mod snapshots;

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
fn journal_recovery() {
    boot::contract_scenario();
    composition::contract_scenario();
    managed::contract_scenario();
    reconcile::contract_scenario();
}

#[test]
fn snapshot_recovery() {
    snapshots::contract_scenario();
}
