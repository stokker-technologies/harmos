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
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};

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
    let mut group = criterion.benchmark_group("storage/ingestion");
    for periods in [4, 64] {
        let samples = waveforms::samples(periods);
        let inputs = waveforms::inputs(&samples);
        group.throughput(Throughput::Elements(inputs.len() as u64));
        for concurrency in [1, 2, 4] {
            for durable in [false, true] {
                let name = if durable { "durable" } else { "accepted" };
                group.bench_with_input(
                    BenchmarkId::new(format!("{name}/{concurrency}"), inputs.len()),
                    &inputs,
                    |bencher, inputs| {
                        bencher.iter_custom(|iterations| {
                            executor.block_on(async {
                                let mut elapsed = Duration::ZERO;
                                let mut resources = super::metrics::Observed::default();
                                let mut configuration = None;
                                for _ in 0..iterations {
                                    let fixture = Fixture::new(concurrency).await;
                                    configuration = Some(fixture.settings.clone());
                                    let batch: Vec<_> = inputs
                                        .iter()
                                        .map(harmos::storage::StreamInput::borrowed)
                                        .collect();
                                    let probe =
                                        super::metrics::Probe::start(fixture.storage.clone());
                                    let started = Instant::now();
                                    let accepted =
                                        fixture.storage.write_stream(&batch).await.unwrap();
                                    if durable {
                                        fixture.storage.flush_until(&accepted).await.unwrap();
                                    }
                                    elapsed += started.elapsed();
                                    fixture.storage.flush_until(&accepted).await.unwrap();
                                    resources.merge(probe.finish());
                                    assert_eq!(
                                        fixture.data.rows.lock().unwrap().len(),
                                        inputs.len()
                                    );
                                    assert_eq!(fixture.storage.status().pending_bytes, 0);
                                    fixture.finish().await;
                                }
                                resources.report(
                                    &format!("ingestion/{name}/accepted-through-durable"),
                                    inputs.len(),
                                    concurrency,
                                    configuration.as_ref(),
                                );
                                elapsed
                            })
                        });
                    },
                );
            }
        }
    }
    group.finish();
}
/* === Private Functions === */
/* === Tests === */
