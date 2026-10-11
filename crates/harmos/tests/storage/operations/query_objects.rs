/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use harmos::storage::operations::{ObjectResult, ObjectSelection, ReadObjects, ReadObjectsRequest};
use harmos::storage::{
    ObjectComparison, ObjectFields, ObjectFilter, ObjectMetadata, ObjectNullOrder, ObjectScalar,
    ObjectSort,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn fields() -> ObjectFields {
    ObjectFields {
        projection: vec![Identifier::new("text").unwrap()],
        filters: vec![ObjectFilter {
            field: Identifier::new("text").unwrap(),
            comparison: ObjectComparison::GreaterOrEqual,
            value: ObjectScalar::Text("b".into()),
        }],
        order: vec![ObjectSort {
            field: Identifier::new("text").unwrap(),
            descending: true,
            nulls: ObjectNullOrder::Last,
        }],
    }
}
async fn collect(runtime: &Runtime<Bank>, request: ReadObjectsRequest) -> Vec<ObjectMetadata> {
    let (sender, mut receiver) =
        harmos::operations::stream::<ObjectResult>(harmos::operations::StreamSettings {
            items: NonZeroUsize::MIN,
            bytes: NonZeroUsize::new(4096).unwrap(),
        })
        .unwrap();
    let call = runtime
        .operations
        .submit_endpoints::<ReadObjects, _>(request, sender)
        .unwrap();
    let mut rows = Vec::new();
    while let Some(item) = receiver.recv().await {
        let ObjectResult::Fields(row) = &*item else {
            panic!("expected scalar fields");
        };
        rows.push(row.clone());
    }
    call.await.unwrap();
    rows
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("projection_keeps_global", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, storage, data) = query_fixture().await;
                    let mut scripted = Vec::new();
                    for (key, text) in [("first", "c"), ("filtered", "a"), ("last", "b")] {
                        let receipt = storage
                            .write_object(&Document {
                                id: key.into(),
                                text: text.into(),
                            })
                            .await
                            .unwrap();
                        if text >= "b" {
                            let value = value(key, text);
                            scripted.push(ObjectMetadata {
                                schema: value.schema,
                                item: value.item,
                                position: receipt.position,
                                version: receipt.version,
                                values: vec![ObjectScalar::Text(text.into())],
                            });
                        }
                    }
                    storage.flush().await.unwrap();
                    *data.control.browse_rows.lock().unwrap() = Some(scripted.clone());
                    let mut selection = ObjectSelection {
                        schema: Some(value("first", "").schema),
                        ..Default::default()
                    };
                    selection.selection.page_rows = 1;
                    let request = ReadObjectsRequest {
                        selection,
                        fields: Some(fields()),
                    };
                    let rows = collect(&runtime, request.clone()).await;
                    assert_eq!(rows, scripted);
                    let observed = data.control.browse_request.lock().unwrap().clone().unwrap();
                    assert_eq!(
                        observed.predicates[0].comparison,
                        ObjectComparison::GreaterOrEqual
                    );
                    assert_eq!(observed.columns[0].name.as_str(), "text");
                    assert_eq!(
                        observed.order[0].direction,
                        harmos::storage::ReadOrder::Descending
                    );
                    let mut metadata = request;
                    metadata.fields.as_mut().unwrap().projection.clear();
                    let rows = collect(&runtime, metadata).await;
                    assert_eq!(
                        rows.iter().map(|row| row.item.as_str()).collect::<Vec<_>>(),
                        vec!["first", "last"]
                    );
                    assert!(rows.iter().all(|row| row.values.is_empty()));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("projection_rejects_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, storage, data) = query_fixture().await;
                    let receipt = storage
                        .write_object(&Document {
                            id: "key".into(),
                            text: "c".into(),
                        })
                        .await
                        .unwrap();
                    storage.flush().await.unwrap();
                    let value = value("key", "c");
                    *data.control.browse_rows.lock().unwrap() = Some(vec![ObjectMetadata {
                        schema: value.schema.clone(),
                        item: value.item,
                        position: receipt.position,
                        version: receipt.version,
                        values: vec![ObjectScalar::Unsigned(7)],
                    }]);
                    let request = ReadObjectsRequest {
                        selection: ObjectSelection {
                            schema: Some(value.schema),
                            ..Default::default()
                        },
                        fields: Some(fields()),
                    };
                    let (sender, mut receiver) = harmos::operations::stream::<ObjectResult>(
                        harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        },
                    )
                    .unwrap();
                    let call = runtime
                        .operations
                        .submit_endpoints::<ReadObjects, _>(request, sender)
                        .unwrap();
                    let error = call.await.err().unwrap();
                    assert!(
                        error.to_string().contains("scalar value does not match"),
                        "{error}"
                    );
                    assert!(receiver.recv().await.is_none());
                    runtime.stop().await.unwrap();
                });
        });
    }
}
