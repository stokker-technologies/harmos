/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn mapped_snapshot_preserves_schema_and_progresses_concurrently() {
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

    // Concurrent Readers Share The Same Stored Snapshot.
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
