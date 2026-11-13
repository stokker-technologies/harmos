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
use harmos_operations::storage as operations;
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{Identifier, PayloadEncoding, RecordInput, SchemaKey};
use std::num::{NonZeroU32, NonZeroUsize};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}

/* === Tests === */

#[tokio::test]
async fn cached_cuts_expire_while_explicit_leases_pin_their_image() {
    let mut settings = config();
    settings.request.max_active_queries = NonZeroUsize::MIN;
    settings.response.batch_rows = NonZeroUsize::MIN;
    let fixture = Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("lease-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    fixture
        .storage
        .append_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("first").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 1 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let selection = v1::StorageListRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Records(wire::RecordSelection {
                schema: Some(schema.clone().into()),
                ..Default::default()
            })),
        }),
        at: None,
        transforms: vec![],
        page: None,
    };
    let local_query = harmos_storage::Query {
        at: fixture.storage.get_saved_position(),
        source: harmos_storage::query::Source::Records(harmos_storage::query::RecordSelection {
            schema: schema.clone(),
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: Default::default(),
    };
    let image = std::sync::Arc::new(fixture.storage.snapshot(&local_query.at).await.unwrap());
    let old = fixture.runtime.operations.call_protobuf::<operations::ListRows>(selection.clone()).await.unwrap();
    // Construction Against A Preopened Image Performs No I/O: The Paused Page
    // Must Leave The Only Active-Query Slot Available To Another Native Reader.
    tokio::task::yield_now().await;
    let mut native = Box::pin(fixture.storage.query_snapshot(local_query, image));
    let ready = std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(native.as_mut(), cx))).await;
    let std::task::Poll::Ready(Ok(native_reader)) = ready else {
        panic!("paused page retained active execution");
    };
    drop(native_reader);
    drop(native);

    fixture
        .storage
        .append_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("second").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 2 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let current = fixture.runtime.operations.call_protobuf::<operations::ListRows>(selection.clone()).await.unwrap();
    assert_ne!(current.resolved_at, old.resolved_at);
    assert!(
        fixture
            .runtime
            .operations
            .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest { at: old.resolved_at, ..selection.clone() })
            .await
            .is_err()
    );
    assert!(
        fixture
            .runtime
            .operations
            .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest {
                page: Some(v1::PageRequest {
                    page_token: old.next_page_token,
                    ..Default::default()
                }),
                ..selection.clone()
            })
            .await
            .is_err()
    );
    let pinned = fixture
        .runtime
        .operations
        .call_protobuf::<operations::CreateSnapshot>(v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    fixture
        .storage
        .append_records(vec![RecordInput {
            schema,
            scope: None,
            item: Identifier::new("third").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 3 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    assert!(fixture.runtime.operations.call_protobuf::<operations::ListRows>(selection.clone()).await.is_err());
    let retained = fixture
        .runtime
        .operations
        .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest {
            at: pinned.resolved_at.clone(),
            ..selection.clone()
        })
        .await
        .unwrap();
    assert_eq!(retained.resolved_at, pinned.resolved_at);
    fixture
        .runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(v1::StorageDeleteSnapshotRequest { snapshot: pinned.snapshot })
        .await
        .unwrap();
    let fresh = fixture.runtime.operations.call_protobuf::<operations::ListRows>(selection.clone()).await.unwrap();
    assert_ne!(fresh.resolved_at, retained.resolved_at);
    let other = Fixture::new(config()).await;
    crate::support::app::register(&other.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    assert!(
        other
            .runtime
            .operations
            .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest {
                at: fresh.resolved_at,
                ..selection.clone()
            })
            .await
            .is_err()
    );
    assert!(
        other
            .runtime
            .operations
            .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest {
                page: Some(v1::PageRequest {
                    page_token: fresh.next_page_token,
                    ..Default::default()
                }),
                ..selection
            })
            .await
            .is_err()
    );
    other.finish().await;
    fixture.finish().await;
}
