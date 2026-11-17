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

mod fixture;
mod opened_files;
mod projection;
mod spill;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion};
use tokio::runtime::Runtime;

// Local Imports
use harmos_domain::state::Position;
use harmos_query::{Accumulator, Quantiles};
use harmos_storage::Aggregate;
use harmos_storage::memory::RecoveryBackend;
use harmos_storage::query::scalar::Agg;
use harmos_storage::query::scalar::ScalarValue;
use harmos_storage::state::{RecoveryEntry, RecoverySnapshot, SnapshotPayload, StateBackend};
use harmos_storage::{FileMetadata, Identifier, MutationId, ReadLimits, RecoveryPosition, StorageBatch, StorageCursor};

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
                                let ScalarValue::Float(actual) = answer else {
                                    panic!("order statistic must return a floating-point value");
                                };
                                let expected = (n - 1) as f64 * fraction;
                                let tolerance = 2.0 * f64::EPSILON * expected.abs().max(1.0);
                                assert!((actual - expected).abs() <= tolerance, "{statistic:?}: {actual} != {expected}");
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
    let runtime = Runtime::new().unwrap();
    spill::benchmarks(&mut criterion);
    opened_files::benchmarks(&mut criterion, &runtime);
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
                let mut values = Quantiles::new(memory);
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
    group.bench_function("seven_days", |b| b.iter(|| runtime.block_on(journal_recovery())));
    group.finish();
    let mut group = criterion.benchmark_group("integration/mutation_receipts");
    group.bench_function("2048_files", |b| b.iter(|| runtime.block_on(mutation_receipts())));
    group.finish();
    criterion.final_summary();
}

async fn journal_recovery() {
    let history = fixture::history();
    let journal = RecoveryBackend::new(history.clone());
    journal.create_recovery_session().await.unwrap();
    for day in 0..7 {
        let entries = (0..1440)
            .map(|minute| {
                let sequence = day * 1440 + minute + 1;
                RecoveryEntry {
                    position: RecoveryPosition { history: history.clone(), sequence },
                    required_data: StorageCursor { history: history.clone(), sequence: 0 },
                    payload: sequence.to_le_bytes().to_vec(),
                }
            })
            .collect();
        journal.append_recovery_entries(StorageBatch::new(entries)).await.unwrap();
    }
    let through = RecoveryPosition {
        history: history.clone(),
        sequence: 7 * 1440,
    };
    journal.flush(&through).await.unwrap();
    let mut after = RecoveryPosition { history: history.clone(), sequence: 0 };
    let limits = ReadLimits {
        rows: NonZeroUsize::new(31).unwrap(),
        bytes: NonZeroUsize::new(16384).unwrap(),
    };
    while after.sequence < through.sequence {
        let page = journal.list_recovery_entries(&after, &through, limits).await.unwrap();
        after = page.entries.last().unwrap().position.clone();
        black_box(page);
    }
    let checkpoint = RecoveryPosition {
        history: history.clone(),
        sequence: through.sequence - 10,
    };
    let snapshot = RecoverySnapshot {
        through: checkpoint.clone(),
        required_data: StorageCursor { history, sequence: 0 },
        payload: SnapshotPayload::from_bytes(&[7], NonZeroUsize::new(65536).unwrap(), Position::ORIGIN).unwrap(),
    };
    journal.set_recovery_snapshot(StorageBatch::new(vec![snapshot])).await.unwrap();
    journal.flush(&through).await.unwrap();
    black_box(journal.list_recovery_entries(&checkpoint, &through, limits).await.unwrap());
}

async fn mutation_receipts() {
    let mut settings = fixture::config();
    // Retain Both Mutation Identities for Every File in This Workload.
    settings.request.max_mutation_receipts = NonZeroUsize::new(2 * 2048).unwrap();
    let f = fixture::Fixture::new(settings).await;
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

/* === Tests === */
