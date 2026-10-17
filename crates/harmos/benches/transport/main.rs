/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../tests/support/native_streams.rs"]
mod native;

/* === Re-Exports === */

/* === Imports === */

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
        let mut consumers = c.benchmark_group("streams/native_consumer");
        consumers.sample_size(10);
        for carrier in ["executable"] {
            for size in [64 * 1024, 1024 * 1024] {
                let fixture = if carrier == "executable" {
                    let path =
                        std::path::Path::new(env!("CARGO_BIN_EXE_harmos-stream-consumer-fixture"));
                    runtime.block_on(native::Fixture::executable(path))
                } else {
                    runtime.block_on(native::Fixture::new())
                };
                let mut sequence = 0;
                consumers.throughput(criterion::Throughput::Bytes((8 * size) as u64));
                consumers.bench_with_input(
                    criterion::BenchmarkId::new(carrier, size),
                    &size,
                    |b, &size| {
                        b.iter(|| runtime.block_on(transfer(&fixture, &mut sequence, size)));
                    },
                );
                runtime.block_on(fixture.runtime.stop()).unwrap();
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
