/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use criterion::Criterion;

// Local Imports
use harmos_query::Accumulator;
use harmos_query::TableWriter;
use harmos_query::transform::FieldPath;
use harmos_query::transform::Transform;
use harmos_query::transform::downsample::Algorithm;
use harmos_query::transform::downsample::Downsample;
use harmos_query::transform::sort::Sort;
use harmos_query::transform::sort::SortKey;
use harmos_storage::query::scalar::Agg;
use harmos_storage::query::scalar::ScalarValue;
use harmos_storage::{Aggregate, QueryAccumulatorSettings};

/* === Public Functions === */

pub fn benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("integration/spill_workloads");
    group.sample_size(10);
    group.bench_function("merge_33768_rows_from_37_row_pages", |b| b.iter(merge_runs));
    group.bench_function("seven_days_of_hourly_pages", |b| b.iter(hourly_pages));
    group.bench_function("seven_days_of_percentiles", |b| b.iter(percentiles));
    group.bench_function("seven_days_of_downsampling", |b| b.iter(downsampling));
    group.finish();
}

/* === Private Functions === */

fn merge_runs() {
    let rows = 33_768;
    let schema = Arc::new(Schema::new(vec![Field::new("key", DataType::UInt64, false), Field::new("ordinal", DataType::UInt64, false)]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from((0..rows as u64).map(|row| row * 7_919 % 13).collect::<Vec<_>>())),
            Arc::new(UInt64Array::from((0..rows as u64).collect::<Vec<_>>())),
        ],
    )
    .unwrap();
    let mut writer = TableWriter::new(schema).unwrap();
    for offset in (0..rows).step_by(37) {
        writer.push(&batch.slice(offset, 37.min(rows - offset)), &mut || Ok(())).unwrap();
    }
    let table = writer.finish().unwrap();
    let sort = Transform::Sort(Sort {
        keys: vec![SortKey {
            field: FieldPath::from("key"),
            descending: true,
            nulls_first: false,
        }],
    });
    let sorted = table.transform(&sort, &mut || Ok(())).unwrap();
    let mut reader = sorted.reader().unwrap();
    while let Some(page) = reader.next(&mut || Ok(())).unwrap() {
        black_box(page);
    }
}

fn hourly_pages() {
    let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::UInt64, false)]));
    let page = RecordBatch::try_new(schema.clone(), vec![Arc::new(UInt64Array::from((0..20_000).collect::<Vec<u64>>()))]).unwrap();
    let mut writer = TableWriter::new(schema).unwrap();
    for _hour in 0..7 * 24 {
        writer.push(&page, &mut || Ok(())).unwrap();
    }
    let table = writer.finish().unwrap();
    let mut reader = table.reader().unwrap();
    drop(table);
    while let Some(page) = reader.next(&mut || Ok(())).unwrap() {
        black_box(page);
    }
}

fn percentiles() {
    let values = (0..7 * 24 * 60).map(|i| ((i * 19 % 101) as f64 - 50.0).sin()).collect::<Vec<_>>();
    for rank in [0, 13, 50, 77, 100] {
        let mut accumulator =
            Accumulator::with_settings(Agg::Of(Aggregate::Percentile(rank)), QueryAccumulatorSettings { memory_bytes: 0 });
        for (index, value) in values.iter().enumerate() {
            accumulator.push(index as u64, &ScalarValue::Float(*value)).unwrap();
        }
        black_box(accumulator.finish(&mut || Ok(())).unwrap());
    }
}

fn downsampling() {
    let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false), Field::new("y", DataType::Float64, false)]));
    let x = (1..=10_080).map(f64::from).collect::<Vec<_>>();
    let y = x.iter().map(|value| (value * 0.13).sin()).collect::<Vec<_>>();
    let mut writer = TableWriter::new(schema.clone()).unwrap();
    for (x, y) in x.chunks(120).zip(y.chunks(120)) {
        let page =
            RecordBatch::try_new(schema.clone(), vec![Arc::new(Float64Array::from(x.to_vec())), Arc::new(Float64Array::from(y.to_vec()))])
                .unwrap();
        writer.push(&page, &mut || Ok(())).unwrap();
    }
    let table = writer.finish().unwrap();
    for algorithm in [Algorithm::Decimate, Algorithm::Extrema, Algorithm::Lttb, Algorithm::LttbLog] {
        let transform = Transform::Downsample(Downsample {
            algorithm,
            max_rows: 40,
            value: Some(FieldPath::from("y")),
            axis: Some(FieldPath::from("x")),
        });
        let reduced = table.transform(&transform, &mut || Ok(())).unwrap();
        let mut reader = reduced.reader().unwrap();
        while let Some(page) = reader.next(&mut || Ok(())).unwrap() {
            black_box(page);
        }
    }
}
