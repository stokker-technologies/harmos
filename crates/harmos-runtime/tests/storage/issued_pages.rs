/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Unary Transfers Across Audience Faces

// Resume Saved Pages Across Faces While A Later Commit Is Blocked

/* === Imports === */

use super::{Fixture, config};
use harmos_domain::Message;
use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use harmos_storage::{Identifier, PayloadEncoding, RecordInput, SchemaKey};
use prost::Message as _;
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, atomic::Ordering},
};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}
struct AllowRows;

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for AllowRows {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: vec![harmos_operations::plugin::Grant::of(
                    provider.contract.operation().id.to_string(),
                    "records:schema:page-reading",
                )],
            },
        )))
    }
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
    fixture.runtime.operations.register_authorizer(Arc::new(AllowRows)).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = fixture
        .runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let listener = fixture.runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
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
    let saved = super::transfer_plugin::client_call::<operations::CreateSnapshot>(&channel, v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    let cut = saved.resolved_at.clone().unwrap();
    let source = Some(wire::QuerySource {
        source: Some(wire::query_source::Source::Records(wire::RecordSelection {
            schema: Some(schema.clone().into()),
            ..Default::default()
        })),
    });
    let selection = v1::StorageListRowsRequest {
        source: source.clone(),
        at: Some(cut.clone()),
        transforms: vec![],
        page: Some(v1::PageRequest {
            page_size: 2,
            max_bytes: maximum as u64,
            page_token: String::new(),
        }),
    };
    let first = super::transfer_plugin::client_call::<operations::ListRows>(&channel, selection.clone()).await.unwrap();
    assert_eq!(first.resolved_at, Some(cut.clone()));
    assert!(!first.next_page_token.is_empty());
    assert!(first.encoded_len() <= maximum);

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
    let mut current =
        super::transfer_plugin::client_call::<operations::ListRows>(&channel, v1::StorageListRowsRequest { at: None, ..selection.clone() })
            .await
            .unwrap();
    assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, newer.sequence);
    assert_eq!(fixture.storage.get_saved_position(), newer);
    let mut durable_rows = 0;
    loop {
        assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, newer.sequence);
        for row in &current.rows {
            if let Some(wire::query_output::Output::Batch(bytes)) = &row.output {
                durable_rows += harmos_storage::arrow::ArrowBatch::from_ipc(bytes.clone()).unwrap().as_record_batch().num_rows();
            }
        }
        if current.next_page_token.is_empty() {
            break;
        }
        current = super::transfer_plugin::client_call::<operations::ListRows>(
            &channel,
            v1::StorageListRowsRequest {
                at: None,
                page: Some(v1::PageRequest {
                    page_token: current.next_page_token,
                    ..selection.page.clone().unwrap()
                }),
                ..selection.clone()
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(durable_rows, 5);

    // Resume On The Plugin Face; Replaying The Token Returns The Same Page.
    let request = child::StorageListRowsRequest {
        source: source.clone(),
        at: Some(cut.clone()),
        transforms: vec![],
        page: Some(v1::PageRequest {
            page_token: first.next_page_token.clone(),
            ..selection.page.clone().unwrap()
        }),
    };
    let second = super::transfer_plugin::call::<context::ListRows>(&fixture.runtime.operations, request.clone()).await.unwrap();
    let replay = super::transfer_plugin::client_call::<operations::ListRows>(
        &channel,
        v1::StorageListRowsRequest {
            page: request.page.clone(),
            ..selection.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(replay.rows, second.rows);
    assert_eq!(replay.next_page_token, second.next_page_token);
    assert_eq!(second.resolved_at, Some(cut.clone()));
    assert!(second.encoded_len() <= maximum);
    // Tokens Bind The Selector, Owner And Complete Cut, Including Revisions.
    let mut changed = selection.clone();
    changed.source.as_mut().unwrap().source = Some(wire::query_source::Source::Records(wire::RecordSelection {
        schema: Some(schema.clone().into()),
        item: Some("different".into()),
        ..Default::default()
    }));
    changed.page.as_mut().unwrap().page_token = first.next_page_token.clone();
    assert!(super::transfer_plugin::client_call::<operations::ListRows>(&channel, changed).await.is_err());
    let mut tampered = selection.clone();
    tampered.page.as_mut().unwrap().page_token = "é".repeat(104);
    assert!(super::transfer_plugin::client_call::<operations::ListRows>(&channel, tampered).await.is_err());
    let mut foreign = selection.clone();
    foreign.at.as_mut().unwrap().positions[0].history.as_mut().unwrap().epoch = "foreign".into();
    assert!(super::transfer_plugin::client_call::<operations::ListRows>(&channel, foreign).await.is_err());
    let mut unissued = selection.clone();
    unissued.at.as_mut().unwrap().revisions.push(v1::ResourceRevision {
        name: "unissued".into(),
        revision: vec![1],
    });
    assert!(super::transfer_plugin::client_call::<operations::ListRows>(&channel, unissued).await.is_err());

    let mut resume = request.clone();
    resume.page.as_mut().unwrap().page_token = second.next_page_token;
    let mut count = 0;
    loop {
        let response = super::transfer_plugin::call::<context::ListRows>(&fixture.runtime.operations, resume.clone()).await.unwrap();
        assert_eq!(response.resolved_at, Some(cut.clone()));
        assert!(response.encoded_len() <= maximum);
        for row in response.rows {
            if let Some(wire::query_output::Output::Batch(bytes)) = row.output {
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
    assert!(
        super::transfer_plugin::call::<context::ListRows>(&fixture.runtime.operations, resume.clone())
            .await
            .unwrap()
            .next_page_token
            .is_empty()
    );
    fixture.data.control.block_write.store(false, Ordering::SeqCst);
    fixture.data.control.write_release.notify_waiters();
    let latest = flush.await.unwrap().unwrap();
    let current =
        super::transfer_plugin::client_call::<operations::ListRows>(&channel, v1::StorageListRowsRequest { at: None, ..selection.clone() })
            .await
            .unwrap();
    assert_eq!(current.resolved_at.as_ref().unwrap().positions[0].sequence, latest.sequence);
    super::transfer_plugin::client_call::<operations::DeleteSnapshot>(
        &channel,
        v1::StorageDeleteSnapshotRequest { snapshot: saved.snapshot },
    )
    .await
    .unwrap();
    assert!(super::transfer_plugin::call::<context::ListRows>(&fixture.runtime.operations, resume).await.is_err());
    assert!(super::transfer_plugin::client_call::<operations::ListRows>(&channel, selection).await.is_err());
    attached.stop("page story complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
