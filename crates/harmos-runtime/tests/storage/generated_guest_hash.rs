/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash A Local File Through The Declared Context Callee.

/* === Tests === */

#[tokio::test]
async fn guest_hash_uses_the_native_protobuf_handler() {
    use super::*;
    use harmos_operations::storage::context::GetFileHash;
    use harmos_protocol::context::StorageGetFileHashRequest;
    use harmos_runtime::storage::{ByteRange, FileSource};
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    std::fs::write(&path, b"private guest hash").unwrap();
    let contract = harmos_runtime::operations::OperationFactory::protobuf::<GetFileHash>().unwrap().contract().unwrap();
    let definitions = PluginDefinitions::new([plugin("reader", vec![Required { contract, grants: vec![] }])], []).unwrap();
    let mut manager = PluginManager::new(definitions).unwrap();
    let mut host = Host {
        operations: fixture.runtime.operations.clone(),
        callers: BTreeMap::new(),
    };
    manager.initialize(&mut host).await.unwrap();
    let reader = fixture.runtime.operations.caller(&host.callers["reader"]).unwrap();
    let response = reader
        .call_protobuf::<GetFileHash>(StorageGetFileHashRequest {
            source: Some(FileSource::Local(path).try_into().unwrap()),
            range: Some(ByteRange { offset: 0, limit: None }.into()),
            at: None,
        })
        .await
        .unwrap()
        .hash
        .unwrap();
    assert_eq!(response.bytes_read, 18);
    assert_eq!(response.sha256, Sha256::digest(b"private guest hash").to_vec());
    manager.shutdown(&mut host).await.unwrap();
    fixture.finish().await;
}
