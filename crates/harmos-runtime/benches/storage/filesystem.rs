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
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_operations::storage::{CaptureFileRequest, FollowFileRequest, HashFileRequest, ReadFileRequest};
use harmos_runtime::Identifier;
use harmos_runtime::storage::{ByteRange, CaptureConsistency, FileMetadata, FileSource, FollowStart};
use sha2::{Digest, Sha256};
// Local Imports
use super::fixture::Fixture;

/* === Constants === */

const FILE_BYTES: usize = 30 * 1024 * 1024;
const CHUNK_BYTES: usize = 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.bin");
    let expected = source(&path);
    let mut group = criterion.benchmark_group("storage/filesystem");
    group.sample_size(10);
    group.sampling_mode(criterion::SamplingMode::Flat);
    group.throughput(Throughput::Bytes(FILE_BYTES as u64));
    for location in ["local", "stored"] {
        for action in ["read", "hash", "capture", "follow"] {
            group.bench_function(BenchmarkId::new(format!("{location}/{action}"), FILE_BYTES), |bencher| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            let fixture = Fixture::new().await;
                            let input = if location == "stored" {
                                let reference = fixture
                                    .storage
                                    .capture_file(CaptureFileRequest {
                                        source: FileSource::Local(path.clone()),
                                        range: range(),
                                        metadata: metadata("source", expected),
                                        consistency: CaptureConsistency::Stable,
                                    })
                                    .await
                                    .unwrap();
                                FileSource::Stored(reference)
                            } else {
                                FileSource::Local(path.clone())
                            };
                            match action {
                                "hash" => {
                                    let started = Instant::now();
                                    let hash = fixture.storage.hash_file(HashFileRequest { source: input, range: range() }).await.unwrap();
                                    elapsed += started.elapsed();
                                    assert_eq!(hash.bytes_read, FILE_BYTES as u64);
                                    assert_eq!(hash.sha256, expected);
                                }
                                "capture" => {
                                    let started = Instant::now();
                                    let reference = fixture
                                        .storage
                                        .capture_file(CaptureFileRequest {
                                            source: input,
                                            range: range(),
                                            metadata: metadata("captured", expected),
                                            consistency: CaptureConsistency::Stable,
                                        })
                                        .await
                                        .unwrap();
                                    elapsed += started.elapsed();
                                    assert_eq!(reference.length, FILE_BYTES as u64);
                                    assert_eq!(reference.sha256, expected);
                                    let hash = fixture
                                        .storage
                                        .hash_file(HashFileRequest { source: FileSource::Stored(reference), range: range() })
                                        .await
                                        .unwrap();
                                    assert_eq!(hash.sha256, expected);
                                }
                                "read" => {
                                    let started = Instant::now();
                                    let mut reader =
                                        fixture.storage.read_file(ReadFileRequest { source: input, range: range() }).await.unwrap();
                                    elapsed += started.elapsed();
                                    let mut hash = Sha256::new();
                                    let mut count = 0_u64;
                                    loop {
                                        let started = Instant::now();
                                        let batch = reader.next().await.unwrap();
                                        elapsed += started.elapsed();
                                        let Some(batch) = batch else { break };
                                        assert_eq!(batch.offset(), count);
                                        count += batch.bytes().len() as u64;
                                        hash.update(batch.bytes());
                                    }
                                    assert_eq!(count, FILE_BYTES as u64);
                                    assert_eq!(<[u8; 32]>::from(hash.finalize()), expected);
                                }
                                "follow" => {
                                    let started = Instant::now();
                                    let mut reader = fixture
                                        .storage
                                        .follow_file(FollowFileRequest { source: input, start: FollowStart::Beginning })
                                        .await
                                        .unwrap();
                                    elapsed += started.elapsed();
                                    let mut hash = Sha256::new();
                                    let mut count = 0_u64;
                                    while count < FILE_BYTES as u64 {
                                        let started = Instant::now();
                                        let event = reader.next().await.unwrap().expect("unchanged source contains the complete range");
                                        elapsed += started.elapsed();
                                        assert_eq!(event.offset(), Some(count));
                                        let bytes = event.data().expect("unchanged source emits data");
                                        count += bytes.len() as u64;
                                        hash.update(bytes);
                                    }
                                    assert_eq!(count, FILE_BYTES as u64);
                                    assert_eq!(<[u8; 32]>::from(hash.finalize()), expected);
                                }
                                _ => unreachable!(),
                            }
                            fixture.finish().await;
                        }
                        elapsed
                    })
                });
            });
        }
    }
    group.finish();
}

/* === Private Functions === */

fn range() -> ByteRange {
    ByteRange { offset: 0, limit: None }
}

fn metadata(id: &str, digest: [u8; 32]) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(FILE_BYTES as u64),
        expected_sha256: Some(digest),
    }
}

fn source(path: &Path) -> [u8; 32] {
    let chunk = super::waveforms::binary(CHUNK_BYTES);
    let mut file = std::fs::File::create(path).unwrap();
    let mut hash = Sha256::new();
    for _ in 0..FILE_BYTES / CHUNK_BYTES {
        file.write_all(&chunk).unwrap();
        hash.update(&chunk);
    }
    file.sync_all().unwrap();
    hash.finalize().into()
}

/* === Tests === */
