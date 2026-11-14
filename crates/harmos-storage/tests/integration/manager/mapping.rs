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

    use harmos_query::transform::{
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

#[path = "mapping/mapped_snapshot_preserves_schema_and_progresses_concurrently.rs"]
mod mapped_snapshot_preserves_schema_and_progresses_concurrently;

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
