/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Arrow Storage Round Trip Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
use arrow_array::{Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use harmos::storage::arrow::{ArrowBatch, ArrowSchema, decoding_memory};
use harmos::storage::query::transform::{
    FieldPath, Transform,
    project::{Project, Projection},
    sort::{Sort, SortKey},
};
use harmos::storage::query::{RowSlice, execute::Pipeline};
use harmos::storage::{StorageBackends, StorageManager};
use harmos::{MemorySize, Runtime};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[tokio::test]
async fn arrow() {
    // Start Storage With The Explicit Fixture Budget.
    let directory = tempfile::tempdir().unwrap();
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage = StorageManager::new(
        StorageBackends {
            primary: data,
            state: Arc::new(Recovery::new(history())),
            replicas: vec![],
        },
        storage_settings(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();

    // Native Buffers And Self-Contained Wire Values Agree.
    let schema = Arc::new(Schema::new(vec![
        Field::new("key", DataType::Int64, false),
        Field::new("row", DataType::Int64, false),
    ]));
    let original = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![2, 1, 1, 2])),
            Arc::new(Int64Array::from(vec![0, 1, 2, 3])),
        ],
    )
    .unwrap();
    let encoded = ArrowBatch::try_new(original.clone()).unwrap();
    let bound = decoding_memory(encoded.ipc_bytes(), true).unwrap();
    let decoded = ArrowBatch::from_ipc(encoded.ipc_bytes()).unwrap();
    assert_eq!(decoded.as_record_batch(), &original);
    assert!(bound >= decoded.memory_size());
    let header = ArrowSchema::try_new(schema).unwrap();
    let decoded_header = ArrowSchema::from_ipc(header.ipc_bytes()).unwrap();
    decoded.validate_schema(decoded_header.as_schema()).unwrap();
    assert!(ArrowSchema::from_ipc(encoded.ipc_bytes()).is_err());
    assert!(ArrowBatch::from_ipc(header.ipc_bytes()).is_err());

    // Malformed Framing Is Refused Before Arrow Allocates Arrays.
    for length in [0, 1, 4, encoded.ipc_bytes().len() - 1] {
        assert!(decoding_memory(&encoded.ipc_bytes()[..length], true).is_err());
    }
    let mut trailing = encoded.ipc_bytes().to_vec();
    trailing.extend_from_slice(&[0, 0, 0, 0]);
    assert!(decoding_memory(&trailing, true).is_err());
    let unsupported = Schema::new(vec![Field::new("value", DataType::LargeUtf8, false)]);
    assert!(ArrowSchema::try_new(Arc::new(unsupported)).is_err());
    let duplicate = Schema::new(vec![
        Field::new("x", DataType::Int64, false),
        Field::new("x", DataType::Int64, false),
    ]);
    assert!(ArrowSchema::try_new(Arc::new(duplicate)).is_err());
    drop(decoded_header);
    drop(header);
    drop(decoded);
    drop(encoded);

    // Stable Ties Precede Output Slicing, With Admission Through The Real Pool.
    let pipeline = Pipeline::new(
        original.schema(),
        vec![
            Transform::Sort(Sort {
                keys: vec![SortKey {
                    field: FieldPath::from("key"),
                    descending: false,
                    nulls_first: false,
                }],
            }),
            Transform::Project(Project {
                fields: vec![Projection {
                    field: FieldPath::from("row"),
                    name: "selected".into(),
                }],
            }),
        ],
        RowSlice {
            offset: 1,
            limit: Some(2),
        },
    )
    .unwrap();
    let baseline = storage.status().read_bytes;
    let output = pipeline
        .execute(&[original], &mut || Ok(()), &mut |bytes| {
            storage.reserve_arrow_memory(bytes)
        })
        .unwrap();
    let values = output
        .as_record_batch()
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(values.values().as_ref(), &[2, 0]);

    // A Consumer's Raw Zero-Copy Clone And Slice Keep The Reservation Alive.
    let retained = output.as_record_batch().clone();
    let slice = retained.column(0).slice(1, 1);
    drop(output);
    drop(retained);
    assert!(storage.status().read_bytes > baseline);
    assert_eq!(
        slice
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .value(0),
        0
    );
    drop(slice);
    assert_eq!(storage.status().read_bytes, baseline);

    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
