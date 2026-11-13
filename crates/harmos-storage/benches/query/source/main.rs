/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Benchmark Columnar and Reference Payload Decoding

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;

// Third Party Imports
extern crate harmos_domain as harmos;
use criterion::{BenchmarkId, Criterion, Throughput};

// Local Imports
use harmos_storage::{Message, query::PayloadDecoder};

/* === Constants === */

const CHUNK_ROWS: usize = 16_384;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    x: f64,
    y: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let decoder = PayloadDecoder::new::<Sample>();
    let mut group = criterion.benchmark_group("unit/payload_decode");
    for rows in [40_000_u64, 1_000_000] {
        let payloads = (0..rows)
            .map(|row| {
                Sample {
                    x: row as f64,
                    y: (row * 7919 % 65521) as f64,
                }
                .encode()
            })
            .collect::<Vec<_>>();
        let chunks = payloads.chunks(CHUNK_ROWS).map(|chunk| chunk.iter().map(Vec::as_slice).collect::<Vec<_>>()).collect::<Vec<_>>();
        group.throughput(Throughput::Elements(rows));
        group.bench_with_input(BenchmarkId::new("columnar", rows), &chunks, |b, chunks| {
            b.iter(|| {
                for chunk in chunks {
                    black_box(decoder.decode(chunk).unwrap());
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("reference", rows), &chunks, |b, chunks| {
            b.iter(|| {
                for chunk in chunks {
                    black_box(decoder.reference(chunk).unwrap());
                }
            });
        });
    }
    group.finish();
    criterion.final_summary();
}

/* === Tests === */
