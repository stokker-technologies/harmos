/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read Bounded Record Pages

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::Ordering;

// Third Party Imports
use arrow_array::Float64Array;

// Local Imports
use crate::manager::{backend, mapping_fixture::Mapped};
use harmos_storage::query::{
    QueryArrow, RowSlice,
    transform::{
        FieldPath, Transform,
        project::{Project, Projection},
    },
};
use harmos_storage::{bench_support::workload::sample, *};

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
async fn record_pages_seek_without_copying_history() {
    // Save A History Larger Than The Requested Page.
    let values = (0..257).map(sample).collect::<Vec<_>>();
    let mapped = Mapped::new(backend::storage_settings(), "records", &values).await;
    let mut query = mapped.query(None);
    query.transforms.push(Transform::Project(Project {
        fields: vec![Projection {
            field: FieldPath::from("payload.value"),
            name: "value".into(),
        }],
    }));
    query.slice = RowSlice { offset: 200, limit: Some(3) };
    mapped.data.control.snapshot_rows_copied.store(0, Ordering::Relaxed);
    mapped.data.control.record_rows_examined.store(0, Ordering::Relaxed);
    mapped.data.control.record_rows_copied.store(0, Ordering::Relaxed);
    let mut reader = mapped.storage.query(query.clone()).await.unwrap();
    assert_eq!(mapped.data.control.snapshot_rows_copied.load(Ordering::Relaxed), 0);

    // Append After Opening The Saved Image And Read Its Exact Deep Page.
    let harmos_storage::query::Source::Records(selection) = &query.source else { unreachable!() };
    mapped
        .storage
        .write_records(vec![RecordInput {
            schema: selection.schema.clone(),
            scope: None,
            item: Identifier::new("appended").unwrap(),
            source: None,
            source_time: None,
            payload: sample(257).encode(),
        }])
        .await
        .unwrap();
    mapped.storage.flush().await.unwrap();
    let mut actual = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        if let QueryArrow::Batch(batch) = output {
            actual.extend_from_slice(batch.as_record_batch().column(0).as_any().downcast_ref::<Float64Array>().unwrap().values());
        }
    }
    assert_eq!(actual, values[200..203].iter().map(|value| value.value).collect::<Vec<_>>());
    assert_eq!(reader.completion().unwrap().rows, 3);
    assert!(mapped.data.control.record_rows_examined.load(Ordering::Relaxed) <= actual.len() + 1);
    assert_eq!(mapped.data.control.record_rows_copied.load(Ordering::Relaxed), 0);
    drop(reader);

    // An Offset Beyond The Saved Image Completes Without Reading Payload Rows.
    query.slice = RowSlice {
        offset: values.len() as u64 + 10,
        limit: Some(3),
    };
    mapped.data.control.record_rows_examined.store(0, Ordering::Relaxed);
    let mut reader = mapped.storage.query(query).await.unwrap();
    while let Some(output) = reader.next().await.unwrap() {
        if let QueryArrow::Batch(batch) = output {
            assert_eq!(batch.as_record_batch().num_rows(), 0);
        }
    }
    assert_eq!(reader.completion().unwrap().rows, 0);
    assert_eq!(mapped.data.control.record_rows_examined.load(Ordering::Relaxed), 0);
    drop(reader);
    mapped.finish().await;
}
