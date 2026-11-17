/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure 128 Streams in 100 ms Columnar Windows Through Durable Completion.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::{Duration, Instant},
};

// Third Party Imports
use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray, StructArray, UInt64Array};
use arrow_schema::{self, DataType};
use tokio::{sync::Mutex, task::JoinHandle};

// Local Imports
use harmos_domain::schema::{Cardinality, graph};
use harmos_query::{arrow::ArrowBatch, source};
use harmos_storage::writes::DecodedBatch;
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Task(Mutex<Option<JoinHandle<Result<()>>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WorkerOwner for Task {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if let Some(task) = self.0.lock().await.take() {
                task.await.map_err(|source| Error::OperationFailed {
                    operation: "join columnar benchmark worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run());
}

async fn run() {
    // Open the Reference Backend.
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_millis(100),
            max_entries: nz(100_000),
            event_capacity: nz(1024),
        },
        request: StorageRequestSettings {
            max_file_productions: nz(256),
            max_mutation_receipts: nz(2048),
            max_pending_queries: nz(64),
            max_active_queries: nz(4),
            max_pending_write_batches: nz(256),
            max_active_file_operations: nz(1),
            max_pending_write_bytes: nz(16 * 1024 * 1024),
        },
        response: StorageResponseSettings {
            batch_rows: nz(1024),
            batch_bytes: nz(64 * 1024),
        },
    };
    let backends = StorageBackends::in_memory(ObjectCatalog::new(), execution::Execution::default()).unwrap();
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

    // Register the Sample Schema.
    let key = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = graph::Schema {
        representations: BTreeMap::new(),
        root: 0,
        definitions: vec![vec![graph::Field {
            name: "value".into(),
            tag: 1,
            kind: graph::Kind::F64,
            cardinality: Cardinality::Required,
        }]],
    };
    storage.write_schema(key.clone(), graph).await.unwrap();
    storage.flush().await.unwrap();
    let schema = storage.lookup_schema(&key).await.unwrap();
    let scope = Identifier::new("execution").unwrap();

    // Prepare One Columnar IPC Batch per Stream Window, Without Row Payloads.
    let payload = StructArray::try_new(
        source::payload_fields(&schema.schema).unwrap(),
        vec![Arc::new(Float64Array::from_iter_values((0..100).map(|row| row as f64)))],
        None,
    )
    .unwrap();
    let mut batches = Vec::new();
    let window = storage.mutation_window().unwrap().id;
    for index in 0..10 {
        for stream in 0..128 {
            let destination = Identifier::new(format!("stream.{stream}")).unwrap();
            let metadata = StructArray::from(vec![
                (
                    Arc::new(arrow_schema::Field::new("item", DataType::Utf8, false)),
                    Arc::new(StringArray::from(vec!["execution"; 100])) as ArrayRef,
                ),
                (
                    Arc::new(arrow_schema::Field::new("scope", DataType::Utf8, false)),
                    Arc::new(StringArray::from(vec![scope.as_str(); 100])) as ArrayRef,
                ),
                (
                    Arc::new(arrow_schema::Field::new("stream", DataType::Utf8, false)),
                    Arc::new(StringArray::from(vec![destination.as_str(); 100])) as ArrayRef,
                ),
                (
                    Arc::new(arrow_schema::Field::new("sequence", DataType::UInt64, false)),
                    Arc::new(UInt64Array::from_iter_values((index * 100 + 1)..=(index * 100 + 100))) as ArrayRef,
                ),
            ]);
            let batch = RecordBatch::try_from_iter(vec![
                ("_storage", Arc::new(metadata) as ArrayRef),
                ("payload", Arc::new(payload.clone()) as ArrayRef),
            ])
            .unwrap();
            let data = ArrowBatch::try_new(batch).unwrap().into();
            let batch = writes::DataBatch {
                sequence: index + 1,
                schema: key.clone(),
                destination,
                data,
            };
            let mutation = MutationId {
                window: window.clone(),
                key: Identifier::new(format!("window.{index}.{stream}")).unwrap(),
            };
            batches.push((mutation, batch));
        }
    }

    // Measure IPC Decode, Admission, Fingerprinting and Durable Backend Completion.
    let started = Instant::now();
    for (mutation, batch) in batches {
        let batch = DecodedBatch::of(batch).unwrap();
        storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap();
    }
    storage.flush().await.unwrap();
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "STORAGE_COLUMNAR_APPEND {}",
        serde_json::json!({
            "backend": "memory", "streams": 128, "stream_hz": 1000, "window_ms": 100, "batches": 1280,
            "samples": 128000, "seconds": seconds, "samples_per_second": 128000.0 / seconds,
        })
    );
    storage.close().await.unwrap();
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
