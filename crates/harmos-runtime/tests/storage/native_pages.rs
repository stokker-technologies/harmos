/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Continue Private Native Lists at Their Issued Composite Cuts

/* === Imports === */

use super::{Fixture, config};
use harmos_domain::Message;
use harmos_operations::storage as operations;
use harmos_operations::storage::{ColumnSelection, IndexSelection, ProjectionSelection};
use harmos_protocol::v1;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnPredicate;
use harmos_query::columnar::ColumnSource;
use harmos_query::index::IndexDefinition;
use harmos_storage::MutationId;
use harmos_storage::ReadLimits;
use harmos_storage::query::projection::ProjectionDefinition;
use harmos_storage::query::projection::ProjectionTransform;
use harmos_storage::{Identifier, PayloadEncoding, RecordInput, SchemaKey};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::atomic::Ordering,
};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}
/* === Tests === */

#[tokio::test]
async fn native_lists_continue_filtered_pages_and_refuse_changed_revisions() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("native-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    for (item, value) in [("first", 0), ("second", 0), ("third", 2), ("fourth", 3)] {
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
    fixture.storage.flush().await.unwrap();
    let source = ColumnSource {
        kind: ColumnKind::Records,
        schema: schema.clone(),
    };
    let work = ReadLimits {
        rows: NonZeroUsize::new(64).unwrap(),
        bytes: NonZeroUsize::new(65536).unwrap(),
    };
    let page = v1::PageRequest {
        page_size: 1,
        max_bytes: 8192,
        page_token: String::new(),
    };
    fixture.data.control.record_page_rows.store(1, Ordering::Relaxed);
    let mut request = operations::ListColumnRowsRequest {
        selection: Some(ColumnSelection {
            source: Some(source.clone()),
            columns: vec!["payload.value".into()],
            predicate: Some(ColumnPredicate {
                column: "payload.value".into(),
                lower: Some(2.0),
                upper: None,
            }),
            work: Some(work),
            time: None,
            start: 0,
        }),
        at: None,
        page: Some(page.clone()),
    };
    let first = fixture.runtime.operations.call_native::<operations::ListColumnRows>(request.clone()).await.unwrap();
    let decoded = first.page.unwrap().batch;
    assert_eq!(decoded.num_rows(), 0);
    assert!(!first.next_page_token.is_empty());
    request.at = first.resolved_at.clone();
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let mut changed = request.clone();
    changed.selection.as_mut().unwrap().columns = vec!["_storage.item".into()];
    assert!(fixture.runtime.operations.call_native::<operations::ListColumnRows>(changed).await.is_err());
    let mut tampered = request.clone();
    tampered.page.as_mut().unwrap().page_token.push('0');
    assert!(fixture.runtime.operations.call_native::<operations::ListColumnRows>(tampered).await.is_err());
    let mut values = vec![];
    loop {
        let response = fixture
            .runtime
            .operations
            .call_native::<operations::ListColumnRows>(operations::ListColumnRowsRequest {
                selection: request.selection.clone(),
                at: request.at.clone(),
                page: request.page.clone(),
            })
            .await
            .unwrap();
        assert_eq!(response.resolved_at, request.at);
        let decoded = response.page.unwrap().batch;
        let column = decoded.column(0).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
        values.extend(column.values().iter().copied());
        if response.next_page_token.is_empty() {
            break;
        }
        request.page.as_mut().unwrap().page_token = response.next_page_token;
    }
    assert_eq!(values, vec![2, 3]);
    fixture.data.control.record_page_rows.store(0, Ordering::Relaxed);
    let window = fixture.storage.mutation_window().unwrap().id;
    let mutation = |key: &str| {
        Some(MutationId {
            window: window.clone(),
            key: Identifier::new(key).unwrap(),
        })
    };
    let definition = IndexDefinition {
        name: Identifier::new("ordered").unwrap(),
        version: 1,
        source: source.clone(),
        columns: vec!["payload.value".into()],
    };
    let created = fixture
        .runtime
        .operations
        .call_native::<operations::SetIndexDefinition>(operations::SetIndexDefinitionRequest {
            definition: Some(definition.clone()),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            work: Some(work),
            mutation: mutation("create-index"),
        })
        .await
        .unwrap();
    let mut index_request = operations::ListIndexEntriesRequest {
        selection: Some(IndexSelection {
            name: "ordered".into(),
            work: Some(work),
            lower: None,
            upper: None,
            prefix: Vec::new(),
            descending: false,
        }),
        source: Some(source.clone()),
        at: None,
        page: Some(page.clone()),
    };
    let indexed = fixture.runtime.operations.call_native::<operations::ListIndexEntries>(index_request.clone()).await.unwrap();
    assert_eq!(indexed.page.unwrap().entries.len(), 1);
    assert!(!indexed.next_page_token.is_empty());
    index_request.at = indexed.resolved_at;
    index_request.page.as_mut().unwrap().page_token = indexed.next_page_token;
    let next = fixture
        .runtime
        .operations
        .call_native::<operations::ListIndexEntries>(operations::ListIndexEntriesRequest {
            selection: index_request.selection.clone(),
            source: index_request.source.clone(),
            at: index_request.at.clone(),
            page: index_request.page.clone(),
        })
        .await
        .unwrap();
    assert_eq!(next.resolved_at, index_request.at);
    let mut rebuilt = definition;
    rebuilt.version += 1;
    fixture
        .runtime
        .operations
        .call_native::<operations::SetIndexDefinition>(operations::SetIndexDefinitionRequest {
            definition: Some(rebuilt),
            expected_revision: created.revision,
            create_if_absent: false,
            rebuild: true,
            work: Some(work),
            mutation: mutation("rebuild-index"),
        })
        .await
        .unwrap();
    let old_index_cut = index_request.at.clone();
    assert!(fixture.runtime.operations.call_native::<operations::ListIndexEntries>(index_request).await.is_err());
    assert!(
        fixture
            .runtime
            .operations
            .call_native::<operations::GetIndexDefinition>(operations::GetIndexDefinitionRequest {
                name: "ordered".into(),
                source: Some(source.clone()),
                at: old_index_cut,
            })
            .await
            .is_err()
    );
    let definition = ProjectionDefinition {
        name: Identifier::new("projected").unwrap(),
        version: 1,
        source: source.clone(),
        transform: ProjectionTransform::Select(vec!["payload.value".into(), "_storage.item".into()]),
    };
    let created = fixture
        .runtime
        .operations
        .call_native::<operations::SetProjectionDefinition>(operations::SetProjectionDefinitionRequest {
            definition: Some(definition),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            mutation: mutation("create-projection"),
        })
        .await
        .unwrap();
    let lease =
        fixture.runtime.operations.call_native::<operations::CreateSnapshot>(operations::CreateSnapshotRequest { at: None }).await.unwrap();
    let projected = fixture
        .runtime
        .operations
        .call_native::<operations::AdvanceProjection>(operations::AdvanceProjectionRequest {
            snapshot: lease.snapshot.clone(),
            source: Some(source.clone()),
            name: "projected".into(),
            work: Some(work),
        })
        .await
        .unwrap();
    let mut request = operations::ListProjectionRowsRequest {
        selection: Some(ProjectionSelection {
            name: "projected".into(),
            revision: created.revision,
            frontier: None,
            work: Some(work),
        }),
        source: Some(source),
        at: projected.resolved_at.clone(),
        page: Some(page),
    };
    let first = fixture.runtime.operations.call_native::<operations::ListProjectionRows>(request.clone()).await.unwrap();
    assert_eq!(first.resolved_at, projected.resolved_at);
    assert!(!first.next_page_token.is_empty());
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let next = fixture
        .runtime
        .operations
        .call_native::<operations::ListProjectionRows>(operations::ListProjectionRowsRequest {
            selection: request.selection,
            source: request.source,
            at: request.at.clone(),
            page: request.page,
        })
        .await
        .unwrap();
    assert_eq!(next.resolved_at, request.at);
    assert_ne!(first.page.unwrap().batch, next.page.unwrap().batch);
    fixture
        .runtime
        .operations
        .call_native::<operations::DeleteSnapshot>(operations::DeleteSnapshotRequest { snapshot: lease.snapshot })
        .await
        .unwrap();
    fixture.finish().await;
}
