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
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};
// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_domain::model::storage::{DirectoryEvent, DirectoryRule, DirectorySelection};
// Local Imports
use super::fixture::Fixture;

/* === Constants === */

const ENTRIES: usize = 1000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(criterion: &mut Criterion, executor: &tokio::runtime::Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let expected: BTreeSet<_> = (0..ENTRIES).map(|index| format!("entry-{index:04}.dat")).collect();
    for name in &expected {
        std::fs::write(directory.path().join(name), []).unwrap();
    }
    let mut group = criterion.benchmark_group("storage/directories");
    group.sample_size(10);
    group.sampling_mode(criterion::SamplingMode::Flat);
    group.throughput(Throughput::Elements(ENTRIES as u64));
    for action in ["list", "watch-initial"] {
        group.bench_function(BenchmarkId::new(action, ENTRIES), |bencher| {
            bencher.iter_custom(|iterations| {
                executor.block_on(async {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let fixture = Fixture::new().await;
                        let mut observed = BTreeSet::new();
                        if action == "list" {
                            let started = Instant::now();
                            let mut reader = fixture.storage.list_directory(selection(directory.path())).await.unwrap();
                            elapsed += started.elapsed();
                            loop {
                                let started = Instant::now();
                                let item = reader.next().await.unwrap();
                                elapsed += started.elapsed();
                                let Some(item) = item else { break };
                                assert!(observed.insert(item.path.clone()));
                            }
                            assert_eq!(reader.completion().unwrap().entries, ENTRIES as u64);
                        } else {
                            let started = Instant::now();
                            let mut reader = fixture.storage.watch_directory(selection(directory.path()), true).await.unwrap();
                            elapsed += started.elapsed();
                            loop {
                                let started = Instant::now();
                                let item = reader.next().await.unwrap().unwrap();
                                elapsed += started.elapsed();
                                match &*item {
                                    DirectoryEvent::Existing(entry) => {
                                        assert!(observed.insert(entry.path.clone()));
                                    }
                                    DirectoryEvent::InitialScanComplete => break,
                                    event => panic!("unchanged initial directory emitted {event:?}"),
                                }
                            }
                        }
                        assert_eq!(observed, expected);
                        fixture.finish().await;
                    }
                    elapsed
                })
            });
        });
    }
    group.throughput(Throughput::Elements(1));
    group.bench_function("watch-create", |bencher| {
        bencher.iter_custom(|iterations| {
            executor.block_on(async {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let fixture = Fixture::new().await;
                    let changed = tempfile::tempdir().unwrap();
                    let mut watcher = fixture.storage.watch_directory(selection(changed.path()), false).await.unwrap();
                    assert!(matches!(&*watcher.next().await.unwrap().unwrap(), DirectoryEvent::InitialScanComplete));
                    let started = Instant::now();
                    std::fs::write(changed.path().join("created.dat"), []).unwrap();
                    let event = tokio::time::timeout(Duration::from_secs(10), watcher.next()).await.unwrap().unwrap().unwrap();
                    elapsed += started.elapsed();
                    assert!(matches!(&*event, DirectoryEvent::Created(entry) if entry.path == "created.dat"));
                    drop(event);
                    drop(watcher);
                    fixture.finish().await;
                }
                elapsed
            })
        });
    });
    group.finish();
}

/* === Private Functions === */

fn selection(root: &Path) -> DirectorySelection {
    DirectorySelection {
        root: root.to_owned(),
        recursive: true,
        rules: vec![DirectoryRule {
            pattern: "**/*.dat".into(),
            include: true,
        }],
    }
}

/* === Tests === */
