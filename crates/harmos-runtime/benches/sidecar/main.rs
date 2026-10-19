/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../tests/support/native_publications.rs"]
mod native;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};

// Local Imports

/* === Constants === */

const PROFILE_CALLS: u32 = 2_000;

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

// One sidecar `host.publish` child call end to end: kit encode, transport
// frame, host admission, managed operation dispatch, staging and the answer.
fn benchmarks(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let fixture = runtime.block_on(native::Fixture::new());
    // Warm The Pools And Caches Before Any Measurement
    runtime.block_on(fixture.publish(PROFILE_CALLS, 1));
    profile(&runtime, &fixture);

    let mut group = c.benchmark_group("publication/native_child_call");
    group.sample_size(20);
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_row", |b| {
        b.iter_custom(|iterations| {
            let rows = u32::try_from(iterations).expect("bounded iteration count");
            runtime.block_on(fixture.publish(rows, 1)).into_iter().sum()
        });
    });
    for size in [10_u32, 100] {
        group.throughput(Throughput::Elements(u64::from(size)));
        group.bench_with_input(BenchmarkId::new("serial_rows", size), &size, |b, &size| {
            b.iter_custom(|iterations| {
                let rows = u32::try_from(iterations).expect("bounded iteration count") * size;
                runtime.block_on(fixture.publish(rows, 1)).into_iter().sum()
            });
        });
        group.bench_with_input(BenchmarkId::new("batch_rows", size), &size, |b, &size| {
            b.iter_custom(|iterations| {
                let rows = u32::try_from(iterations).expect("bounded iteration count") * size;
                runtime.block_on(fixture.publish(rows, size)).into_iter().sum()
            });
        });
    }
    group.finish();
    runtime.block_on(fixture.runtime.stop()).unwrap();
}

fn profile(runtime: &tokio::runtime::Runtime, fixture: &native::Fixture) {
    for (rows, batch) in [(PROFILE_CALLS, 1), (PROFILE_CALLS * 10, 100)] {
        let mut calls = runtime.block_on(fixture.publish(rows, batch));
        calls.sort_unstable();
        eprintln!(
            "PUBLICATION ({batch} row(s) per call): p50 {:?}, p95 {:?}, p99 {:?}, worst {:?} over {} calls",
            percentile(&calls, 50),
            percentile(&calls, 95),
            percentile(&calls, 99),
            calls.last().copied().unwrap_or_default(),
            calls.len(),
        );
    }
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    samples[(samples.len() * percentile).div_ceil(100).saturating_sub(1)]
}

/* === Tests === */
