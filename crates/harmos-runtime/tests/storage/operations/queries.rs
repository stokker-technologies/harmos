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
use super::{Fixture, Lanes, config};
use harmos_runtime::storage::operations::Query;
use harmos_runtime::storage::operations::QueryRequest as Request;
use harmos_runtime::storage::query::scalar::ScalarValue;
use harmos_runtime::storage::query::transform::{
    Transform,
    filter::{Clause, Compare, Condition, Filter, Term},
    project::{Project, Projection},
    sort::{Sort, SortKey},
};
use harmos_runtime::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
use harmos_runtime::storage::{
    ObjectKey, ObjectRetention, PayloadEncoding, RecordInput, SchemaKey, StorageBackends, StorageManager, StoredObject,
};
use harmos_runtime::{DefinitionId, Identified, Identifier, Message, Runtime};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
struct Reading {
    value: u64,
}

#[harmos_runtime::message]
struct First {
    id: String,
    value: u64,
}
#[harmos_runtime::message]
struct Second {
    id: String,
    value: u64,
    annotation: String,
}
#[harmos_runtime::message]
struct Third {
    id: String,
    value: u64,
}

/* === Enums === */

#[harmos_runtime::catalog]
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

#[tokio::test]
async fn query_operation_batches() {
    // Configure Response Boundaries And Register The Actual Operation.
    let mut settings = config();
    settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
    let fixture = Fixture::new(settings).await;
    fixture
        .runtime
        .operations
        .register(harmos_runtime::operations::RegistrationExt::lane(
            harmos_runtime::storage::operations::registration(Arc::new(fixture.storage.clone())).unwrap(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("query-reading").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(
        <Reading as harmos_runtime::schema::Described>::DEFINITION,
        &harmos_runtime::config::SchemaSettings::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph.clone()).await.unwrap();
    let values = [9, 4, 7, 4, 1, 8];
    let items = (0..values.len()).map(|index| Identifier::new(format!("row-{index}")).unwrap()).collect::<Vec<_>>();
    let payloads = values.iter().map(|value| Reading { value: *value }.encode()).collect::<Vec<_>>();
    let records = items
        .iter()
        .zip(&payloads)
        .map(|(item, payload)| RecordInput {
            schema: schema.clone(),
            scope: None,
            item: item.clone(),
            source: None,
            source_time: None,
            payload: (payload).to_vec(),
        })
        .collect::<Vec<_>>();
    fixture.storage.write_records((records).to_vec()).await.unwrap();
    let at = fixture.storage.flush().await.unwrap();

    // A Later Durable Row Must Not Advance The Requested Query Boundary.
    let late_item = Identifier::new("later").unwrap();
    let late_payload = Reading { value: 100 }.encode();
    fixture
        .storage
        .write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: late_item.clone(),
            source: None,
            source_time: None,
            payload: (late_payload).to_vec(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let request = Request {
        at: at.clone(),
        source: Source::Records(RecordSelection { schema, scope: None, item: None, source_start: None, source_end: None }),
        transforms: vec![
            Transform::Filter(Filter {
                clauses: vec![Clause {
                    terms: vec![Term {
                        field: "payload.value".into(),
                        condition: Condition::Compare { op: Compare::Ge, value: ScalarValue::Unsigned(4) },
                        negate: false,
                    }],
                }],
            }),
            Transform::Sort(Sort { keys: vec![SortKey { field: "payload.value".into(), descending: false, nulls_first: false }] }),
            Transform::Project(Project {
                fields: vec![
                    Projection { field: "payload.value".into(), name: "value".into() },
                    Projection { field: "_storage.item".into(), name: "item".into() },
                ],
            }),
        ],
        slice: RowSlice { offset: 1, limit: Some(4) },
    };
    let stream =
        harmos_runtime::operations::StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap() };
    let (sender, mut receiver) = harmos_runtime::operations::stream::<harmos_runtime::storage::query::QueryOutput>(stream).unwrap();
    let instance = fixture.runtime.operations.submit_endpoints::<Query, _>(request, sender).unwrap();

    // One Invocation Announces One Schema, Then Bounded Batches.
    let first = QueryArrow::try_from(receiver.recv().await.unwrap().into_owned_request().unwrap().0).unwrap();
    let QueryArrow::Schema(header) = &first else {
        panic!("query must announce schema first");
    };
    assert_eq!(header.as_schema().fields().iter().map(|field| field.name().as_str()).collect::<Vec<_>>(), vec!["value", "item"]);
    let announced = header.as_schema().clone();
    drop(first);
    let mut received = Vec::new();
    let mut identities = Vec::new();
    let mut batches = 0;
    while let Some(item) = receiver.recv().await {
        let item = QueryArrow::try_from(item.into_owned_request().unwrap().0).unwrap();
        let QueryArrow::Batch(batch) = &item else {
            panic!("schema may only be announced once");
        };
        batch.validate_schema(&announced).unwrap();
        let batch = batch.as_record_batch();
        assert!((1..=2).contains(&batch.num_rows()));
        let values = batch.column_by_name("value").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
        let items = batch.column_by_name("item").unwrap().as_any().downcast_ref::<StringArray>().unwrap();
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
}

/* === Private Functions === */

#[tokio::test]
async fn query_heterogeneous_projection() {
    let storage = StorageManager::<Objects>::new(
        StorageBackends { primary: Arc::new(Data::<Objects>::with_catalog(history())), state: Arc::new(Recovery::new(history())) },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    storage.write_object(&First { id: "shared".into(), value: 1 }).await.unwrap();
    storage.write_object(&Second { id: "different".into(), value: 2, annotation: "extra field".into() }).await.unwrap();
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
        .query(Request {
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
        .query(Request {
            at: at.clone(),
            source: selected(None),
            transforms: vec![Transform::Project(Project {
                fields: vec![
                    Projection { field: "payload.value".into(), name: "value".into() },
                    Projection { field: "_storage.schema_id".into(), name: "schema".into() },
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
    let mut incompatible =
        storage.query(Request { at, source: selected(None), transforms: vec![], slice: RowSlice::default() }).await.unwrap();
    assert!(incompatible.next().await.is_err());
    drop(incompatible);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn ipc_admission_retains() {
    let fixture = Fixture::new(config()).await;
    let schema = SchemaKey {
        id: Identifier::new("admitted-query").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(
        <Reading as harmos_runtime::schema::Described>::DEFINITION,
        &harmos_runtime::config::SchemaSettings::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    fixture
        .storage
        .write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("row").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 7 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    for cancel in [true, false] {
        let mut reader = fixture
            .storage
            .query(Request {
                at: fixture.storage.cursor_saved(),
                source: Source::Records(RecordSelection {
                    schema: schema.clone(),
                    scope: None,
                    item: None,
                    source_start: None,
                    source_end: None,
                }),
                transforms: Vec::new(),
                slice: RowSlice::default(),
            })
            .await
            .unwrap();
        let item = reader.next().await.unwrap().unwrap();
        // The Announced Schema Shares Its Claim With The Reader's Plan.
        // Release That Peer So This Story Isolates The Pending Output Owner.
        drop(reader);
        fixture.data.snapshots_released().await;
        let before = fixture.storage.status().read_bytes;
        assert!(before > 0);
        let bytes = 4 * 1024 * 1024;
        let (sender, mut receiver) =
            harmos_runtime::operations::stream::<harmos_runtime::storage::query::QueryOutput>(harmos_runtime::operations::StreamSettings {
                items: NonZeroUsize::MIN,
                bytes: NonZeroUsize::new(bytes).unwrap(),
            })
            .unwrap();
        let blocked = sender.reserve(bytes).await.unwrap();
        let converted = std::cell::Cell::new(false);
        let mut sending = Box::pin(sender.send_with(item.output_memory_size(), || {
            converted.set(true);
            Ok(item.output())
        }));
        assert!(super::super::pending(sending.as_mut()).await);
        assert!(!converted.get());
        assert_eq!(fixture.storage.status().read_bytes, before);
        drop(blocked);
        if cancel {
            drop(sending);
            assert!(!converted.get());
            assert_eq!(fixture.storage.status().read_bytes, 0);
        } else {
            sending.await.unwrap();
            assert!(converted.get());
            let output = receiver.recv().await.unwrap();
            assert!(matches!(&*output, harmos_runtime::storage::query::QueryOutput::Schema(_)));
            assert_eq!(fixture.storage.status().read_bytes, 0);
            // Destination Admission Now Covers The Wire Bytes Until Consumption.
            let mut next = Box::pin(sender.reserve(bytes));
            assert!(super::super::pending(next.as_mut()).await);
            drop(output);
            drop(next.await.unwrap());
            continue;
        }
        assert_eq!(fixture.storage.status().read_bytes, 0);
    }
    fixture.finish().await;
}
