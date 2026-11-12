/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash a Local File Through the Private Guest Callee Boundary.

/* === Tests === */

#[tokio::test]
async fn guest_hash_uses_the_native_protobuf_handler() {
    use super::*;
    use harmos_protocol::storage::FileHash;
    use harmos_protocol::storage::HashFile;
    use harmos_runtime::storage::{ByteRange, FileSource};
    use prost::Message as _;
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    std::fs::write(&path, b"private guest hash").unwrap();
    let contract =
        harmos_runtime::operations::OperationFactory::protobuf::<harmos_operations::storage::HashFile>().unwrap().contract().unwrap();
    let definitions = PluginDefinitions::new([plugin("reader", vec![Required { contract, grants: vec![] }])], []).unwrap();
    let mut manager = PluginManager::new(definitions).unwrap();
    let mut host = Host {
        operations: fixture.runtime.operations.clone(),
        callers: BTreeMap::new(),
    };
    manager.initialize(&mut host).await.unwrap();
    let reader = fixture.runtime.operations.caller(&host.callers["reader"]).unwrap();
    let response = reader
        .call_encoded(
            "host.storage.hash_file",
            HashFile {
                source: Some(FileSource::Local(path).try_into().unwrap()),
                range: Some(ByteRange { offset: 0, limit: None }.into()),
            }
            .encode_to_vec(),
        )
        .await
        .unwrap();
    let response = FileHash::decode(response.as_slice()).unwrap();
    assert_eq!(response.bytes_read, 18);
    assert_eq!(response.sha256, Sha256::digest(b"private guest hash").to_vec());
    manager.shutdown(&mut host).await.unwrap();
    fixture.finish().await;
}
