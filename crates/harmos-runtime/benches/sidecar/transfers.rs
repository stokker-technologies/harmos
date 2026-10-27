/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Symmetric Typed Transfers Across Real Native And Wasm Carrier Boundaries.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, SamplingMode, Throughput};

// Local Imports
use super::fixture::Fixture;
use harmos_runtime::operations::StreamSettings;
use schematic_operations::transfer::{
    Batch, Download, DownloadRequest, PutRequest, Receipt, Record, Sample, TakeRequest, Upload, UploadRequest,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(executor: &tokio::runtime::Runtime, carrier: &str, fixture: &Fixture) {
    std::thread::scope(|threads| {
        for (transfer, (kind, count)) in [("records", 10_000), ("file", 30 * 1024 * 1024), ("samples", 1_000_000)].into_iter().enumerate() {
            threads.spawn(move || {
                let mut criterion = Criterion::default().configure_from_args();
                dataset_bench(&mut criterion, executor, carrier, fixture, kind, count, transfer as u64);
            });
        }
    });
}

/* === Private Functions === */

fn dataset_bench(
    criterion: &mut Criterion, executor: &tokio::runtime::Runtime, carrier: &str, fixture: &Fixture, kind: &str, count: usize,
    transfer: u64,
) {
    let mut group = criterion.benchmark_group(format!("{carrier}/transfers/{kind}"));
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.throughput(if kind == "file" { Throughput::Bytes(count as u64) } else { Throughput::Elements(count as u64) });
    for mode in ["unary", "streamed"] {
        for direction in ["to-plugin", "from-plugin"] {
            group.bench_function(BenchmarkId::new(format!("{mode}/{direction}"), count), |bencher| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            let batches = dataset(kind, count);
                            let batch_count = batches.len();
                            let started = Instant::now();
                            let receipt = upload(fixture, batches, mode, transfer).await;
                            let uploaded = started.elapsed();
                            verify_receipt(&receipt, kind, count);

                            let started = Instant::now();
                            let (batches, receipt) = download(fixture, mode, batch_count, transfer).await;
                            let downloaded = started.elapsed();
                            verify_receipt(&receipt, kind, count);
                            verify(batches, kind, count);
                            elapsed += if direction == "to-plugin" { uploaded } else { downloaded };
                        }
                        elapsed
                    })
                });
            });
        }
    }
    group.finish();
}

async fn upload(fixture: &Fixture, batches: Vec<Batch>, mode: &str, transfer: u64) -> Receipt {
    if mode == "unary" {
        let mut total = Receipt { records: 0, samples: 0, bytes: 0 };
        for (index, batch) in batches.into_iter().enumerate() {
            let receipt = fixture.runtime.operation(PutRequest { transfer, first: index == 0, batch }).run().await.unwrap();
            total.records += receipt.records;
            total.samples += receipt.samples;
            total.bytes += receipt.bytes;
        }
        return total;
    }
    let mut session = fixture.runtime.operations.upload::<Upload>(UploadRequest { transfer }, settings()).unwrap();
    for batch in batches {
        session.send(batch).await.unwrap();
    }
    session.finish_input();
    session.join().await.unwrap().into_owned()
}

async fn download(fixture: &Fixture, mode: &str, count: usize, transfer: u64) -> (Vec<Batch>, Receipt) {
    if mode == "unary" {
        let mut batches = Vec::with_capacity(count);
        let mut total = Receipt { records: 0, samples: 0, bytes: 0 };
        for _ in 0..count {
            let batch = fixture.runtime.operation(TakeRequest { transfer }).run().await.unwrap().into_owned();
            total.records += batch.records.len() as u64;
            total.samples += batch.samples.len() as u64;
            total.bytes += batch.bytes.len() as u64;
            batches.push(batch);
        }
        return (batches, total);
    }
    let mut session = fixture.runtime.operations.download::<Download>(DownloadRequest { transfer }, settings()).unwrap();
    let mut batches = Vec::new();
    while let Some(item) = session.next().await.unwrap() {
        let (batch, _claims) = item.into_owned_request().unwrap();
        batches.push(batch);
    }
    let receipt = session.join().await.unwrap().into_owned();
    (batches, receipt)
}

fn settings() -> StreamSettings {
    StreamSettings { items: NonZeroUsize::new(4).unwrap(), bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap() }
}

fn dataset(kind: &str, count: usize) -> Vec<Batch> {
    let size = match kind {
        "records" => 256,
        "file" => 1024 * 1024,
        _ => 4096,
    };
    (0..count)
        .step_by(size)
        .map(|start| {
            let end = count.min(start + size);
            let mut batch = Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() };
            match kind {
                "records" => {
                    batch.records = (start..end).map(|sequence| Record { sequence: sequence as u64, payload: vec![0x5a; 256] }).collect()
                }
                "file" => batch.bytes = (start..end).map(|index| (index % 251) as u8).collect(),
                _ => {
                    batch.samples = (start..end)
                        .map(|sequence| Sample { sequence: sequence as u64, timestamp: sequence as u64, value: sequence as f64 * 0.000001 })
                        .collect()
                }
            }
            batch
        })
        .collect()
}

fn verify_receipt(receipt: &Receipt, kind: &str, count: usize) {
    let expected = match kind {
        "records" => (count as u64, 0, 0),
        "file" => (0, 0, count as u64),
        _ => (0, count as u64, 0),
    };
    assert_eq!((receipt.records, receipt.samples, receipt.bytes), expected);
}

fn verify(batches: Vec<Batch>, kind: &str, count: usize) {
    let mut checked = 0usize;
    for batch in batches {
        match kind {
            "records" => {
                assert!(batch.samples.is_empty() && batch.bytes.is_empty());
                for record in batch.records {
                    assert_eq!(record.sequence, checked as u64);
                    assert_eq!(record.payload, [0x5a; 256]);
                    checked += 1;
                }
            }
            "file" => {
                assert!(batch.records.is_empty() && batch.samples.is_empty());
                for byte in batch.bytes {
                    assert_eq!(byte, (checked % 251) as u8);
                    checked += 1;
                }
            }
            _ => {
                assert!(batch.records.is_empty() && batch.bytes.is_empty());
                for sample in batch.samples {
                    assert_eq!(sample.sequence, checked as u64);
                    assert_eq!(sample.timestamp, checked as u64);
                    assert_eq!(sample.value, checked as f64 * 0.000001);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, count);
}

/* === Tests === */
