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
use crate::support::{Bank, Ledger, app};
use arrow_array::UInt64Array;
use harmos_domain::schema::{Described, graph};
use harmos_domain::{MemorySize, Message};
use harmos_operations::storage;
use harmos_operations::storage::{
    ColumnKind, ColumnPredicate, ColumnSelection, ColumnSource, IndexDefinition, IndexSelection, ProjectionDefinition, ProjectionSelection,
    ProjectionTransform,
};
use harmos_protocol::model;
use harmos_query::arrow::{ArrowBatch, ArrowSchema};
use harmos_runtime::operations::{CallContext, Lane};
use harmos_runtime::storage::StorageManager;
use harmos_storage::{Identifier, MutationId, PayloadEncoding, ReadLimits, RecordInput, SchemaKey, StorageBackends};
use sha2::{Digest, Sha256};
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
    app::register(&fixture.runtime, harmos_operations::storage_catalog(), Lane::default()).await.unwrap();
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
    app::register(&fixture.runtime, harmos_operations::storage_catalog(), Lane::default()).await.unwrap();
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

#[tokio::test]
async fn discover_recovered_productions_settles_scope_through_the_facade() {
    // Start Storage and Create Productions Before Fencing Admission
    let fixture = Fixture::new(config()).await;
    app::register(&fixture.runtime, harmos_operations::storage_catalog(), Lane::default()).await.unwrap();
    let scope = Identifier::new("execution").unwrap();
    let call = fixture.runtime.for_call(&CallContext::default());
    let request = storage::InfoRequest {};
    let info = call.storage().info(request).await.unwrap();
    let window = info.mutations.unwrap().id;
    let mut created = None;
    for id in ["file-b", "file-a"] {
        let request = storage::CreateFileProductionRequest {
            metadata: Some(storage::FileMetadata {
                id: Identifier::new(id).unwrap(),
                scope: Some(scope.clone()),
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            }),
            mutation: Some(MutationId {
                window: window.clone(),
                key: Identifier::new(id).unwrap(),
            }),
        };
        let response = call.storage().create_file_production(request).await.unwrap();
        if id == "file-a" {
            created = response.production;
        }
        // The Other Committed Reply Is Lost Before the Consumer Retains Its Session
    }
    let created = created.unwrap();
    let request = storage::AppendFileBytesRequest {
        session: Some(created.session.clone()),
        offset: 0,
        data: b"prefix".to_vec(),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("append").unwrap(),
        }),
    };
    call.storage().append_file_bytes(request).await.unwrap();
    let request = storage::CreateFileWriterRequest {
        session: Some(created.session.clone()),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("resume").unwrap(),
        }),
    };
    let resumed = call.storage().create_file_writer(request).await.unwrap().production.unwrap();

    // Persist the Fixture's Prefix Before Observing Durable Progress
    fixture.storage.flush().await.unwrap();

    // Refuse Scope Deletion Before Settling Its Productions
    let mut deletion = storage::DeleteScopeRequest {
        scope: scope.clone(),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("delete-scope").unwrap(),
        }),
    };
    assert!(call.storage().delete_scope(deletion.clone()).await.is_err());
    let request = storage::ListFileProductionsRequest {
        scope: scope.clone(),
        at: None,
        page: Some(model::PageRequest {
            page_size: 1,
            max_bytes: 1024,
            page_token: String::new(),
        }),
    };
    let first = call.storage().list_file_productions(request.clone()).await.unwrap();
    assert_eq!(first.productions.len(), 1);
    assert!(first.memory_size() <= request.page.as_ref().unwrap().max_bytes as usize);
    assert_eq!(first.productions[0].session, resumed.session);
    assert_eq!(first.productions[0].accepted_length, 6);
    assert_eq!(first.productions[0].durable_length, 6);
    assert!(!first.next_page_token.is_empty());
    assert_eq!(call.storage().list_file_productions(request.clone()).await.unwrap(), first);
    let mut continuation = request.clone();
    continuation.page.as_mut().unwrap().page_token = first.next_page_token.clone();
    continuation.at = first.resolved_at.clone();
    let mut wrong_scope = continuation.clone();
    wrong_scope.scope = Identifier::new("execution.child").unwrap();
    assert!(call.storage().list_file_productions(wrong_scope).await.is_err());
    let mut wrong_cut = continuation.clone();
    wrong_cut.at.as_mut().unwrap().positions[0].sequence += 1;
    assert!(call.storage().list_file_productions(wrong_cut).await.is_err());
    let mut malformed = request.clone();
    malformed.page.as_mut().unwrap().page_token = "invalid".into();
    assert!(call.storage().list_file_productions(malformed).await.is_err());
    let mut tiny = request.clone();
    tiny.page.as_mut().unwrap().max_bytes = 1;
    assert!(call.storage().list_file_productions(tiny).await.is_err());
    drop(call);
    let data = fixture.data.clone();
    let recovery = fixture.recovery.clone();
    fixture.finish().await;

    // Reopen the Same Backend History With a Fresh Runtime and Recover Productions
    let manager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let runtime = app::start::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().storage(&manager).register(crate::support::catalog());
    })
    .await
    .unwrap();
    manager.start(&runtime).await.unwrap();
    app::register(&runtime, harmos_operations::storage_catalog(), Lane::default()).await.unwrap();
    let fixture = Fixture::from_parts(runtime, manager, data, recovery);
    let call = fixture.runtime.for_call(&CallContext::default());

    // Select the Reopened Owner's Retry Window for New Mutations
    let request = storage::InfoRequest {};
    let info = call.storage().info(request).await.unwrap();
    let window = info.mutations.unwrap().id;
    deletion.mutation = Some(MutationId {
        window: window.clone(),
        key: Identifier::new("delete-scope").unwrap(),
    });

    // Claim the Recovered Writer Before Publishing Its Durable Prefix
    let request = storage::CreateFileWriterRequest {
        session: Some(first.productions[0].session.clone()),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("claim-publish").unwrap(),
        }),
    };
    let claimed = call.storage().create_file_writer(request).await.unwrap().production.unwrap();
    assert!(claimed.session.generation > first.productions[0].session.generation);
    let request = storage::PublishFileProductionRequest {
        session: Some(claimed.session),
        length: 6,
        sha256: Sha256::digest(b"prefix").to_vec(),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("publish").unwrap(),
        }),
    };
    call.storage().publish_file_production(request).await.unwrap();

    // Resume the Issued Page After Restart and Abandon the Lost-Response Production
    let second = call.storage().list_file_productions(continuation).await.unwrap();
    assert_eq!(second.resolved_at, first.resolved_at);
    assert_eq!(second.productions.len(), 1);
    assert_eq!(second.productions[0].session.id.as_str(), "file-b");
    assert_eq!(second.productions[0].session.generation, 1);
    assert!(second.next_page_token.is_empty());
    let request = storage::CreateFileWriterRequest {
        session: Some(second.productions[0].session.clone()),
        mutation: Some(MutationId {
            window: window.clone(),
            key: Identifier::new("claim-abandon").unwrap(),
        }),
    };
    let claimed = call.storage().create_file_writer(request).await.unwrap().production.unwrap();
    assert!(claimed.session.generation > second.productions[0].session.generation);
    let request = storage::DeleteFileProductionRequest {
        session: Some(claimed.session),
        mutation: Some(MutationId {
            window,
            key: Identifier::new("abandon").unwrap(),
        }),
    };
    call.storage().delete_file_production(request).await.unwrap();
    let request = storage::ListFileProductionsRequest { scope, page: None, at: None };
    assert!(call.storage().list_file_productions(request).await.unwrap().productions.is_empty());

    // Delete the Settled Scope Through the Same Previously Refused Operation
    let response = call.storage().delete_scope(deletion).await.unwrap();
    assert!(matches!(response.outcome, storage::CompoundOutcome::Applied { .. }));
    drop(call);
    fixture.finish().await;
}
