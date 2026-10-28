/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Deterministic Double Run Oracle

#[path = "../../tests/support/simulation.rs"]
mod oracle;
#[path = "../../tests/support/mod.rs"]
mod support;

extern crate harmos_runtime as harmos;

use criterion::Criterion;
use harmos_runtime::Runtime;
use std::hint::black_box;
use support::{Bank, Ledger};

criterion::criterion_group!(benches, benchmarks);
criterion::criterion_main!(benches);

fn benchmarks(criterion: &mut Criterion) {
    criterion.bench_function("simulation/double_run_identity", |bench| {
        bench.iter(|| {
            let make = || {
                let sim = Runtime::<Bank>::simulate_with(Ledger::default(), (), 42, |setup| {
                    setup.settings().register(support::catalog()).register(support::streams());
                    oracle::configure(setup);
                })
                .unwrap();
                oracle::scripted(sim)
            };
            let first = make();
            let second = make();
            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
            black_box((first, second));
        });
    });
}
