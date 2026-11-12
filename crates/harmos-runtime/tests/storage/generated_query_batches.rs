/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Query a Fixed Storage Frontier Through Bounded Generated Batches.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn query_operation_batches() {
    // Configure Response Boundaries And Register The Actual Operation.
    let mut settings = config();
    settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
    let fixture = Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
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
    fixture.storage.append_records((records).to_vec()).await.unwrap();
    let at = fixture.storage.flush().await.unwrap();

    let retained = fixture
        .runtime
        .operations
        .call_protobuf::<harmos_operations::storage::CreateSnapshot>(harmos_protocol::v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    let cut = retained.resolved_at.unwrap();

    // A Later Durable Row Must Not Advance The Requested Query Boundary.
    let late_item = Identifier::new("later").unwrap();
    let late_payload = Reading { value: 100 }.encode();
    fixture
        .storage
        .append_records(vec![RecordInput {
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
    let wire: harmos_protocol::storage::Query = request.try_into().unwrap();
    let mut request = harmos_protocol::v1::StorageListRowsRequest {
        source: wire.source,
        transforms: wire.transforms,
        at: Some(cut.clone()),
        page: Some(harmos_protocol::v1::PageRequest {
            page_size: 2,
            max_bytes: 65536,
            page_token: String::new(),
        }),
    };
    let mut announced = None;
    let mut received = Vec::new();
    let mut identities = Vec::new();
    let mut batches = 0;
    let mut skip = 1;
    loop {
        let page = fixture.runtime.operations.call_protobuf::<ListRows>(request.clone()).await.unwrap();
        assert_eq!(page.resolved_at, Some(cut.clone()));
        assert_eq!(page.resolved_at.as_ref().unwrap().positions[0].sequence, at.sequence);
        for output in page.rows {
            match QueryArrow::try_from(output).unwrap() {
                QueryArrow::Schema(header) => {
                    assert_eq!(
                        header.as_schema().fields().iter().map(|field| field.name().as_str()).collect::<Vec<_>>(),
                        vec!["value", "item"]
                    );
                    if let Some(schema) = &announced {
                        assert_eq!(header.as_schema(), schema);
                    }
                    announced = Some(header.as_schema().clone());
                }
                QueryArrow::Batch(batch) => {
                    batch.validate_schema(announced.as_ref().unwrap()).unwrap();
                    let batch = batch.as_record_batch();
                    assert!((1..=2).contains(&batch.num_rows()));
                    let values = batch.column_by_name("value").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
                    let items = batch.column_by_name("item").unwrap().as_any().downcast_ref::<StringArray>().unwrap();
                    for row in 0..batch.num_rows() {
                        if skip > 0 {
                            skip -= 1;
                            continue;
                        }
                        received.push(values.value(row));
                        identities.push(items.value(row).to_owned());
                    }
                    batches += 1;
                }
            }
        }
        if page.next_page_token.is_empty() {
            break;
        }
        request.page.as_mut().unwrap().page_token = page.next_page_token;
    }
    assert_eq!(batches, 3);
    assert_eq!(received, vec![4, 7, 8, 9]);
    assert_eq!(identities, vec!["row-3", "row-2", "row-5", "row-0"]);
    fixture
        .runtime
        .operations
        .call_protobuf::<harmos_operations::storage::DeleteSnapshot>(harmos_protocol::v1::StorageDeleteSnapshotRequest {
            snapshot: retained.snapshot,
        })
        .await
        .unwrap();
    fixture.finish().await;
}
