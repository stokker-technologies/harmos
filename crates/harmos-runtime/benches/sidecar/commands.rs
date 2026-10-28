/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Host Commands Through A Real Schematic Sidecar Process.

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput};

use super::fixture::{self, Fixture};

pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let mut startup = criterion.benchmark_group("sidecar-host/startup");
    startup.sample_size(10);
    startup.bench_function("schematic", |bencher| {
        bencher.iter_custom(|iterations| {
            executor.block_on(async {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let fixture = Fixture::sidecar().await;
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

    let fixture = executor.block_on(Fixture::sidecar());
    for _ in 0..8 {
        let answer = executor.block_on(fixture.command());
        fixture::verify_route(&answer);
    }
    let mut commands = criterion.benchmark_group("sidecar-host/commands");
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
    executor.block_on(fixture.finish());
}
