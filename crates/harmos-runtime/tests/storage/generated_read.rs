/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read a Stored File Through Generated Stream Items.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn read_file_streams() {
    let (fixture, mut producer, _) = setup().await;
    let finished = producer.finish().await.unwrap();
    fixture.storage.flush().await.unwrap();
    let request = wire::ReadFile {
        source: Some(harmos_domain::model::storage::files::FileSource::Stored(finished).try_into().unwrap()),
        range: Some(wire::ByteRange { offset: 0, limit: None }),
    };
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<ReadFile>(request, limits()).unwrap();
    let receiver = receiver.unwrap();
    let first = receive(&receiver).await.unwrap();
    assert_eq!(first.offset, 0);
    let mut bytes = first.bytes.clone();
    drop(first);

    while let Some(chunk) = receive(&receiver).await {
        assert_eq!(chunk.offset, bytes.len() as u64);

        bytes.extend_from_slice(&chunk.bytes);
    }
    assert_eq!(bytes, b"prefix");
    let result = wire::FileCompletion::decode(instance.await.unwrap().as_slice()).unwrap();
    assert_eq!(result.bytes, 6);
    fixture.finish().await;
}
