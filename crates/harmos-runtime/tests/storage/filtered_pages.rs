/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Private Storage Pages at Their Issued Cuts

/* === Imports === */

use super::{Fixture, config};
use harmos_domain::Message;
use harmos_domain::model::storage::query::Query;
use harmos_operations::storage::{ListRows, ListRowsRequest};
use harmos_protocol::model;
use harmos_query::QueryArrow;
use harmos_query::transform::Transform;
use harmos_query::transform::filter::Clause;
use harmos_query::transform::filter::Compare;
use harmos_query::transform::filter::Condition;
use harmos_query::transform::filter::Filter;
use harmos_query::transform::filter::Term;
use harmos_storage::Identifier;
use harmos_storage::PayloadEncoding;
use harmos_storage::RecordInput;
use harmos_storage::SchemaKey;
use harmos_storage::query::RecordSelection;
use harmos_storage::query::RowSlice;
use harmos_storage::query::Source;
use harmos_storage::query::scalar::ScalarValue;
use std::{num::NonZeroU32, sync::atomic::Ordering};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}

/* === Tests === */

#[tokio::test]
async fn filtered_physical_pages_return_progress_without_rescanning_the_prefix() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("filtered-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    for (item, value) in [("first", 0), ("second", 0), ("third", 0), ("match", 1)] {
        fixture
            .storage
            .append_records(vec![RecordInput {
                schema: schema.clone(),
                scope: None,
                item: Identifier::new(item).unwrap(),
                source: None,
                source_time: None,
                payload: Reading { value }.encode(),
            }])
            .await
            .unwrap();
    }
    let at = fixture.storage.flush().await.unwrap();
    fixture.data.control.record_page_rows.store(2, Ordering::Relaxed);
    let query = Query {
        merge: None,
        at,
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
                            value: ScalarValue::Unsigned(1),
                        },
                        negate: false,
                    }],
                }],
            }),
            Transform::Project(harmos_query::transform::project::Project {
                fields: vec![harmos_query::transform::project::Projection {
                    field: "_storage.item".into(),
                    name: "item".into(),
                }],
            }),
        ],
        slice: RowSlice::default(),
    };
    let mut request = ListRowsRequest {
        merge: None,
        source: Some(query.source),
        transforms: query.transforms,
        at: None,
        page: None,
    };
    let schema = fixture.runtime.operations.call_native::<ListRows>(request.clone()).await.unwrap();
    assert!(matches!(QueryArrow::try_from(schema.rows[0].clone()).unwrap(), QueryArrow::Schema(_)));
    assert_eq!(fixture.data.control.record_rows_examined.load(Ordering::Relaxed), 0);
    request.page = Some(model::PageRequest {
        page_token: schema.next_page_token,
        ..Default::default()
    });
    let filtered = fixture.runtime.operations.call_native::<ListRows>(request.clone()).await.unwrap();
    assert!(filtered.rows.is_empty());
    assert!(!filtered.next_page_token.is_empty());
    let examined = fixture.data.control.record_rows_examined.load(Ordering::Relaxed);
    assert!(examined > 0 && examined < fixture.data.records.lock().unwrap().len());
    assert_eq!(
        fixture.runtime.operations.call_native::<ListRows>(request.clone()).await.unwrap().next_page_token,
        filtered.next_page_token
    );
    assert_eq!(fixture.data.control.record_rows_examined.load(Ordering::Relaxed), examined);
    request.page.as_mut().unwrap().page_token = filtered.next_page_token;
    let mut items = Vec::new();
    loop {
        let response = fixture.runtime.operations.call_native::<ListRows>(request.clone()).await.unwrap();
        assert_eq!(response.resolved_at, filtered.resolved_at);
        for row in response.rows {
            if let QueryArrow::Batch(batch) = QueryArrow::try_from(row).unwrap() {
                let values =
                    batch.as_record_batch().column_by_name("item").unwrap().as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
                items.extend(values.iter().map(|item| item.unwrap().to_owned()));
            }
        }
        if response.next_page_token.is_empty() {
            break;
        }
        request.page.as_mut().unwrap().page_token = response.next_page_token;
    }
    assert_eq!(items, vec!["match"]);
    fixture.finish().await;
}
