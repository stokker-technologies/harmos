/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Durable Native Path Against The Locked Multi-Day Workload.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    hint::black_box,
    num::{NonZeroU32, NonZeroUsize},
    time::Instant,
};

// Third Party Imports
use arrow_array::Float64Array;
use criterion::Criterion;

// Local Imports
use super::workload::{self, ROWS, Sample};
use harmos_domain::{Message, schema::Described};
use harmos_storage::{
    query::{
        columnar::{ArrowScan, Cancellation, ColumnKind, ColumnSource, scan_arrow},
        index::{IndexDefinition, IndexRead, scan_index},
        projection::{ProjectionDefinition, ProjectionRead, ProjectionTransform, advance_projection},
    },
    sqlite::{SqliteBackend, SqliteSettings},
    *,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn run(runtime: &tokio::runtime::Runtime, criterion: &mut Criterion) {
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("multi-day").unwrap(),
        epoch: Identifier::new("native").unwrap(),
    };
    let backend = SqliteBackend::new(
        root.path().join("storage.sqlite"),
        history.clone(),
        SqliteSettings {
            chunk_rows: nz(16384),
            chunk_bytes: nz(8 * 1024 * 1024),
        },
    )
    .unwrap();
    let key = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let schema =
        harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let source = ColumnSource {
        kind: ColumnKind::Records,
        schema: key.clone(),
    };
    let work = ReadLimits {
        rows: nz(ROWS + 1),
        bytes: nz(1024 * 1024 * 1024),
    };
    runtime.block_on(async {
        backend.open().await.unwrap();
        backend
            .write_schemas(StorageBatch::new(vec![StorageSchema::new(
                StorageCursor { history: history.clone(), sequence: 1 },
                key.clone(),
                schema,
            )]))
            .await
            .unwrap();
        let start = Instant::now();
        for first in (0..ROWS).step_by(16384) {
            let rows = (first..(first + 16384).min(ROWS))
                .map(|index| StoredRecord {
                    position: StorageCursor {
                        history: history.clone(),
                        sequence: index as u64 + 2,
                    },
                    schema: key.clone(),
                    scope: None,
                    item: Identifier::new(format!("sample-{index}")).unwrap(),
                    source: None,
                    source_time: None,
                    payload: workload::sample(index).encode(),
                })
                .collect();
            backend.write_records(StorageBatch::new(rows)).await.unwrap();
        }
        println!("native/ingest: {ROWS} rows in {:.6} seconds", start.elapsed().as_secs_f64());
    });
    let cut = StorageCursor { history, sequence: ROWS as u64 + 1 };
    let snapshot = runtime.block_on(backend.open_snapshot(&cut)).unwrap();
    let mut group = criterion.benchmark_group("integration/multi_day/native");
    for (name, offset, limit) in [("full", 0, ROWS), ("first_window", ROWS / 3, 4096), ("next_window", ROWS / 3 + 4096, 4096)] {
        let read = ArrowScan {
            source: source.clone(),
            columns: vec!["payload.value".into()],
            predicate: None,
            time: None,
            start: offset as u64 + 1,
            after: None,
            limits: ReadLimits {
                rows: nz(4096),
                bytes: nz(8 * 1024 * 1024),
            },
            work,
        };
        let start = Instant::now();
        let measured = scan(runtime, &backend, &snapshot, &read, limit);
        let elapsed = start.elapsed();
        let expected = (offset..offset + limit).map(|index| workload::sample(index).value).sum::<f64>();
        assert_eq!(measured.0, limit);
        assert_eq!(measured.1, expected);
        println!(
            "native/{name}: {} rows in {:.6} seconds; {} column bytes; {} payload rows",
            measured.0,
            elapsed.as_secs_f64(),
            measured.2,
            measured.3
        );
        group.bench_function(name, |b| b.iter(|| black_box(scan(runtime, &backend, &snapshot, &read, limit))));
    }
    let definition = ProjectionDefinition {
        name: Identifier::new("values").unwrap(),
        version: 1,
        source: source.clone(),
        transform: ProjectionTransform::Select(vec!["payload.value".into()]),
    };
    runtime.block_on(backend.register_projection(&definition, false)).unwrap();
    let start = Instant::now();
    let revision = runtime.block_on(advance_projection(&backend, &snapshot, &definition.name, work, &Cancellation::default())).unwrap();
    println!("native/projection_catchup: {ROWS} source rows in {:.6} seconds", start.elapsed().as_secs_f64());
    let read = ProjectionRead {
        name: definition.name.clone(),
        revision,
        frontier: cut.clone(),
        after: ROWS / 3,
        limits: ReadLimits {
            rows: nz(4096),
            bytes: nz(8 * 1024 * 1024),
        },
        work,
    };
    let page = runtime.block_on(backend.read_projection(&read)).unwrap();
    assert_eq!(page.batch.num_rows(), 4096);
    assert_eq!(sum(&page.batch), (ROWS / 3..ROWS / 3 + 4096).map(|index| workload::sample(index).value).sum::<f64>());
    group.bench_function("projection_window", |b| b.iter(|| black_box(runtime.block_on(backend.read_projection(&read)).unwrap())));
    group.bench_function("projection_replay", |b| {
        b.iter(|| {
            black_box(runtime.block_on(advance_projection(&backend, &snapshot, &definition.name, work, &Cancellation::default())).unwrap())
        })
    });
    let index = IndexDefinition {
        name: Identifier::new("by-value").unwrap(),
        version: 1,
        source,
        columns: vec!["payload.value".into()],
    };
    let start = Instant::now();
    runtime.block_on(backend.register_index(&index, false, work, &Cancellation::default())).unwrap();
    println!("native/index_build: {ROWS} source rows in {:.6} seconds", start.elapsed().as_secs_f64());
    let read = IndexRead {
        name: index.name,
        lower: None,
        upper: None,
        prefix: vec![],
        descending: false,
        after: None,
        limits: ReadLimits {
            rows: nz(4096),
            bytes: nz(8 * 1024 * 1024),
        },
        work,
    };
    let page = runtime.block_on(scan_index(&backend, &snapshot, &read, &Cancellation::default())).unwrap();
    let mut expected = (0..ROWS).map(|index| (workload::sample(index).value as u64, index as u64 + 2)).collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(
        page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(),
        expected[..4096].iter().map(|entry| entry.1).collect::<Vec<_>>()
    );
    group.bench_function("indexed_window", |b| {
        b.iter(|| black_box(runtime.block_on(scan_index(&backend, &snapshot, &read, &Cancellation::default())).unwrap()))
    });
    group.finish();
    runtime.block_on(backend.release_snapshot(&snapshot)).unwrap();
    runtime.block_on(backend.close()).unwrap();
    drop(backend); // TempDir Removes Only This Workload's Database After Closing Its Lease.
}

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn sum(batch: &arrow_array::RecordBatch) -> f64 {
    batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values().iter().sum()
}
fn scan(
    runtime: &tokio::runtime::Runtime, backend: &SqliteBackend, snapshot: &BackendSnapshot, read: &ArrowScan, limit: usize,
) -> (usize, f64, usize, usize) {
    runtime.block_on(async {
        let mut read = read.clone();
        let mut result = (0, 0.0, 0, 0);
        loop {
            read.limits.rows = nz(4096.min(limit - result.0));
            let page = scan_arrow(backend, snapshot, &read, &Cancellation::default()).await.unwrap();
            result.0 += page.batch.num_rows();
            result.1 += sum(&page.batch);
            result.2 += page.bytes_read;
            result.3 += page.payload_rows;
            if result.0 == limit || page.complete {
                break;
            }
            read.after = page.next;
        }
        result
    })
}

/* === Tests === */
