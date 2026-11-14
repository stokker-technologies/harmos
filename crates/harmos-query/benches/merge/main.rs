/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Ordered Merge Over One Million Rows Per Input

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::VecDeque;
use std::hint::black_box;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports
use arrow_array::{Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tokio::runtime::Builder;

// Local Imports
use harmos_domain::Identifier;
use harmos_domain::model::storage::query::{Source, StreamSelection, TimeMerge, TimeMergeMode};
use harmos_domain::model::storage::{PayloadEncoding, ReadLimits, SchemaKey, StorageCursor, StoreHistory};
use harmos_query::batches::{BatchPage, BatchReader, QueryFuture};
use harmos_query::merge::TimeMergeReader;
use harmos_query::transform::FieldPath;

/* === Constants === */

const ROWS: usize = 1_000_000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Input {
    schema: SchemaRef,
    batches: VecDeque<RecordBatch>,
    at: StorageCursor,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Input {
    fn new(batch: RecordBatch) -> Self {
        Self {
            schema: batch.schema(),
            batches: VecDeque::from([batch]),
            at: StorageCursor {
                sequence: ROWS as u64,
                history: StoreHistory {
                    store: Identifier::new("merge-benchmark").unwrap(),
                    epoch: Identifier::new("run").unwrap(),
                },
            },
        }
    }
}

impl BatchReader for Input {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
    fn at(&self) -> &StorageCursor {
        &self.at
    }
    fn next(&mut self) -> QueryFuture<'_, BatchPage> {
        Box::pin(async move {
            Ok(BatchPage {
                batch: self.batches.pop_front().unwrap_or_else(|| RecordBatch::new_empty(Arc::clone(&self.schema))),
                complete: self.batches.is_empty(),
            })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn batches() -> (RecordBatch, RecordBatch) {
    let schema = Arc::new(Schema::new(vec![
        Field::new("time", DataType::Int64, false),
        Field::new("end", DataType::Int64, false),
        Field::new("value", DataType::Int64, false),
    ]));
    let primary = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|row| row as i64 * 2))),
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|row| row as i64 * 2 + 2))),
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|row| row as i64))),
        ],
    )
    .unwrap();
    let secondary = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|row| row as i64 * 2 + 1))),
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|_| 0))),
            Arc::new(Int64Array::from_iter_values((0..ROWS).map(|row| row as i64))),
        ],
    )
    .unwrap();
    (primary, secondary)
}

fn measure(criterion: &mut Criterion) {
    let runtime = Builder::new_current_thread().build().unwrap();
    let (primary, secondary) = batches();
    let mut group = criterion.benchmark_group("time_merge");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(ROWS as u64));
    for (name, mode) in [
        ("record_range", TimeMergeMode::Range { end: FieldPath::from("end") }),
        ("value_at_record", TimeMergeMode::AtOrBefore),
        ("record_intervals", TimeMergeMode::Intervals),
        ("stream_against_stream", TimeMergeMode::AtOrBefore),
    ] {
        let plan = TimeMerge {
            source: Source::Stream(StreamSelection {
                schema: SchemaKey {
                    id: Identifier::new("reading").unwrap(),
                    version: NonZeroU32::MIN,
                    payload_encoding: PayloadEncoding::PostcardV1,
                },
                scope: None,
                item: None,
                stream: None,
                source_start: None,
                source_end: None,
                after: 0,
            }),
            primary_time: FieldPath::from("time"),
            secondary_time: FieldPath::from("time"),
            mode,
            work: ReadLimits {
                rows: NonZeroUsize::new(ROWS).unwrap(),
                bytes: NonZeroUsize::new(128 * 1024 * 1024).unwrap(),
            },
        };
        group.bench_with_input(BenchmarkId::new(name, ROWS), &plan, |bench, plan| {
            bench.iter(|| {
                runtime.block_on(async {
                    let mut reader = TimeMergeReader::new(
                        Box::new(Input::new(primary.clone())),
                        Box::new(Input::new(secondary.clone())),
                        plan.clone(),
                        NonZeroUsize::new(16_384).unwrap(),
                    )
                    .unwrap();
                    let mut rows = 0usize;
                    loop {
                        let page = reader.next().await.unwrap();
                        rows += black_box(page.batch).num_rows();
                        if page.complete {
                            break;
                        }
                    }
                    black_box(rows)
                })
            });
        });
    }
    group.finish();
}

criterion_group!(benches, measure);
criterion_main!(benches);

/* === Tests === */
