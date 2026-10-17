/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[allow(dead_code, reason = "the codec bench uses only the shared payload declarations")]
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
        use harmos_runtime::Message;
        let mut codec = c.benchmark_group("streams/native_codec");
        codec.sample_size(20);
        for size in [64 * 1024, 1024 * 1024] {
            let row = native::Payload {
                sequence: 1,
                bytes: (0..size).map(|index| (index % 251) as u8).collect(),
            };
            let payload = row.encode_to_vec();
            let command = harmos_sdk::operations::WireCommand::Prepare {
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
                        harmos_sdk::operations::WireCommand::decode(std::hint::black_box(
                            envelope,
                        ))
                        .unwrap()
                    });
                },
            );
        }
        codec.finish();
}

/* === Tests === */
