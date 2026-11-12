/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Reusable benchmark contract. Install TrackingAllocator in a dedicated executable,
//! then call run on an empty backend and verify_reopened after reopening it.

/* === Modules === */

mod allocations;
pub mod workload;

/* === Re-Exports === */

pub use allocations::TrackingAllocator;

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

// Third Party Imports
use arrow_array::{BinaryArray, Float64Array, RecordBatch, UInt64Array};
use serde::Serialize;

// Local Imports
use crate::query::{
    columnar::{ArrowScan, Cancellation, scan_arrow},
    index::{encode_key, scan_index},
    projection::{ProjectionRead, advance_projection},
    scalar::ScalarValue,
};
use crate::*;
use workload::{CHUNK_ROWS, CONTENTION_ROUNDS, SAMPLES, WINDOW_ROWS, WRITERS, Workload, sample};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Serialize)]
pub struct Measurement {
    pub name: String,
    pub seconds: f64,
    pub allocations: u64,
    pub allocated_bytes: u64,
    pub peak_heap_bytes: u64,
}

pub struct Completed {
    pub replay: ObjectExchange,
    pub outcome: CompareExchange,
    pub cut: StorageCursor,
    pub winner: ObjectEntry,
}

#[derive(Debug)]
pub struct ScanResult {
    pub rows: usize,
    pub sum: f64,
    pub bytes_read: usize,
    pub payload_rows: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Run every case, including owner disposal and restart, with a backend-owned opener.
pub fn run_with_reopen(runtime: &tokio::runtime::Runtime, workload: &Workload, mut open: impl FnMut() -> Box<dyn StorageBackend>) {
    let mut owner = Some(open());
    let completed = run(runtime, owner.as_deref().unwrap(), workload);
    measure("storage.stop.2m", 1, || drop(owner.take()));
    let backend = measure("storage.restart.2m", 1, open);
    verify_reopened(runtime, backend.as_ref(), workload, &completed);
}

/// A full, isolated run on an initially empty backend. Setup and result checks are untimed.
/// The reference fixture uses WAL/FULL; consumers must disclose their durability mode.
pub fn run(runtime: &tokio::runtime::Runtime, backend: &dyn StorageBackend, workload: &Workload) -> Completed {
    // Populate Identical Source Batches.
    assert_eq!(workload.rows, workload::ROWS, "published case names require the locked two-million-row workload");
    assert_eq!(runtime.block_on(backend.open()).unwrap().sequence, 0, "benchmark backend must be empty");
    runtime.block_on(backend.write_schemas(StorageBatch::new(vec![workload.schema(1)]))).unwrap();
    let batches = (0..workload.rows)
        .step_by(CHUNK_ROWS)
        .map(|first| workload.records(first, (first + CHUNK_ROWS).min(workload.rows)))
        .collect::<Vec<_>>();
    measure("storage.ingest.2m", 1, || {
        runtime.block_on(async {
            for batch in &batches {
                backend.write_records(batch.clone()).await.unwrap();
            }
        });
    });
    drop(batches);
    let cut = workload.cut();
    assert_eq!(runtime.block_on(backend.cursor_saved()).unwrap(), cut);
    let snapshot = runtime.block_on(backend.open_snapshot(&cut)).unwrap();

    // Read Cold-Owner And Warm Pages.
    for (name, offset, limit) in [
        ("storage.scan.read.2m", 0, workload.rows),
        ("storage.window.read.2m", workload.rows / 3, WINDOW_ROWS),
        ("storage.window.next.2m", workload.rows / 3 + WINDOW_ROWS, WINDOW_ROWS),
    ] {
        let read = workload.scan(offset);
        let result = measure(&format!("{name}.first"), 1, || scan(runtime, backend, &snapshot, &read, limit));
        verify_scan(workload, offset, limit, &result);
        println!(
            "STORAGE_COUNTER {}",
            serde_json::json!({"name": name, "bytes_read": result.bytes_read, "rows": result.rows,
            "payload_rows": result.payload_rows})
        );
        let result = measure(name, SAMPLES, || scan(runtime, backend, &snapshot, &read, limit));
        verify_scan(workload, offset, limit, &result);
    }

    // Publish And Replay An Exact Projection Frontier.
    let definition = workload.projection();
    let revision = runtime.block_on(backend.register_projection(&definition, false)).unwrap();
    let cancel = Cancellation::default();
    measure("storage.projection.advance.2m", 1, || {
        runtime.block_on(advance_projection(backend, &snapshot, &definition.name, workload.work(), &cancel)).unwrap()
    });
    assert_eq!(runtime.block_on(backend.projection_frontier(&definition.name, revision)).unwrap(), Some(cut.clone()));
    let read = ProjectionRead {
        name: definition.name.clone(),
        revision,
        frontier: cut.clone(),
        after: workload.rows / 3,
        limits: workload.page(),
        work: workload.work(),
    };
    let page = measure("storage.projection.read.2m", SAMPLES, || runtime.block_on(backend.read_projection(&read)).unwrap());
    assert_eq!(page.batch.num_rows(), WINDOW_ROWS);
    assert_eq!(sum(&page.batch), expected_sum(workload.rows / 3, WINDOW_ROWS));
    measure("storage.projection.replay.2m", SAMPLES, || {
        runtime.block_on(advance_projection(backend, &snapshot, &definition.name, workload.work(), &cancel)).unwrap()
    });
    assert_eq!(runtime.block_on(backend.projection_frontier(&definition.name, revision)).unwrap(), Some(cut.clone()));

    // Compare First, Next And Deep Index Seeks.
    let index = workload.index();
    measure("storage.index.build.2m", 1, || runtime.block_on(backend.register_index(&index, false, workload.work(), &cancel)).unwrap());
    let mut expected = (0..workload.rows).map(|index| (sample(index).value as u64, index as u64 + 2)).collect::<Vec<_>>();
    expected.sort_unstable();
    let read = workload.index_read();
    let page = measure("storage.index.read.2m", SAMPLES, || runtime.block_on(scan_index(backend, &snapshot, &read, &cancel)).unwrap());
    assert_eq!(
        page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(),
        expected[..WINDOW_ROWS].iter().map(|entry| entry.1).collect::<Vec<_>>()
    );
    let next = crate::query::index::IndexRead { after: page.next, ..read.clone() };
    let page = measure("storage.index.next.2m", SAMPLES, || runtime.block_on(scan_index(backend, &snapshot, &next, &cancel)).unwrap());
    assert_eq!(
        page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(),
        expected[WINDOW_ROWS..2 * WINDOW_ROWS].iter().map(|entry| entry.1).collect::<Vec<_>>()
    );
    let seek = crate::query::index::IndexSeek {
        snapshot: snapshot.token.clone(),
        plan: read.identity(&index).unwrap(),
        revision: 1,
        key: encode_key(&[ScalarValue::Float(expected[workload.rows / 2 - 1].0 as f64)]).unwrap(),
        position: expected[workload.rows / 2 - 1].1,
    };
    let deep = crate::query::index::IndexRead { after: Some(seek), ..read };
    let page = measure("storage.index.deep.2m", SAMPLES, || runtime.block_on(scan_index(backend, &snapshot, &deep, &cancel)).unwrap());
    assert_eq!(
        page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(),
        expected[workload.rows / 2..workload.rows / 2 + WINDOW_ROWS].iter().map(|entry| entry.1).collect::<Vec<_>>()
    );
    drop(expected);

    // Publish Buckets, Then Replace Late And Corrected Identities.
    let definition = workload.buckets();
    let revision = runtime.block_on(backend.register_projection(&definition, false)).unwrap();
    measure("storage.bucket.advance.2m", 1, || {
        runtime.block_on(advance_projection(backend, &snapshot, &definition.name, workload.work(), &cancel)).unwrap()
    });
    verify_buckets(runtime, backend, workload, &cut, false);
    let replacements = (0..WINDOW_ROWS)
        .map(|index| {
            let mut value = sample(index);
            value.time_ns -= 60 * 1_000_000_000;
            value.value += 1.0;
            workload.record(index, cut.sequence + index as u64 + 1, value)
        })
        .collect();
    runtime.block_on(backend.write_records(StorageBatch::new(replacements))).unwrap();
    let late_cut = workload.position(cut.sequence + WINDOW_ROWS as u64);
    let late = runtime.block_on(backend.open_snapshot(&late_cut)).unwrap();
    measure("storage.bucket.replace.2m", 1, || {
        runtime.block_on(advance_projection(backend, &late, &definition.name, workload.work(), &cancel)).unwrap()
    });
    verify_buckets(runtime, backend, workload, &late_cut, true);
    verify_buckets(runtime, backend, workload, &cut, false);
    assert_eq!(runtime.block_on(backend.projection_frontier(&definition.name, revision)).unwrap(), Some(late_cut.clone()));

    // Measure Refusal Before Read Admission.
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let result = measure("storage.scan.cancel.2m", SAMPLES, || runtime.block_on(scan_arrow(backend, &late, &workload.scan(0), &cancelled)));
    assert!(result.is_err());
    let result = measure("storage.projection.cancel.2m", SAMPLES, || {
        runtime.block_on(advance_projection(backend, &late, &workload.projection().name, workload.work(), &cancelled))
    });
    assert!(result.is_err());
    assert_eq!(runtime.block_on(backend.projection_frontier(&workload.projection().name, 1)).unwrap(), Some(cut));
    runtime.block_on(backend.release_snapshot(&snapshot)).unwrap();
    runtime.block_on(backend.release_snapshot(&late)).unwrap();

    // Race Eight Updates Against One Version In Each Round.
    let first = late_cut.sequence + 2;
    let create = workload.exchange(first, None, 0);
    let created = runtime.block_on(backend.compare_exchange(&create)).unwrap();
    let CompareExchange::Applied(receipt) = created else { panic!("initial create must apply") };
    let mut current = receipt.version;
    let mut winner = create.value;
    let mut last = None;
    for round in 0..CONTENTION_ROUNDS {
        let exchanges = (0..WRITERS)
            .map(|writer| {
                workload.exchange(
                    first + 2 + (round * WRITERS + writer) as u64 * 2,
                    Some(current.clone()),
                    (round * WRITERS + writer + 1) as u64,
                )
            })
            .collect::<Vec<_>>();
        let outcomes = measure("storage.object.contend.8", 1, || {
            runtime.block_on(async {
                futures_util::future::join_all(exchanges.iter().map(|exchange| backend.compare_exchange(exchange))).await
            })
        });
        let outcomes = outcomes.into_iter().map(Result::unwrap).collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|outcome| matches!(outcome, CompareExchange::Applied(_))).count(), 1);
        for (exchange, outcome) in exchanges.into_iter().zip(outcomes) {
            if let CompareExchange::Applied(receipt) = &outcome {
                current = receipt.version.clone();
                winner = exchange.value.clone();
            }
            last = Some((exchange, outcome));
        }
    }
    let (replay, outcome) = last.unwrap();
    let replayed = measure("storage.object.replay.512", SAMPLES, || runtime.block_on(backend.compare_exchange(&replay)).unwrap());
    assert_eq!(replayed, outcome);
    let final_cut = runtime.block_on(backend.cursor_saved()).unwrap();
    verify_object(runtime, backend, &final_cut, &winner);
    measure("storage.close.2m", 1, || runtime.block_on(backend.close()).unwrap());
    Completed { replay, outcome, cut: final_cut, winner }
}

/// Verify durable outcomes and retained projection output on a freshly reopened owner.
pub fn verify_reopened(runtime: &tokio::runtime::Runtime, backend: &dyn StorageBackend, workload: &Workload, completed: &Completed) {
    let cut = runtime.block_on(backend.open()).unwrap();
    assert_eq!(cut, completed.cut);
    verify_object(runtime, backend, &cut, &completed.winner);
    let outcome =
        measure("storage.object.restart_replay.512", SAMPLES, || runtime.block_on(backend.compare_exchange(&completed.replay)).unwrap());
    assert_eq!(outcome, completed.outcome);
    verify_buckets(runtime, backend, workload, &workload.position(workload.cut().sequence + WINDOW_ROWS as u64), true);
    let snapshot = runtime.block_on(backend.open_snapshot(&workload.cut())).unwrap();
    measure("storage.projection.restart_replay.2m", SAMPLES, || {
        runtime
            .block_on(advance_projection(backend, &snapshot, &workload.projection().name, workload.work(), &Cancellation::default()))
            .unwrap()
    });
    runtime.block_on(backend.release_snapshot(&snapshot)).unwrap();
    runtime.block_on(backend.close()).unwrap();
}

pub fn measure<T>(name: &str, samples: usize, mut action: impl FnMut() -> T) -> T {
    if samples > 1 {
        let warmup = Instant::now();
        while warmup.elapsed() < Duration::from_secs(1) {
            std::hint::black_box(action());
        }
    }
    let mut measurements = Vec::with_capacity(samples);
    let mut result = None;
    for _ in 0..samples {
        let allocation = allocations::Region::start();
        let start = Instant::now();
        let value = action();
        let seconds = start.elapsed().as_secs_f64();
        let (allocations, allocated_bytes, peak_heap_bytes) = allocation.finish();
        measurements.push(Measurement {
            name: name.into(),
            seconds,
            allocations,
            allocated_bytes,
            peak_heap_bytes,
        });
        result = Some(value);
    }
    measurements.sort_by(|left, right| left.seconds.total_cmp(&right.seconds));
    let mut median = measurements[measurements.len() / 2].clone();
    if measurements.len().is_multiple_of(2) {
        let lower = &measurements[measurements.len() / 2 - 1];
        median.seconds = (median.seconds + lower.seconds) / 2.0;
        median.allocations = (median.allocations + lower.allocations) / 2;
        median.allocated_bytes = (median.allocated_bytes + lower.allocated_bytes) / 2;
        median.peak_heap_bytes = (median.peak_heap_bytes + lower.peak_heap_bytes) / 2;
    }
    println!("STORAGE_MEASUREMENT {}", serde_json::to_string(&median).unwrap());
    println!("STORAGE_SAMPLES {}", serde_json::to_string(&measurements).unwrap());
    result.unwrap()
}

pub fn scan(
    runtime: &tokio::runtime::Runtime, backend: &dyn StorageBackend, snapshot: &BackendSnapshot, read: &ArrowScan, limit: usize,
) -> ScanResult {
    runtime.block_on(async {
        let mut read = read.clone();
        let mut result = ScanResult {
            rows: 0,
            sum: 0.0,
            bytes_read: 0,
            payload_rows: 0,
        };
        loop {
            read.limits.rows = std::num::NonZeroUsize::new(WINDOW_ROWS.min(limit - result.rows)).unwrap();
            let page = scan_arrow(backend, snapshot, &read, &Cancellation::default()).await.unwrap();
            result.rows += page.batch.num_rows();
            result.sum += sum(&page.batch);
            result.bytes_read += page.bytes_read;
            result.payload_rows += page.payload_rows;
            if result.rows == limit || page.complete {
                break;
            }
            read.after = page.next;
        }
        result
    })
}

/* === Private Functions === */

fn verify_object(runtime: &tokio::runtime::Runtime, backend: &dyn StorageBackend, cut: &StorageCursor, expected: &ObjectEntry) {
    let snapshot = runtime.block_on(backend.open_snapshot(cut)).unwrap();
    let point = ObjectPoint {
        schema: expected.value.schema.id.clone(),
        item: expected.value.item.clone(),
        version: ObjectVersion::Latest,
    };
    let actual = runtime.block_on(backend.read_object(&snapshot, &point)).unwrap().unwrap();
    assert_eq!(actual, *expected);
    runtime.block_on(backend.release_snapshot(&snapshot)).unwrap();
}

fn expected_sum(offset: usize, limit: usize) -> f64 {
    (offset..offset + limit).map(|index| sample(index).value).sum()
}

fn verify_scan(workload: &Workload, offset: usize, limit: usize, result: &ScanResult) {
    assert!(offset + limit <= workload.rows);
    assert_eq!(result.rows, limit);
    assert_eq!(result.sum, expected_sum(offset, limit));
}

fn sum(batch: &RecordBatch) -> f64 {
    batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values().iter().sum()
}

fn verify_buckets(
    runtime: &tokio::runtime::Runtime, backend: &dyn StorageBackend, workload: &Workload, cut: &StorageCursor, corrected: bool,
) {
    let mut expected = BTreeMap::<Vec<u8>, (u64, f64)>::new();
    for index in 0..workload.rows {
        let mut value = sample(index);
        if corrected && index < WINDOW_ROWS {
            value.time_ns -= 60 * 1_000_000_000;
            value.value += 1.0;
        }
        let bucket = value.time_ns.div_euclid(60 * 1_000_000_000);
        let key = encode_key(&[ScalarValue::Unsigned(u64::from(value.plot)), ScalarValue::Signed(bucket)]).unwrap();
        let entry = expected.entry(key).or_default();
        entry.0 += 1;
        entry.1 += value.value;
    }
    let mut read = ProjectionRead {
        name: workload.buckets().name,
        revision: 1,
        frontier: cut.clone(),
        after: 0,
        limits: workload.page(),
        work: workload.work(),
    };
    let mut actual = BTreeMap::new();
    let mut changed = 0;
    loop {
        let page = runtime.block_on(backend.read_projection(&read)).unwrap();
        let keys = page.batch.column_by_name("key").unwrap().as_any().downcast_ref::<BinaryArray>().unwrap();
        let counts = page.batch.column_by_name("count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
        let sums = page.batch.column_by_name("sum").unwrap().as_any().downcast_ref::<Float64Array>().unwrap();
        for row in 0..page.batch.num_rows() {
            assert!(actual.insert(keys.value(row).to_vec(), (counts.value(row), sums.value(row))).is_none());
        }
        changed += page.changed_keys.len();
        let Some(next) = page.next else { break };
        read.after = next;
    }
    assert_eq!(actual, expected);
    assert!(changed > 0);
}

/* === Tests === */
