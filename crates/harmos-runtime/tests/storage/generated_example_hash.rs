/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash Through a Generated Application Operation and Its Declared Native Callee.

/* === Tests === */

#[tokio::test]
async fn example_hash_invokes_generated_storage() {
    use harmos_protocol::storage::{ByteRange, FileSource, HashFile, file_source};
    use sha2::{Digest, Sha256};
    let fixture = super::Fixture::new(super::config()).await;
    fixture
        .runtime
        .operations
        .register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    fixture
        .runtime
        .operations
        .register_batch(
            harmos_runtime::operations::Registration::new(harmos_runtime::operations::Lane::default())
                .protobuf::<schematic_operations::transfer::Hash>()
                .unwrap(),
        )
        .await
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    let payload = b"generated application hash";
    std::fs::write(&path, payload).unwrap();
    let response = fixture
        .runtime
        .operations
        .call_protobuf::<schematic_operations::transfer::Hash>(schematic_operations::protocol::Hash {
            request: Some(HashFile {
                source: Some(FileSource {
                    source: Some(file_source::Source::Local(path.to_str().unwrap().into())),
                }),
                range: Some(ByteRange { offset: 3, limit: Some(9) }),
            }),
        })
        .await
        .unwrap();
    assert_eq!(response.read_bytes, 9);
    assert_eq!(response.sha256, Sha256::digest(&payload[3..12]).to_vec());
    fixture.finish().await;
}
