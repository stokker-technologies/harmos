/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
/* === Imports === */

use super::mapping_fixture::*;

/* === Tests === */

#[tokio::test]
async fn seeded_projection_matches_stored_values_and_order() {
    // Store Enough Rows To Cross A Write Page
    let rows = 4099;

    use harmos_storage::query::transform::{
        FieldPath, Transform,
        project::{Project, Projection},
    };
    let mut seed = 0x1234_5678_u64;
    let payloads = (0..rows)
        .map(|row| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            PlotSample {
                x: row as f64,
                y: (seed >> 11) as f64 / (1_u64 << 53) as f64,
            }
        })
        .collect::<Vec<_>>();

    let mut settings = storage_settings();
    settings.response.batch_rows = NonZeroUsize::new(16_384).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(8 * 1024 * 1024).unwrap();
    let mapped = Mapped::new(settings, "plot-sample", &payloads).await;

    for (_name, offset, limit) in [("full", 0, rows), ("window", rows / 3, rows.min(4096))] {
        let mut query = mapped.query(None);
        query.transforms.push(Transform::Project(Project {
            fields: vec![
                Projection {
                    field: FieldPath::from("payload.x"),
                    name: "x".into(),
                },
                Projection {
                    field: FieldPath::from("payload.y"),
                    name: "y".into(),
                },
            ],
        }));
        query.slice = RowSlice {
            offset: offset as u64,
            limit: Some(limit as u64),
        };
        let mut reader = mapped.storage.query(query).await.unwrap();
        let mut actual = Vec::new();
        while let Some(output) = reader.next().await.unwrap() {
            let QueryArrow::Batch(batch) = output else { continue };
            let batch = batch.as_record_batch();
            let x = batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
            let y = batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap();
            let points = x.values().iter().copied().zip(y.values().iter().copied());
            actual.extend(points);
        }
        let expected = payloads.iter().skip(offset).take(limit).map(|row| (row.x, row.y)).collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(reader.completion().unwrap().rows as usize, expected.len());
    }
    mapped.finish().await;
}

#[tokio::test]
async fn selection_preserves_schema() {
    let m = Mapped::new(storage_settings(), "mapping", &payloads()).await;
    let mut reader = m.storage.query(m.query(None)).await.unwrap();
    let Some(QueryArrow::Schema(announced)) = reader.next().await.unwrap() else {
        panic!("schema first");
    };
    let Some(QueryArrow::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("data batch");
    };

    // Optional, Repeated And Enum Fields Map To Nullable, List And Variant Columns.
    let payload = batch.as_record_batch().column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    assert!(payload.column_by_name("label").unwrap().is_null(0));
    let values = payload.column_by_name("values").unwrap().as_any().downcast_ref::<ListArray>().unwrap();
    let first = values.value(0);
    assert_eq!(first.as_any().downcast_ref::<Float64Array>().unwrap().values().as_ref(), &[0.0, -1.0, 2.0]);
    let event = payload.column_by_name("event").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    let discriminants = event.column_by_name("__variant").unwrap().as_any().downcast_ref::<UInt32Array>().unwrap();
    assert_eq!(discriminants.values().as_ref(), &[0, 1]);
    assert!(!event.column(1).is_null(0));
    assert!(event.column(1).is_null(1));
    assert!(event.column(2).is_null(0));
    assert!(!event.column(2).is_null(1));
    drop(batch);
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().rows, 2);
    drop(reader);

    // A Selection Without Rows Announces The Same Schema And Completes Empty.
    let query = m.query(Some("absent"));
    let mut empty = m.storage.query(query).await.unwrap();
    let Some(QueryArrow::Schema(empty_schema)) = empty.next().await.unwrap() else {
        panic!("empty schema");
    };
    assert_eq!(announced.as_schema(), empty_schema.as_schema());
    assert!(empty.next().await.unwrap().is_none());
    assert_eq!(empty.completion().unwrap().rows, 0);
    drop(empty);
    drop(announced);
    drop(empty_schema);
    m.finish().await;
}

#[tokio::test]
async fn admitted_queries_progress() {
    // Four Admitted Queries Progress While Their Schema Output Is Retained.
    let m = Mapped::new(storage_settings(), "mapping", &payloads()).await;
    let mut readers = Vec::new();
    for _ in 0..4 {
        let query = m.query(None);
        let reader = m.storage.query(query).await.unwrap();
        readers.push(reader);
    }
    let queries = readers.iter_mut().map(|reader| async move {
        let mut rows = 0;
        while let Some(output) = reader.next().await.unwrap() {
            if let QueryArrow::Batch(batch) = output {
                rows += batch.as_record_batch().num_rows();
            }
        }
        assert_eq!(reader.completion().unwrap().rows, 2);
        rows
    });
    let results = futures_util::future::join_all(queries).await;
    assert_eq!(results, [2, 2, 2, 2]);
    drop(readers);
    m.finish().await;
}

#[tokio::test]
async fn conversion_preserves_expansion() {
    // Compact Payloads Expand Without A Shared Heap Budget.
    let settings = storage_settings();
    let m = Mapped::new(settings.clone(), "expansion", &compact()).await;
    let query = m.query(Some("row-0"));
    let mut reader = m.storage.query(query).await.unwrap();
    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    let Some(QueryArrow::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("expanded data batch");
    };
    assert_eq!(batch.as_record_batch().num_rows(), 1);
    let retained = batch.as_record_batch().clone();
    let payload = retained.column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    let values = payload.column_by_name("values").unwrap().as_any().downcast_ref::<ListArray>().unwrap().value(0);
    let strings = values.as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
    assert_eq!(strings.len(), 4096);
    assert!(strings.iter().all(|value| value == Some("")));
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().rows, 1);

    // Arrow Clones And Empty Slices Keep Their Original Buffers Alive.
    let empty = retained.column(0).slice(retained.num_rows(), 0);
    drop(values);
    drop(batch);
    drop(reader);
    drop(retained);
    drop(empty);
    m.finish().await;
}
