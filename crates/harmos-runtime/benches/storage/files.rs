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
use harmos_runtime::Identifier;
use harmos_runtime::storage::operations::ReadFile;
use harmos_runtime::storage::{ByteRange, FileMetadata};

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
    let mut group = criterion.benchmark_group("storage/files");
    for length in [1024, 1024 * 1024] {
        let payload = waveforms::binary(length);
        group.throughput(Throughput::Bytes(length as u64));
        for action in ["publish", "read"] {
            group.bench_with_input(BenchmarkId::new(action, length), &payload, |bencher, payload| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        let mut resources = super::metrics::Observed::default();
                        let mut configuration = None;
                        for _ in 0..iterations {
                            let fixture = Fixture::new(4).await;
                            configuration = Some(fixture.settings.clone());
                            let metadata = FileMetadata {
                                id: Identifier::new("binary").unwrap(),
                                scope: None,
                                media_type: "application/octet-stream".into(),
                                expected_length: Some(payload.len() as u64),
                                expected_sha256: None,
                            };
                            let probe = super::metrics::Probe::start(fixture.storage.clone());
                            let started = Instant::now();
                            let reference = fixture.storage.write_file(metadata, payload).await.unwrap();
                            fixture.storage.flush_until(&reference.position).await.unwrap();
                            let publication = started.elapsed();
                            let started = Instant::now();
                            let mut reader = fixture
                                .storage
                                .read_file(ReadFile { file: reference, range: ByteRange { offset: 0, limit: None } })
                                .await
                                .unwrap();
                            let mut received = Vec::with_capacity(payload.len());
                            while let Some(batch) = reader.next().await.unwrap() {
                                received.extend_from_slice(&batch.bytes);
                            }
                            let read = started.elapsed();
                            resources.merge(probe.finish());
                            elapsed += if action == "publish" { publication } else { read };
                            assert_eq!(&received, payload);
                            drop(reader);
                            fixture.finish().await;
                        }
                        resources.report(&format!("files/{action}/publication-and-read-lifecycle"), length, 4, configuration.as_ref());
                        elapsed
                    })
                });
            });
        }
    }
    group.finish();
}
/* === Private Functions === */
/* === Tests === */
