/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Generated File Streams After Cancellation.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn follow_reconnect_resumes() {
    let (fixture, mut producer, mut request) = setup().await;
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<WatchFileProduction>(request.clone(), limits()).unwrap();
    let receiver = receiver.unwrap();
    let mut offset = 0;
    while offset < 6 {
        let chunk = receive_production(&receiver).await.unwrap();
        assert_eq!(chunk.offset, offset);
        offset += chunk.bytes.len() as u64;
    }
    instance.request_cancel();
    assert!(instance.await.is_err());
    drop(receiver);
    producer.write_all((b"later").to_vec()).await.unwrap();
    request.offset = offset;
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<WatchFileProduction>(request, limits()).unwrap();
    let receiver = receiver.unwrap();
    let mut bytes = Vec::new();
    while offset < 11 {
        let chunk = receive_production(&receiver).await.unwrap();
        assert_eq!(chunk.offset, offset);
        bytes.extend_from_slice(&chunk.bytes);
        offset += chunk.bytes.len() as u64;
    }
    assert_eq!(bytes, b"later");
    producer.abort().await.unwrap();
    assert!(receive_production(&receiver).await.is_none());
    let result = v1::StorageWatchFileProductionResponse::decode(instance.await.unwrap().as_slice()).unwrap();
    let Some(v1::storage_watch_file_production_response::Event::Progress(result)) = result.event else {
        panic!("terminal production progress is required");
    };
    assert_eq!(result.length, 11);
    assert_eq!(result.phase, wire::FilePhase::Abandoned as i32);
    fixture.finish().await;
}
