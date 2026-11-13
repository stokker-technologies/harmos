/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Keep A Plugin Child Policy Override Local To One Storage Call

use crate::support::clock;

/* === Imports === */

use harmos_domain::Message;
use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use harmos_storage::{Identifier, PayloadEncoding, RecordInput, SchemaKey, StorageBackends};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}
struct Authority;

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: vec![harmos_operations::plugin::Grant::of(
                    provider.contract.operation().id.to_string(),
                    "records:schema:policy-reading",
                )],
            },
        )))
    }
}

/* === Tests === */

#[tokio::test]
async fn a_child_deadline_override_does_not_change_the_next_storage_call() {
    let clock = clock::Controlled::new();
    let data = Arc::new(super::Data::new(super::history()));
    let storage = harmos_runtime::storage::StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(super::Recovery::new(super::history())),
        },
        super::config(),
    )
    .unwrap();
    let runtime = crate::support::app::launch::<super::Bank>(super::Ledger::default(), (), |setup| {
        setup.resource(std::ops::Deref::deref(&storage).clone());
        setup.resource(operations::SavedSnapshots::new(storage.query_capacity()));
        setup.settings().clock(clock.source()).register(super::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    storage.start(&runtime).await.unwrap();
    crate::support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("policy-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    storage.write_schema(schema.clone(), graph).await.unwrap();
    storage
        .append_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("row").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 7 }.encode(),
        }])
        .await
        .unwrap();
    storage.flush().await.unwrap();
    runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: Duration::from_secs(30),
                reconnect_delay: Duration::from_secs(1),
                stop_grace: Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let query: wire::Query = harmos_storage::Query {
        at: storage.get_saved_position(),
        source: harmos_storage::query::Source::Records(harmos_storage::query::RecordSelection {
            schema,
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: Default::default(),
    }
    .try_into()
    .unwrap();
    let request = v1::StorageListRowsRequest {
        source: query.source.clone(),
        transforms: vec![],
        at: None,
        page: None,
    };
    let first = runtime.operations.call_protobuf::<operations::ListRows>(request.clone()).await.unwrap();
    let second = runtime.operations.call_protobuf::<operations::ListRows>(request).await.unwrap();
    let short = Duration::from_millis(3);
    plugin.override_next_child(v1::OperationPolicy {
        timeout: Some(short.try_into().unwrap()),
        attempt_timeout: Some(short.try_into().unwrap()),
        cancellation_timeout: None,
        retries: None,
    });
    data.control.block_read.store(true, Ordering::Release);
    let first = child::StorageListRowsRequest {
        source: query.source.clone(),
        transforms: vec![],
        at: first.resolved_at,
        page: Some(v1::PageRequest {
            page_token: first.next_page_token,
            ..Default::default()
        }),
    };
    let mut overridden = Box::pin(super::transfer_plugin::call::<context::ListRows>(&runtime.operations, first));
    assert!(crate::support::events::until(overridden.as_mut(), data.control.read_entered.notified()).await.is_ok());
    clock.scheduled(short).await;
    clock.advance(short);
    assert!(overridden.await.is_err());
    let second_cut = second.resolved_at.clone();
    let second = child::StorageListRowsRequest {
        source: query.source,
        transforms: vec![],
        at: second.resolved_at,
        page: Some(v1::PageRequest {
            page_token: second.next_page_token,
            ..Default::default()
        }),
    };
    let mut inherited = Box::pin(super::transfer_plugin::call::<context::ListRows>(&runtime.operations, second));
    assert!(crate::support::events::until(inherited.as_mut(), data.control.read_entered.notified()).await.is_ok());
    clock.advance(short);
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(inherited.as_mut(), context).is_pending())).await
    );
    data.control.block_read.store(false, Ordering::Release);
    data.control.read_release.notify_waiters();
    let completed = inherited.await.unwrap();
    assert_eq!(completed.resolved_at, second_cut);
    attached.stop("call-local policy journey complete").await.unwrap();
    plugin.stop().await;
    storage.close().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
