/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod commands;
mod fixture;
#[path = "../../tests/support/native_streams.rs"]
mod native;
mod transfers;

/* === Re-Exports === */

/* === Imports === */

extern crate harmos_runtime as harmos;

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

fn benchmarks(c: &mut criterion::Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    commands::run(c, &runtime);
    let fixture = runtime.block_on(fixture::Fixture::sidecar());
    runtime.block_on(fixture.verify_filesystem());
    transfers::run(&runtime, "sidecar-host", &fixture);
    runtime.block_on(fixture.finish());
    let mut consumers = c.benchmark_group("streams/native_consumer");
    consumers.sample_size(10);
    for carrier in ["executable"] {
        for size in [64 * 1024 / 10, 1024 * 1024 / 10] {
            let fixture = if carrier == "executable" {
                let executable = std::env::current_exe().unwrap();
                let name = if cfg!(windows) { "harmos-stream-consumer-fixture.exe" } else { "harmos-stream-consumer-fixture" };
                let path = &executable.parent().unwrap().parent().unwrap().join("bin").join(name);
                runtime.block_on(native::Fixture::executable(path))
            } else {
                runtime.block_on(native::Fixture::new())
            };
            let mut sequence = 0;
            consumers.throughput(criterion::Throughput::Bytes((8 * size) as u64));
            consumers.bench_with_input(criterion::BenchmarkId::new(carrier, size), &size, |b, &size| {
                b.iter(|| runtime.block_on(transfer(&fixture, &mut sequence, size)));
            });
            runtime.block_on(fixture.runtime.shutdown()).unwrap();
        }
    }
    consumers.finish();
}

#[cfg(feature = "plugin-sidecar")]
async fn transfer(fixture: &native::Fixture, sequence: &mut u64, size: usize) {
    for _ in 0..8 {
        fixture.runtime.streams.publish(native::Payload {
            sequence: *sequence,
            bytes: vec![(*sequence % 251) as u8; size],
        });
        *sequence += 1;
    }
    let acknowledged = fixture.acknowledged(*sequence).await;
    assert_eq!(acknowledged.rows, *sequence);
    assert_eq!(acknowledged.bytes, *sequence * size as u64);
}

/* === Tests === */
