/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Durable Native Path Against The Locked Multi-Day Workload.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_storage::bench_support::{
    self,
    workload::{ROWS, Workload, Sample},
};
use harmos_storage::memory::Backend;
use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn run(runtime: &tokio::runtime::Runtime) {
    let workload = Workload::new(ROWS);
    let catalog = harmos_storage::ObjectCatalog::new().entry(harmos_storage::ObjectDefinition::of::<Sample>());
    let owner = Arc::new(Backend::with_catalog(catalog, workload.history.clone()));
    bench_support::run_with_reopen(runtime, &workload, || owner.clone());
}

/* === Private Functions === */

/* === Tests === */
