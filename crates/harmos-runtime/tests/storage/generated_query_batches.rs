/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Query a Fixed Storage Frontier Through Bounded Generated Batches.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

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

/* === Tests === */

#[tokio::test]
async fn query_operation_batches() {
    // Configure Response Boundaries And Register The Actual Operation.
    let mut settings = config();
    settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
    let fixture = Fixture::new(settings).await;
    fixture
        .runtime
        .operations
        .register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
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
        slice: RowSlice { offset: 1, limit: Some(4) },
    };
    let stream = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
    };
    let mut response = fixture.runtime.operations.download_protobuf::<Query>(request.into(), stream).unwrap();

    // One Invocation Announces One Schema, Then Bounded Batches.
    let first = QueryArrow::try_from(response.next().await.unwrap().unwrap()).unwrap();
    let QueryArrow::Schema(header) = &first else {
        panic!("query must announce schema first");
    };
    assert_eq!(header.as_schema().fields().iter().map(|field| field.name().as_str()).collect::<Vec<_>>(), vec!["value", "item"]);
    let announced = header.as_schema().clone();
    drop(first);
    let mut received = Vec::new();
    let mut identities = Vec::new();
    let mut batches = 0;
    while let Some(item) = response.next().await.unwrap() {
        let item = QueryArrow::try_from(item).unwrap();
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
    let completion = response.join().await.unwrap();
    assert_eq!(completion.at, Some(at.into()));
    assert_eq!(completion.rows, 4);
    assert_eq!(batches, 2);
    assert_eq!(received, vec![4, 7, 8, 9]);
    assert_eq!(identities, vec!["row-3", "row-2", "row-5", "row-0"]);
    fixture.finish().await;
}
