/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Continue Private Native Lists at Their Issued Composite Cuts

/* === Modules === */

mod compound_mutation_through_operations;

/* === Imports === */

use super::{Fixture, config};
use arrow_array::UInt64Array;
use harmos_domain::Message;
use harmos_domain::schema::{Described, graph};
use harmos_operations::storage;
use harmos_operations::storage::{
    ColumnKind, ColumnPredicate, ColumnSelection, ColumnSource, IndexDefinition, IndexSelection, ProjectionDefinition, ProjectionSelection,
    ProjectionTransform,
};
use harmos_protocol::model;
use harmos_query::arrow::{ArrowBatch, ArrowSchema};
use harmos_storage::{Identifier, MutationId, PayloadEncoding, ReadLimits, RecordInput, SchemaKey};
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
async fn native_text_predicates_bind_pages_to_the_prefix_and_read_cut() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("text-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = graph::Schema::from_declaration(Reading::DEFINITION, &Default::default()).unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    for (index, item) in ["other-first", "group-first", "group-last", "other-last"].into_iter().enumerate() {
        fixture
            .storage
            .append_records(vec![RecordInput {
                schema: schema.clone(),
                scope: None,
                item: Identifier::new(item).unwrap(),
                source: None,
                source_time: None,
                payload: Reading { value: index as u64 }.encode(),
            }])
            .await
            .unwrap();
    }
    fixture.storage.flush().await.unwrap();
    let mut request = storage::ListColumnRowsRequest {
        selection: Some(ColumnSelection {
            source: Some(ColumnSource {
                kind: ColumnKind::Records,
                schema: schema.clone(),
            }),
            columns: vec!["payload.value".into()],
            predicate: Some(ColumnPredicate {
                column: "_storage.item".into(),
                lower: None,
                upper: None,
                prefix: Some("group-".into()),
            }),
            time: None,
            start: 0,
            work: Some(ReadLimits {
                rows: NonZeroUsize::new(64).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            }),
        }),
        at: None,
        page: Some(model::PageRequest {
            page_size: 2,
            max_bytes: 8192,
            page_token: String::new(),
        }),
    };
    let first = fixture.runtime.operations.call_native::<storage::ListColumnRows>(request.clone()).await.unwrap();
    assert_eq!(first.page.as_ref().unwrap().payload_rows, 1);
    assert!(!first.next_page_token.is_empty());
    request.at = first.resolved_at;
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let mut changed = request.clone();
    changed.selection.as_mut().unwrap().predicate.as_mut().unwrap().prefix = Some("other-".into());
    assert!(fixture.runtime.operations.call_native::<storage::ListColumnRows>(changed).await.is_err());

    fixture
        .storage
        .append_records(vec![RecordInput {
            schema,
            scope: None,
            item: Identifier::new("group-later").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 99 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let second = fixture.runtime.operations.call_native::<storage::ListColumnRows>(request.clone()).await.unwrap();
    assert_eq!(second.resolved_at, request.at);
    let page = second.page.unwrap();
    assert_eq!(page.payload_rows, 1);
    let batch = ArrowBatch::from_ipc(page.batch.data).unwrap();
    let values = batch.as_record_batch().column(0).as_any().downcast_ref::<UInt64Array>().unwrap();
    assert_eq!(values.values(), &[2]);
    assert!(second.next_page_token.is_empty());

    request.page.as_mut().unwrap().page_token.clear();
    for predicate in [
        ColumnPredicate {
            column: "_storage.item".into(),
            lower: Some(1.0),
            upper: None,
            prefix: Some("group-".into()),
        },
        ColumnPredicate {
            column: "_storage.item".into(),
            lower: None,
            upper: None,
            prefix: Some(String::new()),
        },
        ColumnPredicate {
            column: "payload.value".into(),
            lower: None,
            upper: None,
            prefix: Some("group-".into()),
        },
        ColumnPredicate {
            column: "_storage.item".into(),
            lower: Some(1.0),
            upper: None,
            prefix: None,
        },
    ] {
        request.selection.as_mut().unwrap().predicate = Some(predicate);
        assert!(fixture.runtime.operations.call_native::<storage::ListColumnRows>(request.clone()).await.is_err());
    }
    fixture.finish().await;
}

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
    let graph = graph::Schema::from_declaration(Reading::DEFINITION, &Default::default()).unwrap();
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
    let page = model::PageRequest {
        page_size: 1,
        max_bytes: 8192,
        page_token: String::new(),
    };
    fixture.data.control.record_page_rows.store(1, Ordering::Relaxed);
    let mut request = storage::ListColumnRowsRequest {
        selection: Some(ColumnSelection {
            source: Some(source.clone()),
            columns: vec!["payload.value".into()],
            predicate: Some(ColumnPredicate {
                column: "payload.value".into(),
                lower: Some(2.0),
                upper: None,
                prefix: None,
            }),
            work: Some(work),
            time: None,
            start: 0,
        }),
        at: None,
        page: Some(page.clone()),
    };
    let first = fixture.runtime.operations.call_native::<storage::ListColumnRows>(request.clone()).await.unwrap();
    let batch = first.page.unwrap().batch;
    let schema = ArrowSchema::from_ipc(batch.schema).unwrap();
    let batch = ArrowBatch::from_ipc(batch.data).unwrap();
    batch.validate_schema(schema.as_schema()).unwrap();
    let decoded = batch.as_record_batch();
    assert_eq!(decoded.num_rows(), 0);
    assert!(!first.next_page_token.is_empty());
    request.at = first.resolved_at.clone();
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let mut changed = request.clone();
    changed.selection.as_mut().unwrap().columns = vec!["_storage.item".into()];
    assert!(fixture.runtime.operations.call_native::<storage::ListColumnRows>(changed).await.is_err());
    let mut tampered = request.clone();
    tampered.page.as_mut().unwrap().page_token.push('0');
    assert!(fixture.runtime.operations.call_native::<storage::ListColumnRows>(tampered).await.is_err());
    let mut values = vec![];
    loop {
        let response = fixture
            .runtime
            .operations
            .call_native::<storage::ListColumnRows>(storage::ListColumnRowsRequest {
                selection: request.selection.clone(),
                at: request.at.clone(),
                page: request.page.clone(),
            })
            .await
            .unwrap();
        assert_eq!(response.resolved_at, request.at);
        let batch = response.page.unwrap().batch;
        let schema = ArrowSchema::from_ipc(batch.schema).unwrap();
        let batch = ArrowBatch::from_ipc(batch.data).unwrap();
        batch.validate_schema(schema.as_schema()).unwrap();
        let decoded = batch.as_record_batch();
        let column = decoded.column(0).as_any().downcast_ref::<UInt64Array>().unwrap();
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
        .call_native::<storage::SetIndexDefinition>(storage::SetIndexDefinitionRequest {
            definition: Some(definition.clone()),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            work: Some(work),
            mutation: mutation("create-index"),
        })
        .await
        .unwrap();
    let mut index_request = storage::ListIndexEntriesRequest {
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
    let indexed = fixture.runtime.operations.call_native::<storage::ListIndexEntries>(index_request.clone()).await.unwrap();
    assert_eq!(indexed.page.unwrap().entries.len(), 1);
    assert!(!indexed.next_page_token.is_empty());
    index_request.at = indexed.resolved_at;
    index_request.page.as_mut().unwrap().page_token = indexed.next_page_token;
    let next = fixture
        .runtime
        .operations
        .call_native::<storage::ListIndexEntries>(storage::ListIndexEntriesRequest {
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
        .call_native::<storage::SetIndexDefinition>(storage::SetIndexDefinitionRequest {
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
    assert!(fixture.runtime.operations.call_native::<storage::ListIndexEntries>(index_request).await.is_err());
    assert!(
        fixture
            .runtime
            .operations
            .call_native::<storage::GetIndexDefinition>(storage::GetIndexDefinitionRequest {
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
        .call_native::<storage::SetProjectionDefinition>(storage::SetProjectionDefinitionRequest {
            definition: Some(definition),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            mutation: mutation("create-projection"),
        })
        .await
        .unwrap();
    let lease =
        fixture.runtime.operations.call_native::<storage::CreateSnapshot>(storage::CreateSnapshotRequest { at: None }).await.unwrap();
    let projected = fixture
        .runtime
        .operations
        .call_native::<storage::AdvanceProjection>(storage::AdvanceProjectionRequest {
            snapshot: lease.snapshot.clone(),
            source: Some(source.clone()),
            name: "projected".into(),
            work: Some(work),
        })
        .await
        .unwrap();
    let status = fixture
        .runtime
        .operations
        .call_native::<storage::GetProjectionStatus>(storage::GetProjectionStatusRequest {
            name: "projected".into(),
            revision: created.revision.clone(),
            at: projected.resolved_at.clone(),
            source: Some(source.clone()),
        })
        .await
        .unwrap();
    assert_eq!(status.resolved_at, projected.resolved_at);
    let status = status.status.unwrap();
    assert_eq!(status.rows, 4);
    assert_eq!(status.changed_keys, 0);
    let mut request = storage::ListProjectionRowsRequest {
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
    let first = fixture.runtime.operations.call_native::<storage::ListProjectionRows>(request.clone()).await.unwrap();
    assert_eq!(first.resolved_at, projected.resolved_at);
    assert!(!first.next_page_token.is_empty());
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let next = fixture
        .runtime
        .operations
        .call_native::<storage::ListProjectionRows>(storage::ListProjectionRowsRequest {
            selection: request.selection,
            source: request.source,
            at: request.at.clone(),
            page: request.page,
        })
        .await
        .unwrap();
    assert_eq!(next.resolved_at, request.at);
    let first = first.page.unwrap().batch;
    let schema = ArrowSchema::from_ipc(first.schema).unwrap();
    let first = ArrowBatch::from_ipc(first.data).unwrap();
    first.validate_schema(schema.as_schema()).unwrap();

    let next = next.page.unwrap().batch;
    let schema = ArrowSchema::from_ipc(next.schema).unwrap();
    let next = ArrowBatch::from_ipc(next.data).unwrap();
    next.validate_schema(schema.as_schema()).unwrap();
    assert_ne!(first.as_record_batch(), next.as_record_batch());
    fixture
        .runtime
        .operations
        .call_native::<storage::DeleteSnapshot>(storage::DeleteSnapshotRequest { snapshot: lease.snapshot })
        .await
        .unwrap();
    fixture.finish().await;
}
