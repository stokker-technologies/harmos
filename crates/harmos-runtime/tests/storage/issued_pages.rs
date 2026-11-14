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
use harmos_operations::storage as operations;
use harmos_protocol::v1;
use harmos_query::QueryOutput;
use harmos_storage::query::RecordSelection;
use harmos_storage::query::Source;
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
async fn issued_pages_keep_their_cut_while_newer_writes_are_blocked() {
    let mut settings = config();
    settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
    let maximum = settings.response.batch_bytes.get();
    let fixture = Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("page-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    for value in 0..4 {
        fixture
            .storage
            .append_records(vec![RecordInput {
                schema: schema.clone(),
                scope: None,
                item: Identifier::new(format!("row-{value}")).unwrap(),
                source: None,
                source_time: None,
                payload: Reading { value }.encode(),
            }])
            .await
            .unwrap();
    }
    let original = fixture.storage.flush().await.unwrap();
    let saved =
        fixture.runtime.operations.call_native::<operations::CreateSnapshot>(operations::CreateSnapshotRequest { at: None }).await.unwrap();
    let cut = saved.resolved_at.clone().unwrap();
    let source = Some(Source::Records(RecordSelection {
        schema: schema.clone(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    }));
    let selection = operations::ListRowsRequest {
            merge: None,
        source: source.clone(),
        at: Some(cut.clone()),
        transforms: vec![],
        page: Some(v1::PageRequest {
            page_size: 2,
            max_bytes: maximum as u64,
            page_token: String::new(),
        }),
    };
    let first = fixture.runtime.operations.call_native::<operations::ListRows>(selection.clone()).await.unwrap();
    assert_eq!(first.resolved_at, Some(cut.clone()));
    assert!(!first.next_page_token.is_empty());

    // Commit A Newer Position Without Issuing Its Cut, Then Block A Later Write.
    fixture
        .storage
        .append_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("later").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 99 }.encode(),
        }])
        .await
        .unwrap();
    let newer = fixture.storage.flush().await.unwrap();
    fixture.data.control.block_write.store(true, Ordering::SeqCst);
    fixture
        .storage
        .append_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("blocked").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 100 }.encode(),
        }])
        .await
        .unwrap();
    let storage = fixture.storage.clone();
    let flush = tokio::spawn(async move { storage.flush().await });
    fixture.data.control.write_entered.notified().await;
    let mut current = fixture
        .runtime
        .operations
        .call_native::<operations::ListRows>(operations::ListRowsRequest {
            merge: None, at: None, ..selection.clone() })
        .await
        .unwrap();
    assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, newer.sequence);
    assert_eq!(fixture.storage.get_saved_position(), newer);
    let mut durable_rows = 0;
    loop {
        assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, newer.sequence);
        for row in &current.rows {
            if let QueryOutput::Batch(bytes) = row {
                durable_rows += harmos_storage::arrow::ArrowBatch::from_ipc(bytes.clone()).unwrap().as_record_batch().num_rows();
            }
        }
        if current.next_page_token.is_empty() {
            break;
        }
        current = fixture
            .runtime
            .operations
            .call_native::<operations::ListRows>(operations::ListRowsRequest {
            merge: None,
                at: None,
                page: Some(v1::PageRequest {
                    page_token: current.next_page_token,
                    ..selection.page.clone().unwrap()
                }),
                ..selection.clone()
            })
            .await
            .unwrap();
    }
    assert_eq!(durable_rows, 5);

    // Replay The Native Continuation Without Changing Its Page.
    let request = operations::ListRowsRequest {
            merge: None,
        source: source.clone(),
        at: Some(cut.clone()),
        transforms: vec![],
        page: Some(v1::PageRequest {
            page_token: first.next_page_token.clone(),
            ..selection.page.clone().unwrap()
        }),
    };
    let second = fixture.runtime.operations.call_native::<operations::ListRows>(request.clone()).await.unwrap();
    let replay = fixture
        .runtime
        .operations
        .call_native::<operations::ListRows>(operations::ListRowsRequest {
            merge: None,
            page: request.page.clone(),
            ..selection.clone()
        })
        .await
        .unwrap();
    assert_eq!(replay.rows, second.rows);
    assert_eq!(replay.next_page_token, second.next_page_token);
    assert_eq!(second.resolved_at, Some(cut.clone()));
    // Tokens Bind The Selector, Owner And Complete Cut, Including Revisions.
    let mut changed = selection.clone();
    changed.source = Some(Source::Records(RecordSelection {
        schema: schema.clone(),
        scope: None,
        item: Some(Identifier::new("different").unwrap()),
        source_start: None,
        source_end: None,
    }));
    changed.page.as_mut().unwrap().page_token = first.next_page_token.clone();
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(changed).await.is_err());
    let mut tampered = selection.clone();
    tampered.page.as_mut().unwrap().page_token = "é".repeat(104);
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(tampered).await.is_err());
    let mut foreign = selection.clone();
    foreign.at.as_mut().unwrap().positions[0].history.as_mut().unwrap().epoch = "foreign".into();
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(foreign).await.is_err());
    let mut unissued = selection.clone();
    unissued.at.as_mut().unwrap().revisions.push(v1::ResourceRevision {
        name: "unissued".into(),
        revision: vec![1],
    });
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(unissued).await.is_err());

    let mut resume = request.clone();
    resume.page.as_mut().unwrap().page_token = second.next_page_token;
    let mut count = 0;
    loop {
        let response = fixture.runtime.operations.call_native::<operations::ListRows>(resume.clone()).await.unwrap();
        assert_eq!(response.resolved_at, Some(cut.clone()));
        for row in response.rows {
            if let QueryOutput::Batch(bytes) = row {
                count += harmos_storage::arrow::ArrowBatch::from_ipc(bytes).unwrap().as_record_batch().num_rows();
            }
        }
        if response.next_page_token.is_empty() {
            break;
        }
        resume.page.as_mut().unwrap().page_token = response.next_page_token;
    }
    assert_eq!(count, 4);
    assert_eq!(cut.positions[0].sequence, original.sequence);
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(resume.clone()).await.unwrap().next_page_token.is_empty());
    fixture.data.control.block_write.store(false, Ordering::SeqCst);
    fixture.data.control.write_release.notify_waiters();
    let latest = flush.await.unwrap().unwrap();
    let current = fixture
        .runtime
        .operations
        .call_native::<operations::ListRows>(operations::ListRowsRequest {
            merge: None, at: None, ..selection.clone() })
        .await
        .unwrap();
    assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, latest.sequence);
    fixture
        .runtime
        .operations
        .call_native::<operations::DeleteSnapshot>(operations::DeleteSnapshotRequest { snapshot: saved.snapshot })
        .await
        .unwrap();
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(resume).await.is_err());
    assert!(fixture.runtime.operations.call_native::<operations::ListRows>(selection).await.is_err());
    fixture.finish().await;
}
