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

// Standard Library Imports
use std::time::{Duration, Instant, UNIX_EPOCH};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_runtime::storage::Query;
use harmos_runtime::storage::query::transform::{
    Transform,
    project::{Project, Projection},
};
use harmos_runtime::storage::query::{QueryArrow, RowSlice, Source, StreamSelection};

// Local Imports
use super::{fixture::Fixture, waveforms};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let mut group = criterion.benchmark_group("storage/query");
    group.sample_size(10);
    group.sampling_mode(criterion::SamplingMode::Flat);
    for periods in [1, 6] {
        let samples = waveforms::samples(periods);
        let inputs = waveforms::inputs(&samples);
        for concurrency in [1, 2, 4] {
            let fixture = executor.block_on(Fixture::new());
            executor.block_on(async {
                fixture.storage.write_schema(waveforms::schema(), waveforms::definition()).await.unwrap();
                for inputs in inputs.chunks(192) {
                    let batch = inputs.to_vec();
                    fixture.storage.write_stream(batch).await.unwrap();
                }
                fixture.storage.flush().await.unwrap();
            });
            group.throughput(Throughput::Elements(((samples.len() / 2 - 3) * concurrency) as u64));
            for metric in ["all-first-batches", "completion"] {
                group.bench_function(BenchmarkId::new(format!("{metric}/{concurrency}"), samples.len()), |bencher| {
                    bencher.iter_custom(|iterations| {
                        executor.block_on(async {
                            let mut elapsed = Duration::ZERO;
                            let mut resources = super::metrics::Observed::default();
                            for _ in 0..iterations {
                                let query = Query {
                                    at: fixture.storage.cursor_saved(),
                                    source: Source::Stream(StreamSelection {
                                        schema: waveforms::schema(),
                                        scope: None,
                                        item: None,
                                        stream: None,
                                        source_start: Some(UNIX_EPOCH),
                                        source_end: Some(
                                            UNIX_EPOCH + Duration::from_millis((periods * waveforms::SAMPLES_PER_PERIOD / 2) as u64),
                                        ),
                                        after: 0,
                                    }),
                                    transforms: vec![Transform::Project(Project {
                                        fields: vec![Projection {
                                            field: "payload.value".into(),
                                            name: "value".into(),
                                        }],
                                    })],
                                    slice: RowSlice { offset: 3, limit: None },
                                };
                                let probe = super::metrics::Probe::start(fixture.storage.clone());
                                let openings = (0..concurrency).map(|_| async {
                                    let mut reader = fixture.storage.query(query.clone()).await.unwrap();
                                    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
                                    let Some(QueryArrow::Batch(first)) = reader.next().await.unwrap() else {
                                        panic!("nonempty query");
                                    };
                                    (reader, first)
                                });

                                let started = Instant::now();
                                let readers = futures_util::future::join_all(openings).await;
                                let first_elapsed = started.elapsed();
                                let completions = readers.into_iter().map(|(mut reader, first)| async move {
                                    let mut rows = first.as_record_batch().num_rows();
                                    drop(first);
                                    while let Some(output) = reader.next().await.unwrap() {
                                        if let QueryArrow::Batch(batch) = output {
                                            rows += batch.as_record_batch().num_rows();
                                        }
                                    }
                                    (rows, reader.completion().unwrap().rows)
                                });
                                let completions = futures_util::future::join_all(completions).await;
                                let completion = started.elapsed();
                                resources.merge(probe.finish());
                                elapsed += if metric == "all-first-batches" { first_elapsed } else { completion };
                                for (rows, completed) in completions {
                                    assert_eq!(rows, samples.len() / 2 - 3);
                                    assert_eq!(completed, rows as u64);
                                }
                            }
                            resources.report(&format!("query/{metric}"), samples.len(), concurrency, Some(&fixture.settings));
                            elapsed
                        })
                    });
                });
            }
            executor.block_on(fixture.finish());
        }
    }
    group.finish();
}
/* === Private Functions === */

/* === Tests === */
