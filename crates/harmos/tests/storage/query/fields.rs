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
use crate::backend::storage_settings;

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn name(value: &str) -> Identifier {
    Identifier::new(value).unwrap()
}
fn query() -> EncodedObjectQuery {
    EncodedObjectQuery {
        schema: Some(SchemaKey {
            payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
            id: name("query.document"),
            version: std::num::NonZeroU32::MIN,
        }),
        ..Default::default()
    }
}
fn fields() -> ObjectFields {
    ObjectFields {
        projection: vec![name("value")],
        filters: vec![ObjectFilter {
            field: name("id"),
            comparison: ObjectComparison::Greater,
            value: ObjectScalar::Unsigned(3),
        }],
        order: vec![ObjectSort {
            field: name("value"),
            descending: true,
            nulls: ObjectNullOrder::Last,
        }],
    }
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("queries_resolve_typed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.data
                        .control
                        .allow_empty_browse
                        .store(true, Ordering::SeqCst);
                    let mut typed = ObjectQuery::<Document>::new()
                        .project(vec![name("value")])
                        .unwrap();
                    typed
                        .filter(
                            name("id"),
                            ObjectComparison::Greater,
                            ObjectScalar::Unsigned(3),
                        )
                        .unwrap();
                    typed
                        .order_by(name("value"), ReadOrder::Descending, ObjectNullOrder::Last)
                        .unwrap();
                    let expected = typed.request().clone();
                    let mut reader = f
                        .storage
                        .query(query().project(fields()).unwrap())
                        .await
                        .unwrap();
                    assert_eq!(
                        f.data.control.browse_request.lock().unwrap().as_ref(),
                        Some(&expected)
                    );
                    let saved = reader.view().unwrap().saved().unwrap();
                    assert_eq!(saved, f.storage.cursor_saved());
                    assert!(reader.next_page().await.unwrap().is_none());
                    assert!(reader.view().is_none());
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("queries_reject_plans", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.data
                        .control
                        .allow_empty_browse
                        .store(true, Ordering::SeqCst);
                    let mut unknown = fields();
                    unknown.projection = vec![name("unknown")];
                    let mut wrong = fields();
                    wrong.filters[0].value = ObjectScalar::Text("3".into());
                    let mut duplicate = fields();
                    duplicate.order.push(duplicate.order[0].clone());
                    let mut twice = fields();
                    twice.projection.push(name("value"));
                    let before = f.data.control.query_views.load(Ordering::SeqCst);
                    for fields in [unknown, wrong, duplicate, twice] {
                        let query = query().project(fields).unwrap();
                        assert!(f.storage.query(query).await.is_err());
                    }
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    assert!(f.data.control.browse_request.lock().unwrap().is_none());
                    let mut unknown = query();
                    unknown.schema.as_mut().unwrap().id = name("unknown");
                    assert!(
                        f.storage
                            .query(unknown.project(fields()).unwrap())
                            .await
                            .is_err()
                    );
                    assert!(EncodedObjectQuery::default().project(fields()).is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("scalar_query_support", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let before = f.data.control.query_views.load(Ordering::SeqCst);
                    assert!(
                        f.storage
                            .query(query().project(fields()).unwrap())
                            .await
                            .is_err()
                    );
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    f.data
                        .control
                        .allow_empty_browse
                        .store(true, Ordering::SeqCst);
                    let mut query = query().project(fields()).unwrap();
                    query.options.execution = Some(QueryLimits {
                        memory_bytes: NonZeroUsize::new(1024).unwrap(),
                        ..storage_settings().query_execution
                    });
                    assert!(f.storage.query(query).await.is_err());
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    f.finish().await;
                });
        });
    }
}
