/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Deterministic Double Run Oracle

/* === Modules === */

#[path = "../../tests/support/simulation.rs"]
mod oracle;
#[path = "../../tests/support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */

use criterion::Criterion;
use harmos_runtime::Runtime;
use std::hint::black_box;
use support::{Bank, Ledger, catalog, streams};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);
criterion::criterion_main!(benches);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn benchmarks(criterion: &mut Criterion) {
    criterion.bench_function("simulation/double_run_identity", |bench| {
        bench.iter(|| {
            let make = || {
                oracle::scripted(Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog()).register(streams()).entropy_seed(42))
            };
            let first = make();
            let second = make();
            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
            black_box((first, second));
        });
    });
}

/* === Tests === */
