/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Startup And Commands In The Real Schematic Wasm Component.

/* === Modules === */

#[path = "../sidecar/fixture.rs"]
mod fixture;

#[path = "../sidecar/transfers.rs"]
mod transfers;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};

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
    let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let mut startup = criterion.benchmark_group("guest/startup");
    startup.sample_size(10);
    startup.bench_function("schematic", |bencher| {
        bencher.iter_custom(|iterations| {
            executor.block_on(async {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let fixture = fixture::Fixture::guest().await;
                    elapsed += fixture.startup_elapsed;
                    let answer = fixture.command().await;
                    fixture::verify_route(&answer);
                    fixture.finish().await;
                }
                elapsed
            })
        });
    });
    startup.finish();

    let fixture = executor.block_on(fixture::Fixture::guest());
    executor.block_on(fixture.verify_filesystem());
    for _ in 0..8 {
        let answer = executor.block_on(fixture.command());
        fixture::verify_route(&answer);
    }
    let mut commands = criterion.benchmark_group("guest/commands");
    commands.sample_size(10);
    commands.throughput(Throughput::Elements(1));
    commands.bench_function("route", |bencher| bencher.iter(|| executor.block_on(fixture.command())));
    for symbols in [12, 409, 6553] {
        let request = fixture::payload(symbols);
        let answer = executor.block_on(fixture.payload(request.clone()));
        assert!(answer.diagnostics.is_empty());
        commands.throughput(Throughput::Elements(symbols as u64));
        commands.bench_with_input(BenchmarkId::new("validate", symbols), &request, |bencher, request| {
            bencher.iter(|| executor.block_on(fixture.payload(request.clone())));
        });
    }
    commands.finish();
    transfers::run(&executor, "guest", &fixture);
    executor.block_on(fixture.finish());
}

/* === Tests === */
