/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../tests/support/storage.rs"]
#[allow(dead_code)]
mod backend;
#[path = "../../tests/support/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "../../tests/support/mapping.rs"]
mod mapping_fixture;

mod native;
use harmos_storage::bench_support::workload;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::num::NonZeroUsize;

// Third Party Imports
use criterion::Criterion;

// Local Imports
use arrow_array::Float64Array;
use harmos_query::transform::FieldPath;
use harmos_query::transform::Transform;
use harmos_query::transform::project::Project;
use harmos_query::transform::project::Projection;
use harmos_query::QueryArrow;
use harmos_storage::query::RowSlice;
use mapping_fixture::Mapped;
use workload::ROWS;

#[global_allocator]
static ALLOCATOR: harmos_storage::bench_support::TrackingAllocator = harmos_storage::bench_support::TrackingAllocator;

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
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut criterion = Criterion::default().configure_from_args();
    if std::env::var_os("HARMOS_STORAGE_NATIVE").is_some() {
        native::run(&runtime);
        criterion.final_summary();
        return;
    }
    let samples = (0..ROWS).map(workload::sample).collect::<Vec<_>>();
    let mut settings = backend::storage_settings();
    settings.response.batch_rows = NonZeroUsize::new(16384).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(8 * 1024 * 1024).unwrap();
    println!("Preparing {ROWS} records across three days and 24 plots");
    let mapped = runtime.block_on(Mapped::new(settings, "multi-day", &samples));
    println!("Workload Ready");
    let mut group = criterion.benchmark_group("integration/multi_day/baseline");
    for (name, offset, limit) in [("full", 0, ROWS), ("first_window", ROWS / 3, 4096), ("next_window", ROWS / 3 + 4096, 4096)] {
        mapped.data.control.record_rows_examined.store(0, std::sync::atomic::Ordering::Relaxed);
        mapped.data.control.record_rows_copied.store(0, std::sync::atomic::Ordering::Relaxed);
        mapped.data.control.snapshot_rows_copied.store(0, std::sync::atomic::Ordering::Relaxed);
        let start = std::time::Instant::now();
        let expected = scan(&runtime, &mapped, offset, limit);
        let elapsed = start.elapsed();
        println!("baseline/{name}: {} rows in {:.6} seconds", expected.0, elapsed.as_secs_f64());
        println!(
            "STORAGE_PROFILE baseline/{name} examined={} snapshot_copies={} record_copies={}",
            mapped.data.control.record_rows_examined.load(std::sync::atomic::Ordering::Relaxed),
            mapped.data.control.snapshot_rows_copied.load(std::sync::atomic::Ordering::Relaxed),
            mapped.data.control.record_rows_copied.load(std::sync::atomic::Ordering::Relaxed)
        );
        assert_eq!(expected.0, limit);
        let sum = samples[offset..offset + limit].iter().map(|sample| sample.value).sum::<f64>();
        assert_eq!(expected.1, sum);
        if std::env::var_os("HARMOS_STORAGE_BASELINE_ONCE").is_none() {
            group.bench_function(name, |b| b.iter(|| black_box(scan(&runtime, &mapped, offset, limit))));
        }
    }
    group.finish();
    runtime.block_on(mapped.finish());
    criterion.final_summary();
}

fn scan(runtime: &tokio::runtime::Runtime, mapped: &Mapped, offset: usize, limit: usize) -> (usize, f64) {
    runtime.block_on(async {
        let mut query = mapped.query(None);
        query.transforms.push(Transform::Project(Project {
            fields: vec![Projection {
                field: FieldPath::from("payload.value"),
                name: "value".into(),
            }],
        }));
        query.slice = RowSlice {
            offset: offset as u64,
            limit: Some(limit as u64),
        };
        let profile_start = std::time::Instant::now();
        let mut reader = mapped.storage.query(query).await.unwrap();
        let opened_s = profile_start.elapsed().as_secs_f64();
        let profile_start = std::time::Instant::now();
        let mut rows = 0;
        let mut sum = 0.0;
        while let Some(output) = reader.next().await.unwrap() {
            if let QueryArrow::Batch(batch) = output {
                let batch = black_box(batch.as_record_batch());
                rows += batch.num_rows();
                let values = batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
                sum += values.values().iter().sum::<f64>();
            }
        }
        println!("STORAGE_PROFILE baseline/query open_s={opened_s:.6} consume_s={:.6}", profile_start.elapsed().as_secs_f64());
        (rows, sum)
    })
}

/* === Tests === */
