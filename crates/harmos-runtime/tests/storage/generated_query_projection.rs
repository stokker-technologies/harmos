/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Project Heterogeneous Objects Through Their Retained Storage Schemas.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn query_heterogeneous_projection() {
    let storage = StorageManager::with_catalog(
        objects_catalog(),
        StorageBackends {
            primary: Arc::new(Data::with_catalog(objects_catalog(), history())),
            state: Arc::new(Recovery::new(history())),
        },
        config(),
    )
    .unwrap();
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog());
    })
    .await
    .unwrap();
    storage.write_object(&First { id: "shared".into(), value: 1 }).await.unwrap();
    storage
        .write_object(&Second {
            id: "different".into(),
            value: 2,
            annotation: "extra field".into(),
        })
        .await
        .unwrap();
    storage.write_object(&Third { id: "shared".into(), value: 3 }).await.unwrap();
    let at = storage.flush().await.unwrap();
    let selected = |key| {
        Source::Objects(harmos_runtime::storage::query::ObjectSelection {
            schema: None,
            key,
            versions: harmos_runtime::storage::query::ObjectVersions::Latest,
        })
    };

    // Equal Graphs Under Distinct Logical Keys Retain Their Actual Identity.
    let mut matching = storage
        .query(harmos_runtime::storage::Query {
            at: at.clone(),
            source: selected(Some(ObjectKey::new("shared", NonZeroUsize::new(128).unwrap()).unwrap())),
            transforms: vec![],
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    let mut schema_ids = Vec::new();
    while let Some(output) = matching.next().await.unwrap() {
        if let QueryArrow::Batch(batch) = output {
            let metadata = batch.as_record_batch().column_by_name("_storage").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
            let ids = metadata.column_by_name("schema_id").unwrap().as_any().downcast_ref::<StringArray>().unwrap();
            schema_ids.extend(ids.iter().map(|id| id.unwrap().to_owned()));
        }
    }
    assert_eq!(schema_ids, vec!["test.query.first", "test.query.third"]);
    drop(matching);

    // A Leading Projection Normalizes Common Fields Across Different Graphs.
    let mut projected = storage
        .query(harmos_runtime::storage::Query {
            at: at.clone(),
            source: selected(None),
            transforms: vec![Transform::Project(Project {
                fields: vec![
                    Projection {
                        field: "payload.value".into(),
                        name: "value".into(),
                    },
                    Projection {
                        field: "_storage.schema_id".into(),
                        name: "schema".into(),
                    },
                ],
            })],
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    let Some(QueryArrow::Schema(header)) = projected.next().await.unwrap() else {
        panic!("schema first");
    };
    assert_eq!(header.as_schema().fields().len(), 2);
    let mut values = Vec::new();
    while let Some(output) = projected.next().await.unwrap() {
        let QueryArrow::Batch(batch) = output else {
            panic!("one header");
        };
        batch.validate_schema(header.as_schema()).unwrap();
        let column = batch.as_record_batch().column(0).as_any().downcast_ref::<UInt64Array>().unwrap();
        values.extend(column.values().iter().copied());
    }
    assert_eq!(values, vec![1, 2, 3]);
    assert_eq!(projected.completion().unwrap().rows, 3);
    drop(header);
    drop(projected);

    // Incompatible Unprojected Types Fail Before Publishing A Schema.
    let mut incompatible = storage
        .query(harmos_runtime::storage::Query {
            at,
            source: selected(None),
            transforms: vec![],
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    assert!(incompatible.next().await.is_err());
    drop(incompatible);
    storage.close().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
