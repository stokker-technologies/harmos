/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Benchmark Exact Scalar Order Statistics

/* === Modules === */

#[path = "../../tests/support/storage.rs"]
#[allow(dead_code)]
mod backend;
#[path = "../../tests/support/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "../../tests/support/mapping.rs"]
mod mapping_fixture;
mod projection;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion};

// Local Imports
use harmos_storage::{
    Aggregate,
    query::{
        Accumulator,
        scalar::{Agg, ScalarValue},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut group = criterion.benchmark_group("integration/order_statistics");
    for n in [2, 3, 4, 11, 13, 45, 100, 2049] {
        for statistic in [
            Aggregate::Median,
            Aggregate::Percentile(95),
            Aggregate::Percentile(99),
            Aggregate::Count,
            Aggregate::Min,
            Aggregate::Max,
            Aggregate::Mean,
            Aggregate::Std,
            Aggregate::Rms,
            Aggregate::Sum,
        ] {
            for phase in ["push", "finish"] {
                let name = format!("{statistic:?}/{phase}");
                group.bench_with_input(BenchmarkId::new(name, n), &n, |b, &n| {
                    b.iter_custom(|iterations| {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            let mut accumulator = Accumulator::new(Agg::Of(statistic));
                            let start = Instant::now();
                            for (capture, value) in (0..n).rev().enumerate() {
                                accumulator.push(capture as u64, &ScalarValue::Float(black_box(value as f64))).unwrap();
                            }
                            let pushed = start.elapsed();
                            let start = Instant::now();
                            let answer = black_box(accumulator.finish(&mut || Ok(())).unwrap());
                            let finished = start.elapsed();
                            let fraction = match statistic {
                                Aggregate::Median => Some(0.5),
                                Aggregate::Percentile(rank) => Some(f64::from(rank) / 100.0),
                                _ => None,
                            };
                            if let Some(fraction) = fraction {
                                assert_eq!(answer, ScalarValue::Float((n - 1) as f64 * fraction));
                            }
                            elapsed += if phase == "push" { pushed } else { finished };
                        }
                        elapsed
                    });
                });
            }
        }
    }
    group.finish();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = criterion.benchmark_group("integration/storage_projection");
    for rows in [40_000, 1_000_000] {
        let mapped = runtime.block_on(projection::prepare(rows));
        for (name, offset, limit) in [("full", 0, rows), ("window", rows / 3, rows.min(4096))] {
            group.bench_function(BenchmarkId::new(name, rows), |b| {
                b.iter(|| runtime.block_on(projection::scan(&mapped, black_box(offset), black_box(limit))));
            });
        }
        runtime.block_on(mapped.finish());
    }
    group.finish();

    let mut group = criterion.benchmark_group("integration/quantile_spill");
    for memory in [0, 4101 * 16] {
        group.bench_with_input(BenchmarkId::from_parameter(memory), &memory, |b, &memory| {
            b.iter(|| {
                let mut values = harmos_storage::query::quantiles::Quantiles::new(memory);
                for index in 0..4096 {
                    values.push(black_box(((index * 7919) % 4093) as f64 - 2046.0)).unwrap();
                }
                for value in [-0.0, 0.0, f64::from_bits(1), -f64::MAX, f64::MAX] {
                    values.push(value).unwrap();
                }
                let percentiles: Vec<_> = (0..=100).collect();
                black_box(values.values(&percentiles, 1024 * 1024, &mut || Ok(())).unwrap());
            });
        });
    }
    group.finish();

    let mut group = criterion.benchmark_group("integration/journal_recovery");
    group.bench_function("seven_days", |b| b.iter(journal_recovery));
    group.finish();
    let mut group = criterion.benchmark_group("integration/mutation_receipts");
    group.bench_function("2048_files", |b| b.iter(|| runtime.block_on(mutation_receipts())));
    group.finish();
    criterion.final_summary();
}

/* === Tests === */

fn journal_recovery() {
    use harmos_storage::state::{RecoveryEntry, StorageStateSettings, file::Journal};
    use harmos_storage::{ReadLimits, RecoveryPosition, StorageCursor};
    use std::{fs::OpenOptions, num::NonZeroUsize, sync::Arc};
    let root = tempfile::tempdir().unwrap();
    let history = backend::history();
    let open = || {
        Journal::open(
            root.path(),
            history.clone(),
            StorageStateSettings {
                max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
                max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
            },
            Arc::new(|path| OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)),
            &mut || Ok(()),
        )
        .unwrap()
    };
    let mut journal = open();
    for day in 0..7 {
        let entries: Vec<_> = (0..1440)
            .map(|minute| {
                let sequence = day * 1440 + minute + 1;
                RecoveryEntry {
                    position: RecoveryPosition { history: history.clone(), sequence },
                    required_data: StorageCursor { history: history.clone(), sequence: 0 },
                    payload: sequence.to_le_bytes().to_vec(),
                }
            })
            .collect();
        journal.append(&entries).unwrap();
    }
    let through = journal.position();
    drop(journal);
    let mut journal = open();
    let mut after = RecoveryPosition { history: history.clone(), sequence: 0 };
    while after.sequence < through.sequence {
        let page = journal
            .page(
                &after,
                &through,
                ReadLimits {
                    rows: NonZeroUsize::new(31).unwrap(),
                    bytes: NonZeroUsize::new(16384).unwrap(),
                },
            )
            .unwrap();
        after = page.entries.last().unwrap().position.clone();
        black_box(page);
    }
    let checkpoint = RecoveryPosition {
        history: history.clone(),
        sequence: through.sequence - 10,
    };
    let snapshot = harmos_storage::state::RecoverySnapshot {
        through: checkpoint.clone(),
        required_data: StorageCursor { history, sequence: 0 },
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            &[7],
            NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };
    journal.checkpoint(&snapshot).unwrap();
    black_box(
        journal
            .page(
                &checkpoint,
                &through,
                ReadLimits {
                    rows: NonZeroUsize::new(31).unwrap(),
                    bytes: NonZeroUsize::new(16384).unwrap(),
                },
            )
            .unwrap(),
    );
}

async fn mutation_receipts() {
    use harmos_storage::{FileMetadata, Identifier, MutationId};
    let f = fixture::Fixture::new(fixture::config()).await;
    let window = f.storage.mutation_window().unwrap().id;
    let mut last = None;
    for index in 0..2048 {
        let id = Identifier::new(format!("file-{index}")).unwrap();
        let create = MutationId {
            window: window.clone(),
            key: Identifier::new(format!("create-{index}")).unwrap(),
        };
        let metadata = FileMetadata {
            id,
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        };
        let producer = f.storage.create_file_once(&create, &metadata).await.unwrap();
        let finish = MutationId {
            window: window.clone(),
            key: Identifier::new(format!("finish-{index}")).unwrap(),
        };
        let reference = f.storage.finish_file_once(&finish, &producer.session).await.unwrap();
        assert_eq!(reference.length, 0);
        f.storage.flush().await.unwrap();
        last = Some((finish, producer.session, reference));
    }
    assert_eq!(f.storage.mutation_window().unwrap().id, window);
    let (finish, session, reference) = last.unwrap();
    assert_eq!(f.storage.finish_file_once(&finish, &session).await.unwrap(), reference);
    f.finish().await;
}
