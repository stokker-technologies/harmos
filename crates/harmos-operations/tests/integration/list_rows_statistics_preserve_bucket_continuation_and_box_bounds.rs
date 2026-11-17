/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// ListRows Statistics Through Native Storage Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports
use arrow_array::{BinaryArray, Float64Array, UInt64Array};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Local Imports
use harmos_domain::model::storage::query::transform::aggregate;
use harmos_domain::model::storage::query::transform::{
    Transform,
    statistics::{BoxSummary, BucketSummary},
};
use harmos_domain::model::storage::query::{Source, StreamSelection};
use harmos_domain::schema::graph::{Field, Kind};
use harmos_domain::schema::{self, Cardinality, graph};
use harmos_domain::{Aggregate, Identifier, Message};
use harmos_operations::execution::operations::{ExecutionContext, NativeOperation};
use harmos_operations::storage::SavedSnapshots;
use harmos_operations::storage::{ListRows, ListRowsRequest};
use harmos_query::QueryOutput;
use harmos_query::arrow::ArrowBatch;
use harmos_storage::{
    ObjectCatalog, PayloadEncoding, RecordInput, SchemaKey, StorageBackends, StorageFilesystemSettings, StorageFuture, StorageManager,
    StorageRequestSettings, StorageResponseSettings, StorageSettings, StreamInput, WorkerOwner,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
#[harmos_path(::harmos_domain)]
struct Sample {
    time_ns: i64,
    value: f64,
}

struct Task(Mutex<Option<JoinHandle<harmos_storage::Result<()>>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WorkerOwner for Task {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if let Some(task) = self.0.lock().await.take() {
                task.await.map_err(|source| harmos_storage::Error::OperationFailed {
                    operation: "join statistics storage worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn fixture() -> (StorageManager, ExecutionContext, SchemaKey) {
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_millis(100),
            max_entries: NonZeroUsize::new(1000).unwrap(),
            event_capacity: NonZeroUsize::new(32).unwrap(),
        },
        request: StorageRequestSettings {
            max_file_productions: NonZeroUsize::new(4).unwrap(),
            max_mutation_receipts: NonZeroUsize::new(64).unwrap(),
            max_pending_queries: NonZeroUsize::new(8).unwrap(),
            max_active_queries: NonZeroUsize::new(4).unwrap(),
            max_pending_write_batches: NonZeroUsize::new(8).unwrap(),
            max_active_file_operations: NonZeroUsize::new(1).unwrap(),
            max_pending_write_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
        response: StorageResponseSettings {
            batch_rows: NonZeroUsize::new(3).unwrap(),
            batch_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
        },
    };
    let backends = StorageBackends::in_memory(ObjectCatalog::new(), Default::default()).unwrap();
    let storage = StorageManager::new(backends, settings).unwrap();
    storage
        .start_with(Arc::new(()), |mut worker| {
            let task = tokio::spawn(async move {
                worker.run().await?;
                worker.cleanup().await
            });
            Ok(Box::new(Task(Mutex::new(Some(task)))))
        })
        .await
        .unwrap();
    let key = SchemaKey {
        id: Identifier::new("statistics.sample").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let definition = graph::Schema {
        root: 0,
        representations: Default::default(),
        definitions: vec![vec![
            Field {
                name: "time_ns".into(),
                tag: schema::field_tag("time_ns"),
                kind: Kind::I64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: "value".into(),
                tag: schema::field_tag("value"),
                kind: Kind::F64,
                cardinality: Cardinality::Required,
            },
        ]],
    };
    storage.write_schema(key.clone(), definition).await.unwrap();
    let mut resources = ExecutionContext::default().resources().clone();
    resources.insert(Arc::new(storage.clone())).unwrap();
    resources.insert(Arc::new(SavedSnapshots::new(NonZeroUsize::new(8).unwrap()))).unwrap();
    let context = ExecutionContext::default().with_resources(resources);
    (storage, context, key)
}

async fn append(storage: &StorageManager, key: &SchemaKey, start: u64, end: u64) {
    let rows = (start..end)
        .map(|index| StreamInput {
            record: RecordInput {
                schema: key.clone(),
                scope: None,
                item: Identifier::new("signal").unwrap(),
                source: None,
                source_time: None,
                payload: Sample {
                    time_ns: index as i64,
                    value: if index == 0 { 100.0 } else { (index % 3) as f64 },
                }
                .encode(),
            },
            stream: Identifier::new("signal").unwrap(),
            sequence: index + 1,
        })
        .collect();
    storage.append_samples(rows).await.unwrap();
    storage.flush().await.unwrap();
}

fn source(key: &SchemaKey, after: u64) -> Source {
    Source::Stream(StreamSelection {
        schema: key.clone(),
        scope: None,
        item: None,
        stream: None,
        source_start: None,
        source_end: None,
        after,
    })
}

async fn read(context: &ExecutionContext, key: &SchemaKey, after: u64, transform: Transform) -> Vec<ArrowBatch> {
    let mut page = None;
    let mut batches = Vec::new();
    loop {
        let request = ListRowsRequest {
            source: Some(source(key, after)),
            at: None,
            transforms: vec![transform.clone()],
            merge: None,
            page,
        };
        let result = ListRows::run(context.clone(), request, &()).await.unwrap();
        for output in result.rows {
            if let QueryOutput::Batch(bytes) = output {
                batches.push(ArrowBatch::from_ipc(bytes).unwrap());
            }
        }
        if result.next_page_token.is_empty() {
            break;
        }
        page = Some(harmos_protocol::model::PageRequest {
            page_token: result.next_page_token,
            ..Default::default()
        });
    }
    batches
}

/* === Tests === */
#[tokio::test]
async fn list_rows_statistics_preserve_bucket_continuation_and_box_bounds() {
    // Prepare Storage
    let (storage, context, key) = fixture().await;

    // Capture The First Range
    append(&storage, &key, 0, 12).await;
    let cut = storage.get_saved_position();
    let plan = BucketSummary {
        time: "payload.time_ns".into(),
        position: "_storage.position".into(),
        value: "payload.value".into(),
        width_ns: NonZeroU64::new(1).unwrap(),
        max_buckets: NonZeroUsize::new(4).unwrap(),
        source_cut: cut,
        prior: None,
    };
    let first = read(&context, &key, 0, Transform::BucketSummary(plan.clone())).await;
    let state = first[0]
        .as_record_batch()
        .column_by_name("continuation")
        .unwrap()
        .as_any()
        .downcast_ref::<BinaryArray>()
        .unwrap()
        .value(0)
        .to_vec();
    // Continue The Envelope
    append(&storage, &key, 12, 24).await;
    let prior_sequence = plan.source_cut.sequence;
    let mut continued = plan;
    continued.source_cut = storage.get_saved_position();
    continued.prior = Some(state);
    let actual = read(&context, &key, prior_sequence, Transform::BucketSummary(continued.clone())).await;
    continued.prior = None;
    let whole = read(&context, &key, 0, Transform::BucketSummary(continued)).await;
    let counts = |batches: &[ArrowBatch]| {
        batches
            .iter()
            .flat_map(|batch| {
                batch.as_record_batch().column_by_name("count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap().values().to_vec()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(counts(&actual), counts(&whole));
    assert_eq!(counts(&actual).iter().sum::<u64>(), 24);
    assert!(actual.iter().map(|batch| batch.as_record_batch().num_rows()).sum::<usize>() <= 4);
    // Query A Box Summary
    let boxes = read(
        &context,
        &key,
        0,
        Transform::BoxSummary(BoxSummary {
            field: "payload.value".into(),
            max_outliers: 0,
            memory_bytes: 0,
            scratch_bytes: 65536,
        }),
    )
    .await;
    let batch = boxes[0].as_record_batch();
    assert_eq!(batch.column_by_name("outlier_count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap().value(0), 1);
    assert_eq!(batch.column_by_name("median").unwrap().as_any().downcast_ref::<Float64Array>().unwrap().value(0), 1.0);
    // Query An Empty Range
    let empty = read(
        &context,
        &key,
        storage.get_saved_position().sequence,
        Transform::Aggregate(aggregate::Aggregate {
            groups: vec![],
            measures: vec![aggregate::Measure {
                field: "payload.value".into(),
                statistic: Aggregate::Sum,
                name: "sum".into(),
            }],
        }),
    )
    .await;
    assert_eq!(empty[0].as_record_batch().column_by_name("sum").unwrap().as_any().downcast_ref::<Float64Array>().unwrap().value(0), 0.0);

    // Cancel A Read
    context.cancellation.request();
    let request = ListRowsRequest {
        source: Some(source(&key, 0)),
        at: None,
        transforms: vec![],
        merge: None,
        page: None,
    };
    assert!(ListRows::run(context, request, &()).await.is_err());
    // Close Storage
    storage.close().await.unwrap();
}
