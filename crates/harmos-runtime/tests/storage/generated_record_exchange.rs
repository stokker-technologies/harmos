/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Replay Bounded Record Appends Through Durable Completion

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

struct Authority;
impl harmos_runtime::operations::authorization::Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: vec![harmos_operations::plugin::Grant::of(provider.contract.operation().id.to_string(), "bench")],
            },
        )))
    }
}

/* === Tests === */

#[tokio::test]
async fn record_append_receipts() {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    seed(&f).await;
    f.runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
    let plugin = super::super::super::transfer_plugin::TransferPlugin::start().await;
    let attached = f
        .runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: std::num::NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let listener = f.runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();

    let mutation = harmos_storage::MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: Identifier::new("records").unwrap(),
    };
    let request = harmos_protocol::v1::StorageAppendRecordsRequest {
        mutation: Some(mutation.clone().into()),
        scope: None,
        batch: Some(batch(1, "bench").into()),
    };
    f.data.control.block_write.store(true, std::sync::atomic::Ordering::Release);
    let accepted =
        super::super::super::transfer_plugin::client_call::<AppendRecords>(&channel, request.clone()).await.unwrap().receipt.unwrap();
    assert!(accepted.accepted_at.is_some());
    assert!(accepted.durable_at.is_none());
    assert!(!accepted.completed);
    let position = f.storage.cursor_accepted();
    let replay = super::super::super::transfer_plugin::call::<harmos_operations::storage::context::AppendRecords>(
        &f.runtime.operations,
        harmos_protocol::context::StorageAppendRecordsRequest {
            mutation: request.mutation.clone(),
            scope: request.scope.clone(),
            batch: request.batch.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(replay.receipt, Some(accepted.clone()));

    assert_eq!(accepted.accepted_at.as_ref().unwrap().sequence, position.sequence);
    assert_eq!(
        super::super::super::transfer_plugin::client_call::<AppendRecords>(&channel, request.clone()).await.unwrap().receipt,
        Some(accepted.clone())
    );
    let mut changed = request.clone();
    changed.scope = Some("changed-scope".into());
    assert!(f.runtime.operations.call_protobuf::<AppendRecords>(changed).await.is_err());
    assert_eq!(f.storage.cursor_accepted(), position);
    f.data.control.write_entered.notified().await;
    f.data.control.block_receipt.store(true, std::sync::atomic::Ordering::Release);
    f.data.control.block_write.store(false, std::sync::atomic::Ordering::Release);
    f.data.control.write_release.notify_waiters();
    f.storage.await_durable(&position).await.unwrap();
    f.data.control.receipt_entered.notified().await;
    assert_eq!(
        super::super::super::transfer_plugin::client_call::<AppendRecords>(&channel, request.clone()).await.unwrap().receipt,
        Some(accepted.clone())
    );
    f.data.control.block_receipt.store(false, std::sync::atomic::Ordering::Release);
    f.data.control.receipt_release.notify_waiters();
    f.storage.wait_mutation_durable(&mutation, &position).await.unwrap();
    let durable =
        super::super::super::transfer_plugin::client_call::<AppendRecords>(&channel, request.clone()).await.unwrap().receipt.unwrap();
    assert_eq!(durable.request_id, accepted.request_id);
    assert_eq!(durable.accepted_at, accepted.accepted_at);
    assert_eq!(durable.durable_at, accepted.accepted_at);
    assert!(durable.completed);
    assert_eq!(f.runtime.operations.call_protobuf::<AppendRecords>(request).await.unwrap().receipt, Some(durable));
    assert_eq!(f.data.records.lock().unwrap().len(), 2);
    attached.stop("record append journey complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    f.finish().await;
}
