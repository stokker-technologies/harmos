/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Lost Caller Delivery Retains Admitted Samples

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn admitted_samples_survive_lost_delivery() {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    seed(&f).await;
    let mutation = harmos_storage::MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: Identifier::new("samples").unwrap(),
    };
    let request = harmos_protocol::v1::StorageAppendSamplesRequest {
        mutation: Some(mutation.clone().into()),
        scope: None,
        batch: Some(batch(1, "voltage").into()),
    };
    f.data.control.block_write.store(true, std::sync::atomic::Ordering::Release);
    let caller = f.runtime.operations.clone();
    let original = request.clone();
    let delivery = tokio::spawn(async move {
        let _reply = caller.call_protobuf::<AppendSamples>(original).await.unwrap();
        std::future::pending::<()>().await;
    });
    f.data.control.write_entered.notified().await;
    delivery.abort();
    let _ = delivery.await;
    let accepted = f.runtime.operations.call_protobuf::<AppendSamples>(request.clone()).await.unwrap().receipt.unwrap();
    assert!(!accepted.completed);
    assert!(accepted.durable_at.is_none());
    let position = f.storage.cursor_accepted();
    f.data.control.block_write.store(false, std::sync::atomic::Ordering::Release);
    f.data.control.write_release.notify_waiters();
    f.storage.wait_mutation_durable(&mutation, &position).await.unwrap();
    let completed = f.runtime.operations.call_protobuf::<AppendSamples>(request.clone()).await.unwrap().receipt.unwrap();
    assert_eq!(completed.accepted_at, accepted.accepted_at);
    assert_eq!(completed.durable_at, completed.accepted_at);
    assert!(completed.completed);
    let mut changed = request;
    changed.batch = Some(batch(2, "other").into());
    assert!(f.runtime.operations.call_protobuf::<AppendSamples>(changed).await.is_err());
    assert_eq!(f.data.rows.lock().unwrap().len(), 2);
    assert_eq!(f.storage.get_saved_position(), position);
    f.finish().await;
}
