/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash a Local File Through the Private Guest Callee Boundary.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn guest_hash_uses_the_native_protobuf_handler() {
    use super::*;
    use harmos_guest::builtins::storage::HASH_FILE_OPERATION;
    use harmos_guest::builtins::storage::HashFile;
    use harmos_runtime::storage::{ByteRange, FileHash, FileSource};
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(harmos_runtime::operations::Lane::default()).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    std::fs::write(&path, b"private guest hash").unwrap();
    let contract = HASH_FILE_OPERATION.wire().contract().unwrap();
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
                source: FileSource::Local(path),
                range: ByteRange { offset: 0, limit: None },
            }
            .encode(),
        )
        .await
        .unwrap();
    let response = FileHash::decode(&response).unwrap();
    assert_eq!(response.bytes_read, 18);
    assert_eq!(response.sha256, <[u8; 32]>::from(Sha256::digest(b"private guest hash")));
    manager.shutdown(&mut host).await.unwrap();
    fixture.finish().await;
}
