/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
/* === Modules === */
/* === Re-Exports === */
/* === Imports === */
use super::{fixture::Fixture, waveforms};
use criterion::{BenchmarkId, Criterion};
use harmos::storage::query::{Query, QueryOutput, RowSlice, Source, StreamSelection};
use harmos::storage::*;
use std::time::{Duration, Instant};
/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
/* === Enums === */
/* === Traits === */
/* === Implementations === */
/* === Public Functions === */
pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let mut group = criterion.benchmark_group("storage/contention");
    let samples = waveforms::samples(16);
    let inputs = waveforms::inputs(&samples);
    for concurrency in [1, 2, 4] {
        group.bench_function(
            BenchmarkId::new("mixed-slow-consumer", concurrency),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        let mut resources = super::metrics::Observed::default();
                        let mut configuration = None;
                        for _ in 0..iterations {
                            let fixture = Fixture::new(concurrency).await;
                            fixture
                                .storage
                                .write_schema(&waveforms::schema(), &waveforms::definition())
                                .await
                                .unwrap();
                            for inputs in inputs.chunks(192) {
                                let batch: Vec<_> =
                                    inputs.iter().map(StreamInput::borrowed).collect();
                                fixture.storage.write_stream(&batch).await.unwrap();
                            }
                            configuration = Some(fixture.settings.clone());
                            let at = fixture.storage.flush().await.unwrap();
                            let readers = (0..concurrency).map(|_| async {
                                let query = Query {
                                    at: at.clone(),
                                    source: Source::Stream(StreamSelection {
                                        schema: waveforms::schema(),
                                        scope: None,
                                        item: None,
                                        stream: None,
                                        source_start: None,
                                        source_end: None,
                                        after: 0,
                                    }),
                                    transforms: vec![],
                                    slice: RowSlice {
                                        offset: 0,
                                        limit: None,
                                    },
                                };
                                let mut reader = fixture.storage.query(query).await.unwrap();
                                let mut rows = 0;
                                let mut peak = 0;
                                while let Some(output) = reader.next().await.unwrap() {
                                    let QueryOutput::Batch(page) = output else {
                                        continue;
                                    };
                                    rows += page.as_record_batch().num_rows();
                                    peak = peak.max(fixture.storage.status().read_bytes);
                                    // Retain Each Batch Across A Scheduling Boundary.
                                    tokio::task::yield_now().await;
                                    drop(page);
                                }
                                (rows, peak)
                            });
                            let writer = async {
                                for inputs in inputs.chunks(192) {
                                    let batch: Vec<_> =
                                        inputs.iter().map(StreamInput::borrowed).collect();
                                    fixture.storage.write_stream(&batch).await.unwrap();
                                }
                                fixture.storage.flush().await.unwrap()
                            };
                            let probe = super::metrics::Probe::start(fixture.storage.clone());
                            let started = Instant::now();
                            let (readers, written) =
                                tokio::join!(futures_util::future::join_all(readers), writer);
                            elapsed += started.elapsed();
                            resources.merge(probe.finish());
                            assert!(written.sequence > at.sequence);
                            for (rows, peak) in readers {
                                assert_eq!(rows, samples.len());
                                assert!(peak <= 128 * 1024 * 1024);
                            }
                            assert_eq!(fixture.storage.status().pending_bytes, 0);
                            fixture.finish().await;
                        }
                        resources.report(
                            "contention/mixed-slow-consumer",
                            samples.len(),
                            concurrency,
                            configuration.as_ref(),
                        );
                        elapsed
                    })
                })
            },
        );
    }
    group.finish();
}
/* === Private Functions === */
/* === Tests === */
