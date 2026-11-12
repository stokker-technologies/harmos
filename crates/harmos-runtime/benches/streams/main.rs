/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../tests/support/allocation.rs"]
pub mod allocation;
#[path = "../../tests/support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */

extern crate harmos_runtime as harmos;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn benchmarks(c: &mut criterion::Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let mut group = c.benchmark_group("streams");
    group.sample_size(10);
    group.bench_function("waveform", |b| b.iter(|| runtime.block_on(allocation::streams_ac006_concurrent_waveform_measurement())));
    group.finish();
}

fn main() {
    benches();
    criterion::Criterion::default().configure_from_args().final_summary();
}

/* === Tests === */
