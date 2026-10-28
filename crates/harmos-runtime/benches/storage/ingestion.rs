/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::time::{Duration, Instant, UNIX_EPOCH};

use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_runtime::storage::query::{RecordSelection, StreamSelection};
use harmos_runtime::storage::*;
use harmos_runtime::{Identifier, MemorySize, Message};

use super::{fixture::Fixture, waveforms};

const RECORDS: usize = 10_000;
const SAMPLES: usize = 1_000_000;
const BATCH_ROWS: usize = 4096;
const BATCH_BYTES: usize = 1024 * 1024;

#[harmos_runtime::message]
struct Record {
    sequence: u64,
    payload: Vec<u8>,
}

pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let mut group = criterion.benchmark_group("storage/ingestion");
    group.sample_size(10);
    group.sampling_mode(criterion::SamplingMode::Flat);
    for (kind, count) in [("records", RECORDS), ("samples", SAMPLES)] {
        group.throughput(Throughput::Elements(count as u64));
        for durable in [false, true] {
            let name = if durable { "durable" } else { "accepted" };
            group.bench_function(BenchmarkId::new(format!("{kind}/{name}"), count), |bencher| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        let mut resources = super::metrics::Observed::default();
                        let mut configuration = None;
                        for _ in 0..iterations {
                            let fixture = Fixture::new().await;
                            configuration = Some(fixture.settings.clone());
                            let schema = if kind == "samples" { waveforms::schema() } else { record_schema() };
                            let definition = if kind == "samples" { waveforms::definition() } else { record_definition() };
                            fixture.storage.write_schema(schema.clone(), definition).await.unwrap();
                            fixture.storage.flush().await.unwrap();

                            // Include Bounded Generation And Admission; Durable Also Includes The Final Barrier.
                            let probe = super::metrics::Probe::start(fixture.storage.clone());
                            let started = Instant::now();
                            let accepted = ingest(&fixture.storage, kind, &schema, count).await;
                            if durable {
                                fixture.storage.flush_until(&accepted).await.unwrap();
                            }
                            elapsed += started.elapsed();
                            fixture.storage.flush_until(&accepted).await.unwrap();
                            resources.merge(probe.finish());

                            // Verify Through The Public Paged Reader Outside Write Timing.
                            read(&fixture, kind, &schema, count).await;
                            assert_eq!(fixture.storage.status().pending_bytes, 0);
                            fixture.finish().await;
                        }
                        resources.report(&format!("ingestion/{kind}/{name}"), count, 1, configuration.as_ref());
                        elapsed
                    })
                });
            });
        }

        group.bench_function(BenchmarkId::new(format!("{kind}/read"), count), |bencher| {
            // Populate Once Before Any Read Samples Or Their Memory Probe.
            let fixture = executor.block_on(Fixture::new());
            let schema = if kind == "samples" { waveforms::schema() } else { record_schema() };
            let definition = if kind == "samples" { waveforms::definition() } else { record_definition() };
            executor.block_on(async {
                fixture.storage.write_schema(schema.clone(), definition).await.unwrap();
                let accepted = ingest(&fixture.storage, kind, &schema, count).await;
                fixture.storage.flush_until(&accepted).await.unwrap();
            });

            bencher.iter_custom(|iterations| {
                executor.block_on(async {
                    let mut elapsed = Duration::ZERO;
                    let mut resources = super::metrics::Observed::default();
                    for _ in 0..iterations {
                        let probe = super::metrics::Probe::start(fixture.storage.clone());
                        elapsed += read(&fixture, kind, &schema, count).await;
                        resources.merge(probe.finish());
                    }
                    resources.report(&format!("ingestion/{kind}/read"), count, 1, Some(&fixture.settings));
                    elapsed
                })
            });
            executor.block_on(fixture.finish());
        });
    }
    group.finish();
}

async fn ingest(storage: &StorageManager, kind: &str, schema: &SchemaKey, count: usize) -> StorageCursor {
    let mut after = 0;
    let mut accepted = storage.cursor_saved();
    while after < count {
        let batch = records(kind, schema, after, count);
        let start = after;
        after += batch.len();
        if kind == "samples" {
            let batch = batch
                .into_iter()
                .enumerate()
                .map(|(index, record)| StreamInput {
                    sequence: (start + index) as u64,
                    record,
                    stream: Identifier::new("samples").unwrap(),
                })
                .collect();
            accepted = storage.write_stream(batch).await.unwrap();
        } else {
            accepted = storage.write_records(batch).await.unwrap();
        }
    }
    accepted
}

fn records(kind: &str, schema: &SchemaKey, after: usize, count: usize) -> Vec<RecordInput> {
    let mut batch = Vec::new();
    let mut bytes = 0;
    for sequence in after..count.min(after + BATCH_ROWS) {
        let payload = if kind == "samples" {
            let sample = waveforms::Sample {
                phase: 0,
                ordinal: sequence as u64,
                value: sequence as f64 * 0.000001,
            };
            sample.encode()
        } else {
            let record = Record {
                sequence: sequence as u64,
                payload: vec![0x5a; 256],
            };
            record.encode()
        };
        let input = RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("benchmark").unwrap(),
            source: None,
            source_time: Some(UNIX_EPOCH + Duration::from_micros(sequence as u64)),
            payload,
        };
        let size = input.memory_size() + std::mem::size_of::<StreamInput>();
        if bytes + size > BATCH_BYTES {
            break;
        }
        bytes += size;
        batch.push(input);
    }
    assert!(!batch.is_empty());
    batch
}

fn record_schema() -> SchemaKey {
    SchemaKey {
        id: Identifier::new("bench.record").unwrap(),
        version: std::num::NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    }
}

fn record_definition() -> harmos_runtime::schema::graph::Schema {
    harmos_runtime::schema::graph::Schema::from_declaration(
        <Record as harmos_runtime::schema::Described>::DEFINITION,
        &harmos_runtime::config::SchemaSettings::default(),
    )
    .unwrap()
}

async fn read(fixture: &Fixture, kind: &str, schema: &SchemaKey, count: usize) -> Duration {
    let limits = ReadLimits {
        rows: super::nonzero(BATCH_ROWS),
        bytes: super::nonzero(BATCH_BYTES),
    };
    let mut after = 0;
    let mut checked = 0usize;
    let mut elapsed = Duration::ZERO;
    if kind == "samples" {
        let selection = StreamSelection {
            schema: schema.clone(),
            scope: None,
            item: None,
            stream: None,
            source_start: None,
            source_end: None,
            after: 0,
        };
        loop {
            let started = Instant::now();
            let page = fixture.storage.page::<StoredRow>(&selection, after, limits).await.unwrap();
            elapsed += started.elapsed();
            let rows = page.entries();
            if rows.is_empty() {
                break;
            }
            assert!(rows.len() <= BATCH_ROWS);
            for row in rows {
                assert!(row.record.position.sequence > after);
                after = row.record.position.sequence;
                let sample = waveforms::Sample::decode(&row.record.payload).unwrap();
                assert_eq!(sample.ordinal, checked as u64);
                assert_eq!(row.sequence, checked as u64);
                assert_eq!(sample.phase, 0);
                assert_eq!(sample.value, checked as f64 * 0.000001);
                assert_eq!(row.record.source_time, Some(UNIX_EPOCH + Duration::from_micros(checked as u64)));
                checked += 1;
            }
        }
    } else {
        let selection = RecordSelection {
            schema: schema.clone(),
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        };
        loop {
            let started = Instant::now();
            let page = fixture.storage.page::<StoredRecord>(&selection, after, limits).await.unwrap();
            elapsed += started.elapsed();
            let rows = page.entries();
            if rows.is_empty() {
                break;
            }
            assert!(rows.len() <= BATCH_ROWS);
            for row in rows {
                assert!(row.position.sequence > after);
                after = row.position.sequence;
                let record = Record::decode(&row.payload).unwrap();
                assert_eq!(record.sequence, checked as u64);
                assert_eq!(record.payload, [0x5a; 256]);
                assert_eq!(row.source_time, Some(UNIX_EPOCH + Duration::from_micros(checked as u64)));
                checked += 1;
            }
        }
    }
    assert_eq!(checked, count);
    elapsed
}
