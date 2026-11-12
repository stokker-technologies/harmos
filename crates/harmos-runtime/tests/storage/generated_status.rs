/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe Storage Frontiers Through Generated Messages.

/* === Tests === */

#[tokio::test]
async fn reports_persisted_frontiers() {
    use harmos_operations::storage::StorageStatus;
    use harmos_protocol::storage;
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let accepted = super::write(&fixture.storage, "protobuf-status").await;
    fixture.storage.flush().await.unwrap();
    let status = fixture.runtime.operations.call_protobuf::<StorageStatus>(storage::StorageStatus {}).await.unwrap();
    assert_eq!(status.accepted, Some(accepted.clone().into()));
    assert_eq!(status.saved, Some(accepted.into()));
    fixture.finish().await;
}
