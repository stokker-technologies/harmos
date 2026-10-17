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

#[path = "../../tests/support/stream_batches.rs"]
pub mod batches;

#[cfg(feature = "plugin-sidecar")]
#[path = "../../tests/support/native_streams.rs"]
mod native;

/* === Re-Exports === */

/* === Imports === */
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let mut group = c.benchmark_group("streams");
    group.sample_size(10);
    group.bench_function("waveform", |b| {
        b.iter(|| runtime.block_on(allocation::streams_ac006_concurrent_waveform_measurement()))
    });
    group.bench_function("summary", |b| {
        b.iter(|| runtime.block_on(batches::summary_cost()))
    });
    group.finish();
    #[cfg(feature = "plugin-sidecar")]
    {
        use harmos_runtime::Message;
        let mut codec = c.benchmark_group("streams/native_codec");
        codec.sample_size(20);
        for size in [64 * 1024, 1024 * 1024] {
            let row = native::Payload {
                sequence: 1,
                bytes: (0..size).map(|index| (index % 251) as u8).collect(),
            };
            let payload = row.encode_to_vec();
            let command = harmos_domain::operations::WireCommand::Prepare {
                invocation: u64::MAX,
                operation: "consumption.consume".to_owned(),
                payload: payload.clone(),
            };
            let envelope = command.encode().unwrap();
            eprintln!(
                "NATIVE CODEC: {size} useful bytes, {} Postcard payload bytes, {} binary envelope bytes",
                payload.len(),
                envelope.len()
            );
            codec.throughput(criterion::Throughput::Bytes(size as u64));
            codec.bench_with_input(
                criterion::BenchmarkId::new("postcard_encode", size),
                &row,
                |b, row| {
                    b.iter(|| std::hint::black_box(row).encode_to_vec());
                },
            );
            codec.bench_with_input(
                criterion::BenchmarkId::new("postcard_decode", size),
                &payload,
                |b, payload| {
                    b.iter(|| native::Payload::decode(std::hint::black_box(payload)).unwrap());
                },
            );
            codec.bench_with_input(
                criterion::BenchmarkId::new("envelope_encode", size),
                &command,
                |b, command| {
                    b.iter(|| std::hint::black_box(command).encode().unwrap());
                },
            );
            codec.bench_with_input(
                criterion::BenchmarkId::new("envelope_decode", size),
                &envelope,
                |b, envelope| {
                    b.iter(|| {
                        harmos_domain::operations::WireCommand::decode(std::hint::black_box(
                            envelope,
                        ))
                        .unwrap()
                    });
                },
            );
        }
        codec.finish();
        let mut consumers = c.benchmark_group("streams/native_consumer");
        consumers.sample_size(10);
        for carrier in ["in_process", "executable"] {
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
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--native-seconds")
    {
        let seconds: u64 = arguments
            .get(index + 1)
            .expect("--native-seconds requires 1 through 60")
            .parse()
            .expect("duration is an integer");
        assert!(
            (1..=60).contains(&seconds),
            "duration must be from 1 through 60 seconds"
        );
        #[cfg(feature = "plugin-sidecar")]
        {
            let executable = arguments
                .iter()
                .position(|argument| argument == "--native-executable")
                .map(|index| {
                    std::path::Path::new(
                        arguments
                            .get(index + 1)
                            .expect("--native-executable requires a prebuilt consumer path"),
                    )
                });
            sustained(seconds, executable);
        }
        #[cfg(not(feature = "plugin-sidecar"))]
        panic!("native consumption requires sidecar and sidecar-host features");
    } else {
        benches();
        criterion::Criterion::default()
            .configure_from_args()
            .final_summary();
    }
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

#[cfg(feature = "plugin-sidecar")]
fn sustained(seconds: u64, executable: Option<&std::path::Path>) {
    use std::time::{Duration, Instant};
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    for size in [64 * 1024, 1024 * 1024] {
        let fixture = match executable {
            Some(path) => executor.block_on(native::Fixture::executable(path)),
            None => executor.block_on(native::Fixture::new()),
        };
        assert_eq!(executor.block_on(fixture.acknowledged(0)).bytes, 0);
        let mut sequence = 0;
        let mut maximum = Duration::ZERO;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(seconds) {
            let batch = Instant::now();
            executor.block_on(transfer(&fixture, &mut sequence, size));
            maximum = maximum.max(batch.elapsed());
        }
        let elapsed = started.elapsed();
        let bytes = sequence * size as u64;
        let stopping = Instant::now();
        executor.block_on(fixture.runtime.stop()).unwrap();
        println!(
            "{}",
            serde_json::json!({
                "carrier": if executable.is_some() {
                    "binary over child stdio with independent native control"
                } else { "binary over independent in-process data/control duplex" },
                "host_workers": 4, "plugin_workers": 4,
                "separate_process": executable.is_some(),
                "payload_bytes": size, "rows_per_batch": 8, "stream_window": 16,
                "rows_consumed": sequence, "bytes_consumed": bytes,
                "elapsed_s": elapsed.as_secs_f64(),
                "useful_mib_s": bytes as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64(),
                "maximum_batch_ms": maximum.as_secs_f64() * 1000.0,
                "settlement_ms": stopping.elapsed().as_secs_f64() * 1000.0,
                "optimized": !cfg!(debug_assertions),
            })
        );
    }
}

/* === Tests === */
