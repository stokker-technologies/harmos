/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../support/allocation.rs"]
pub mod allocation;

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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("publish_allocation_independent", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    allocation::streams_r004_publish_cost_is_independent_of_the_window_size().await;
                });
        });
    }
}

/* === Tests === */
