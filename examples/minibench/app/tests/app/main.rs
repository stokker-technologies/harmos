/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![cfg(feature = "sim")]
/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use minibench_app::app::{self, Simulated};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn scripted(seed: u64) -> Simulated {
    let mut bench = app::simulate((), seed).expect("the bench declaration is simulatable");

    bench.stop().unwrap();
    bench.reboot().expect("the in-memory history reboots");
    bench
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("bench_declaration_simulates", || {
            let first = scripted(7);
            let second = scripted(7);

            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
            first.assert_replay_equivalent();
        });
    }
}
