/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Apply Record Windows And Temporal Sampling Through The In-Memory Adapter

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroUsize};

// Third Party Imports
use arrow_array::{Array, Int64Array, RecordBatch, UInt64Array};

// Local Imports
use super::manager::fixture::{self, Fixture};
use crate::clock;
use harmos_domain::model::storage::query::{Query, RecordSelection, RowSlice, Source, StreamSelection, TimeMerge, TimeMergeMode};
use harmos_domain::model::storage::{PayloadEncoding, ReadLimits, RecordInput, SchemaKey, StreamInput};
use harmos_domain::schema::{
    Cardinality,
    graph::{Field, Kind, Schema},
};
use harmos_domain::{Aggregate, Identifier, Message};
use harmos_query::{
    QueryArrow,
    transform::{
        FieldPath, Transform,
        aggregate::{Aggregate as Reduction, Measure},
        project::{Project, Projection},
    },
};
use harmos_storage::StorageManager;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone)]
struct Reading {
    time: i64,
    end: i64,
    value: i64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn setup(records: &[Reading], primary: &[Reading], samples: &[Reading]) -> Fixture {
    let clock = clock::Controlled::new();
    let mut settings = fixture::config();
    settings.response.batch_rows = NonZeroUsize::MIN;
    settings.request.max_active_queries = NonZeroUsize::MIN;
    let fixture =
        Fixture::build_with(settings, |backends, settings| StorageManager::with_execution(backends, settings, clock.source()).unwrap());
    fixture::start(&fixture.storage).await;
    let definition = Schema {
        root: 0,
        representations: Default::default(),
        definitions: vec![vec![
            Field {
                name: "time".into(),
                tag: 1,
                kind: Kind::I64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: "end".into(),
                tag: 2,
                kind: Kind::I64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: "value".into(),
                tag: 3,
                kind: Kind::I64,
                cardinality: Cardinality::Required,
            },
        ]],
    };
    fixture.storage.write_schema(schema(), definition).await.unwrap();
    if !records.is_empty() {
        fixture.storage.append_records(records.iter().map(|value| input("markers", value)).collect()).await.unwrap();
    }
    for (name, values) in [("primary", primary), ("samples", samples)] {
        if values.is_empty() {
            continue;
        }
        let rows = values
            .iter()
            .enumerate()
            .map(|(sequence, value)| StreamInput {
                record: input(name, value),
                stream: Identifier::new(name).unwrap(),
                sequence: sequence as u64,
            })
            .collect();
        fixture.storage.append_samples(rows).await.unwrap();
    }
    fixture.storage.flush().await.unwrap();
    fixture
}

fn schema() -> SchemaKey {
    SchemaKey {
        id: Identifier::new("temporal.reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    }
}

fn input(item: &str, value: &Reading) -> RecordInput {
    RecordInput {
        schema: schema(),
        scope: None,
        item: Identifier::new(item).unwrap(),
        source: None,
        source_time: None,
        payload: value.encode(),
    }
}

fn stream(name: &str) -> Source {
    Source::Stream(StreamSelection {
        schema: schema(),
        scope: None,
        item: Some(Identifier::new(name).unwrap()),
        stream: Some(Identifier::new(name).unwrap()),
        source_start: None,
        source_end: None,
        after: 0,
    })
}

fn records() -> Source {
    Source::Records(RecordSelection {
        schema: schema(),
        scope: None,
        item: Some(Identifier::new("markers").unwrap()),
        source_start: None,
        source_end: None,
    })
}

fn projection() -> Transform {
    Transform::Project(Project {
        fields: vec![
            Projection {
                field: FieldPath::from("left.payload.value"),
                name: "marker".into(),
            },
            Projection {
                field: FieldPath::from("right.payload.value"),
                name: "value".into(),
            },
        ],
    })
}

async fn read(fixture: &Fixture, source: Source, mode: TimeMergeMode, transforms: Vec<Transform>) -> Vec<RecordBatch> {
    let query = Query {
        at: fixture.storage.get_saved_position(),
        source,
        merge: Some(TimeMerge {
            source: stream("samples"),
            primary_time: FieldPath::from("payload.time"),
            secondary_time: FieldPath::from("payload.time"),
            mode,
            work: ReadLimits {
                rows: NonZeroUsize::new(100).unwrap(),
                bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
            },
        }),
        transforms,
        slice: RowSlice::default(),
    };
    let mut reader = fixture.storage.query(query.clone()).await.unwrap();
    let mut batches = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        if let QueryArrow::Batch(batch) = output {
            batches.push(batch.as_record_batch().clone());
        }
    }
    assert_eq!(reader.completion().unwrap().rows as usize, batches.iter().map(RecordBatch::num_rows).sum::<usize>());
    drop(reader);
    let mut engine = harmos_query::QueryReader::open(
        &fixture.storage,
        query,
        harmos_query::batches::BatchScan {
            limits: ReadLimits {
                rows: NonZeroUsize::new(2).unwrap(),
                bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
            },
            projection: None,
            pruning: None,
        },
        harmos_query::QueryResponse {
            batch_rows: NonZeroUsize::MIN,
            batch_bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
        },
    )
    .await
    .unwrap();
    let mut direct = Vec::new();
    while let Some(output) = engine.next().await.unwrap() {
        if let QueryArrow::Batch(batch) = output {
            direct.push(batch.as_record_batch().clone());
        }
    }
    assert_eq!(direct, batches);
    drop(engine);
    batches
}

fn values(batches: &[RecordBatch], name: &str) -> Vec<Option<i64>> {
    batches
        .iter()
        .flat_map(|batch| batch.column_by_name(name).unwrap().as_any().downcast_ref::<Int64Array>().unwrap().iter().collect::<Vec<_>>())
        .collect()
}

fn reading(time: i64, end: i64, value: i64) -> Reading {
    Reading { time, end, value }
}

/* === Tests === */

#[tokio::test]
async fn record_ranges() {
    let fixture = setup(
        &[reading(2, 6, 10), reading(4, 8, 20)],
        &[],
        &[reading(1, 0, 10), reading(2, 0, 20), reading(4, 0, 40), reading(6, 0, 60), reading(8, 0, 80)],
    )
    .await;
    let batches = read(&fixture, records(), TimeMergeMode::Range { end: FieldPath::from("payload.end") }, vec![projection()]).await;
    assert_eq!(values(&batches, "marker"), vec![Some(10), Some(10), Some(20), Some(20)]);
    assert_eq!(values(&batches, "value"), vec![Some(20), Some(40), Some(40), Some(60)]);
    fixture.finish().await;
}

#[tokio::test]
async fn record_sampling() {
    let fixture = setup(
        &[reading(0, 0, 0), reading(4, 0, 4), reading(5, 0, 5), reading(6, 0, 6), reading(9, 0, 9)],
        &[],
        &[reading(2, 0, 20), reading(4, 0, 40), reading(4, 0, 41), reading(8, 0, 80)],
    )
    .await;
    let batches = read(&fixture, records(), TimeMergeMode::AtOrBefore, vec![projection()]).await;
    assert_eq!(values(&batches, "marker"), vec![Some(0), Some(4), Some(5), Some(6), Some(9)]);
    assert_eq!(values(&batches, "value"), vec![None, Some(41), Some(41), Some(41), Some(80)]);
    fixture.finish().await;
}

#[tokio::test]
async fn record_intervals() {
    let fixture = setup(
        &[reading(2, 0, 100), reading(5, 0, 200), reading(7, 0, 300)],
        &[],
        &[reading(1, 0, 10), reading(2, 0, 20), reading(4, 0, 40), reading(6, 0, 60), reading(8, 0, 80)],
    )
    .await;
    let aggregate = Transform::Aggregate(Reduction {
        groups: vec![Projection {
            field: FieldPath::from("left.payload.value"),
            name: "marker".into(),
        }],
        measures: vec![Measure {
            field: FieldPath::from("right.payload.value"),
            statistic: Aggregate::Count,
            name: "count".into(),
        }],
    });
    let batches = read(&fixture, records(), TimeMergeMode::Intervals, vec![aggregate]).await;
    assert_eq!(values(&batches, "marker"), vec![Some(100), Some(200), Some(300)]);
    let counts = batches
        .iter()
        .flat_map(|batch| batch.column_by_name("count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap().values().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(counts, vec![2, 1, 1]);
    fixture.finish().await;
}

#[tokio::test]
async fn stream_sampling() {
    let fixture = setup(
        &[],
        &[reading(1, 0, 100), reading(3, 0, 300), reading(8, 0, 800)],
        &[reading(2, 0, 20), reading(3, 0, 30), reading(7, 0, 70)],
    )
    .await;
    let batches = read(&fixture, stream("primary"), TimeMergeMode::AtOrBefore, vec![projection()]).await;
    assert_eq!(values(&batches, "marker"), vec![Some(100), Some(300), Some(800)]);
    assert_eq!(values(&batches, "value"), vec![None, Some(30), Some(70)]);
    fixture.finish().await;
}
