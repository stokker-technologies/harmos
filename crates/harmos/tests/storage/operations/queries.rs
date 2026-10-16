/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Query Operation Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
// Third Party Imports
use arrow_array::{StringArray, StructArray, UInt64Array};
// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, history};
use super::{Fixture, Lanes, Operations, config};
use harmos::storage::operations::Query;
use harmos::storage::query::scalar::ScalarValue;
use harmos::storage::query::transform::{
    Transform,
    filter::{Clause, Compare, Condition, Filter, Term},
    project::{Project, Projection},
    sort::{Sort, SortKey},
};
use harmos::storage::query::{Query as Request, QueryOutput, RecordSelection, RowSlice, Source};
use harmos::storage::{
    ObjectKey, ObjectRetention, PayloadEncoding, RecordWrite, SchemaKey, StorageBackends,
    StorageManager, StoredObject,
};
use harmos::{DefinitionId, Identified, Identifier, Message, Runtime};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
struct Reading {
    value: u64,
}

#[harmos::message]
struct First {
    id: String,
    value: u64,
}
#[harmos::message]
struct Second {
    id: String,
    value: u64,
    annotation: String,
}
#[harmos::message]
struct Third {
    id: String,
    value: u64,
}

/* === Enums === */
#[harmos::catalog]
enum Objects {
    First(First),
    Second(Second),
    Third(Third),
}

/* === Traits === */

/* === Implementations === */
macro_rules! object {
    ($ty:ty, $name:literal) => {
        impl Identified for $ty {
            fn definition() -> (DefinitionId, u32) {
                (DefinitionId::new($name), 1)
            }
        }
        impl StoredObject for $ty {
            type Key = String;
            const RETENTION: ObjectRetention = ObjectRetention::UntilReleased;
            fn key(&self) -> &String {
                &self.id
            }
        }
    };
}
object!(First, "test.query.first");
object!(Second, "test.query.second");
object!(Third, "test.query.third");

/* === Public Functions === */
pub(super) fn contract_scenario() {
    objects_scenario();
    crate::scenario_runtime::check("query_schema_batches_completion", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                // Configure Response Boundaries And Register The Actual Operation.
                let mut settings = config();
                settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
                let fixture = Fixture::new(settings).await;
                Operations::register(
                    &fixture.runtime.operations,
                    "storage",
                    Lanes::Open,
                    Arc::new(fixture.storage.clone()),
                )
                .unwrap();
                let schema = SchemaKey {
                    id: Identifier::new("query-reading").unwrap(),
                    version: NonZeroU32::new(1).unwrap(),
                    payload_encoding: PayloadEncoding::PostcardV1,
                };
                let graph = harmos::schema::graph::Schema::from_declaration(
                    <Reading as harmos::schema::Described>::DEFINITION,
                    &harmos::config::SchemaSettings::default(),
                )
                .unwrap();
                fixture.storage.write_schema(&schema, &graph).await.unwrap();
                let values = [9, 4, 7, 4, 1, 8];
                let items = (0..values.len())
                    .map(|index| Identifier::new(format!("row-{index}")).unwrap())
                    .collect::<Vec<_>>();
                let payloads = values
                    .iter()
                    .map(|value| Reading { value: *value }.encode_to_vec())
                    .collect::<Vec<_>>();
                let records = items
                    .iter()
                    .zip(&payloads)
                    .map(|(item, payload)| RecordWrite {
                        schema: &schema,
                        scope: None,
                        item,
                        source: None,
                        source_time: None,
                        payload,
                    })
                    .collect::<Vec<_>>();
                fixture.storage.write_records(&records).await.unwrap();
                let at = fixture.storage.flush().await.unwrap();

                // A Later Durable Row Must Not Advance The Requested Query Boundary.
                let late_item = Identifier::new("later").unwrap();
                let late_payload = Reading { value: 100 }.encode_to_vec();
                fixture
                    .storage
                    .write_records(&[RecordWrite {
                        schema: &schema,
                        scope: None,
                        item: &late_item,
                        source: None,
                        source_time: None,
                        payload: &late_payload,
                    }])
                    .await
                    .unwrap();
                fixture.storage.flush().await.unwrap();
                let request = Request {
                    at: at.clone(),
                    source: Source::Records(RecordSelection {
                        schema,
                        scope: None,
                        item: None,
                        source_start: None,
                        source_end: None,
                    }),
                    transforms: vec![
                        Transform::Filter(Filter {
                            clauses: vec![Clause {
                                terms: vec![Term {
                                    field: "payload.value".into(),
                                    condition: Condition::Compare {
                                        op: Compare::Ge,
                                        value: ScalarValue::Unsigned(4),
                                    },
                                    negate: false,
                                }],
                            }],
                        }),
                        Transform::Sort(Sort {
                            keys: vec![SortKey {
                                field: "payload.value".into(),
                                descending: false,
                                nulls_first: false,
                            }],
                        }),
                        Transform::Project(Project {
                            fields: vec![
                                Projection {
                                    field: "payload.value".into(),
                                    name: "value".into(),
                                },
                                Projection {
                                    field: "_storage.item".into(),
                                    name: "item".into(),
                                },
                            ],
                        }),
                    ],
                    slice: RowSlice {
                        offset: 1,
                        limit: Some(4),
                    },
                };
                let stream = harmos::operations::StreamSettings {
                    items: NonZeroUsize::MIN,
                    bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                };
                let (sender, mut receiver) =
                    harmos::operations::stream::<QueryOutput>(stream).unwrap();
                let instance = fixture
                    .runtime
                    .operations
                    .submit_endpoints::<Query, _>(request, sender)
                    .unwrap();

                // One Invocation Announces One Schema, Then Bounded Batches.
                let first = receiver.recv().await.unwrap();
                let QueryOutput::Schema(header) = &*first else {
                    panic!("query must announce schema first");
                };
                assert_eq!(
                    header
                        .as_schema()
                        .fields()
                        .iter()
                        .map(|field| field.name().as_str())
                        .collect::<Vec<_>>(),
                    vec!["value", "item"]
                );
                let announced = header.as_schema().clone();
                drop(first);
                let mut received = Vec::new();
                let mut identities = Vec::new();
                let mut batches = 0;
                while let Some(item) = receiver.recv().await {
                    let QueryOutput::Batch(batch) = &*item else {
                        panic!("schema may only be announced once");
                    };
                    batch.validate_schema(&announced).unwrap();
                    let batch = batch.as_record_batch();
                    assert!((1..=2).contains(&batch.num_rows()));
                    let values = batch
                        .column_by_name("value")
                        .unwrap()
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap();
                    let items = batch
                        .column_by_name("item")
                        .unwrap()
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .unwrap();
                    received.extend(values.values().iter().copied());
                    identities.extend(items.iter().map(|value| value.unwrap().to_owned()));
                    batches += 1;
                }
                let completion = instance.await.unwrap();
                assert_eq!(completion.at, at);
                assert_eq!(completion.rows, 4);
                assert_eq!(batches, 2);
                assert_eq!(received, vec![4, 7, 8, 9]);
                assert_eq!(identities, vec!["row-3", "row-2", "row-5", "row-0"]);
                fixture.finish().await;
            });
    });
}

/* === Private Functions === */
fn objects_scenario() {
    crate::scenario_runtime::check("query_heterogeneous_projection", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let storage = StorageManager::<Objects>::new(
                    StorageBackends {
                        primary: Arc::new(Data::<Objects>::with_catalog(history())),
                        state: Arc::new(Recovery::new(history())),
                    },
                    config(),
                )
                .unwrap();
                let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                    .register(catalog())
                    .register(Lanes::catalog())
                    .runtime_with_storage(&storage)
                    .await
                    .unwrap();
                storage
                    .write_object(&First {
                        id: "shared".into(),
                        value: 1,
                    })
                    .await
                    .unwrap();
                storage
                    .write_object(&Second {
                        id: "different".into(),
                        value: 2,
                        annotation: "extra field".into(),
                    })
                    .await
                    .unwrap();
                storage
                    .write_object(&Third {
                        id: "shared".into(),
                        value: 3,
                    })
                    .await
                    .unwrap();
                let at = storage.flush().await.unwrap();
                let selected = |key| {
                    Source::Objects(harmos::storage::query::ObjectSelection {
                        schema: None,
                        key,
                        versions: harmos::storage::query::ObjectVersions::Latest,
                    })
                };

                // Equal Graphs Under Distinct Logical Keys Retain Their Actual Identity.
                let mut matching = storage
                    .query(Request {
                        at: at.clone(),
                        source: selected(Some(
                            ObjectKey::new("shared", NonZeroUsize::new(128).unwrap()).unwrap(),
                        )),
                        transforms: vec![],
                        slice: RowSlice::default(),
                    })
                    .await
                    .unwrap();
                let mut schema_ids = Vec::new();
                while let Some(output) = matching.next().await.unwrap() {
                    if let QueryOutput::Batch(batch) = output {
                        let metadata = batch
                            .as_record_batch()
                            .column_by_name("_storage")
                            .unwrap()
                            .as_any()
                            .downcast_ref::<StructArray>()
                            .unwrap();
                        let ids = metadata
                            .column_by_name("schema_id")
                            .unwrap()
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .unwrap();
                        schema_ids.extend(ids.iter().map(|id| id.unwrap().to_owned()));
                    }
                }
                assert_eq!(schema_ids, vec!["test.query.first", "test.query.third"]);
                drop(matching);

                // A Leading Projection Normalizes Common Fields Across Different Graphs.
                let mut projected = storage
                    .query(Request {
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
                let Some(QueryOutput::Schema(header)) = projected.next().await.unwrap() else {
                    panic!("schema first");
                };
                assert_eq!(header.as_schema().fields().len(), 2);
                let mut values = Vec::new();
                while let Some(output) = projected.next().await.unwrap() {
                    let QueryOutput::Batch(batch) = output else {
                        panic!("one header");
                    };
                    batch.validate_schema(header.as_schema()).unwrap();
                    let column = batch
                        .as_record_batch()
                        .column(0)
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap();
                    values.extend(column.values().iter().copied());
                }
                assert_eq!(values, vec![1, 2, 3]);
                assert_eq!(projected.completion().unwrap().rows, 3);
                drop(header);
                drop(projected);

                // Incompatible Unprojected Types Fail Before Publishing A Schema.
                let mut incompatible = storage
                    .query(Request {
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
                runtime.stop().await.unwrap();
            });
    });
}

/* === Tests === */
