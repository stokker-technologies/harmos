/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Follow Durable Rows Across Opening, Filtering, Child Calls And Retention Loss

use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use harmos_runtime::operations::{
    StreamSettings,
    authorization::{Authorization, Authorizer, Provider},
};
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};

#[derive(harmos_domain::Message)]
struct Reading {
    first: u32,
    second: u32,
    third: u32,
}

struct Authority;
impl Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: Provider<'_>,
    ) -> harmos_runtime::Result<Option<Authorization>> {
        Ok(Some(Authorization::Full(provider.contract.clone())))
    }
}

#[tokio::test]
async fn watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss() {
    let mut settings = super::config();
    settings.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
    let fixture = super::Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, operations::storage_catalog(), Default::default()).await.unwrap();
    fixture.runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(super::schema(), graph).await.unwrap();
    fixture.storage.flush().await.unwrap();
    let listener = fixture.runtime.grpc().attach(Default::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let source = wire::QuerySource {
        source: Some(wire::query_source::Source::Records(wire::RecordSelection {
            schema: Some(super::schema().into()),
            scope: None,
            item: Some("target".into()),
            source_start: None,
            source_end: None,
        })),
    };
    let mut grpc = tonic::client::Grpc::new(channel.clone());
    grpc.ready().await.unwrap();
    fixture.data.control.block_view.store(true, Ordering::Release);
    let request = v1::StorageWatchRowsRequest { source: Some(source.clone()), at: None };
    let mut opening = Box::pin(async {
        let mut response: tonic::Streaming<v1::StorageWatchRowsResponse> = grpc
            .server_streaming(
                tonic::Request::new(request),
                tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchRows"),
                tonic_prost::ProstCodec::default(),
            )
            .await
            .unwrap()
            .into_inner();
        let first = response.message().await.unwrap().unwrap();
        (response, first)
    });
    assert!(crate::support::events::until(opening.as_mut(), fixture.data.control.view_entered.notified()).await.is_ok());
    let raced = super::write(&fixture.storage, "target").await;
    fixture.storage.flush().await.unwrap();
    fixture.data.control.block_view.store(false, Ordering::Release);
    fixture.data.control.view_release.notify_one();
    let (mut client, boundary) = opening.await;
    use v1::storage_watch_rows_response::Event;
    assert!(matches!(boundary.event, Some(Event::Boundary(_))));
    assert!(boundary.resolved_at.unwrap().positions[0].sequence < raced.sequence);
    let first = client.message().await.unwrap().unwrap();
    let Some(Event::Record(row)) = first.event else {
        panic!("expected the commit during opening");
    };
    assert_eq!(row.position.unwrap().sequence, raced.sequence);

    fixture.data.control.block_write.store(true, Ordering::Release);
    let accepted = super::write(&fixture.storage, "target").await;
    let mut flush = Box::pin(fixture.storage.flush_until(&accepted));
    assert!(crate::support::events::until(flush.as_mut(), fixture.data.control.write_entered.notified()).await.is_ok());
    let mut pending = Box::pin(client.message());
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(pending.as_mut(), context).is_pending())).await
    );
    fixture.data.control.block_write.store(false, Ordering::Release);
    fixture.data.control.write_release.notify_one();
    flush.await.unwrap();
    let durable = pending.await.unwrap().unwrap();
    let Some(Event::Record(row)) = durable.event else {
        panic!("expected the newly committed record");
    };
    assert_eq!(row.position.unwrap().sequence, accepted.sequence);
    let unmatched = super::write(&fixture.storage, "unmatched").await;
    fixture.storage.flush().await.unwrap();
    let progress = client.message().await.unwrap().unwrap();
    assert!(matches!(progress.event, Some(Event::DurableProgress(_))));
    assert_eq!(progress.resolved_at.unwrap().positions[0].sequence, unmatched.sequence);
    drop(client);

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
    let mut watching = Box::pin(super::transfer_plugin::call::<context::WatchRows>(
        &fixture.runtime.operations,
        child::StorageWatchRowsRequest { source: Some(source.clone()), at: None },
    ));
    assert!(crate::support::events::until(watching.as_mut(), plugin.watch_opened()).await.is_ok());
    let child_position = super::write(&fixture.storage, "target").await;
    fixture.storage.flush().await.unwrap();
    let observed = watching.await.unwrap();
    let Some(child::storage_watch_rows_response::Event::Record(row)) = observed.event else {
        panic!("expected the child's durable record");
    };
    assert_eq!(row.position.unwrap().sequence, child_position.sequence);
    assert_eq!(observed.resolved_at.unwrap().positions[0].sequence, child_position.sequence);
    attached.stop("durable watch child complete").await.unwrap();
    plugin.stop().await;

    let limits = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut slow = fixture
        .runtime
        .operations
        .download_protobuf::<operations::WatchRows>(v1::StorageWatchRowsRequest { source: Some(source.clone()), at: None }, limits)
        .unwrap();
    let opening = slow.next().await.unwrap().unwrap();
    for _ in 0..6 {
        super::write(&fixture.storage, "target").await;
        fixture.storage.flush().await.unwrap();
    }
    let resync = loop {
        let frame = slow.next().await.unwrap().unwrap();
        if let Some(Event::ResyncRequired(error)) = frame.event {
            break (error, frame.resolved_at.unwrap());
        }
    };
    assert_eq!(resync.0.kind, wire::StorageFailureKind::StorageFailureGap as i32);
    assert!(resync.1.positions[0].sequence >= opening.resolved_at.unwrap().positions[0].sequence);
    assert!(slow.next().await.unwrap().is_none());
    slow.join().await.unwrap();
    // A Fresh Read Supplies The New Boundary After A Gap.
    let fresh = fixture
        .runtime
        .operations
        .call_protobuf::<operations::ListRows>(v1::StorageListRowsRequest {
            source: Some(source.clone()),
            transforms: vec![],
            at: None,
            page: None,
        })
        .await
        .unwrap();
    let mut resumed = fixture
        .runtime
        .operations
        .download_protobuf::<operations::WatchRows>(
            v1::StorageWatchRowsRequest {
                source: Some(source),
                at: fresh.resolved_at.clone(),
            },
            limits,
        )
        .unwrap();
    assert_eq!(resumed.next().await.unwrap().unwrap().resolved_at, fresh.resolved_at);
    resumed.request_cancel();
    assert!(resumed.join().await.is_err());
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
