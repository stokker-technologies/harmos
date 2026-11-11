/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Generated File Streams After Cancellation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

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
async fn follow_reconnect_resumes() {
    let (fixture, mut producer, mut request) = setup().await;
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<FollowProduction>(request.clone(), limits()).unwrap();
    let receiver = receiver.unwrap();
    let mut offset = 0;
    while offset < 6 {
        let chunk = receive(&receiver).await.unwrap();
        assert_eq!(chunk.offset, offset);
        offset += chunk.bytes.len() as u64;
    }
    instance.request_cancel();
    assert!(instance.await.is_err());
    drop(receiver);
    producer.write_all((b"later").to_vec()).await.unwrap();
    request.offset = offset;
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<FollowProduction>(request, limits()).unwrap();
    let receiver = receiver.unwrap();
    let mut bytes = Vec::new();
    while offset < 11 {
        let chunk = receive(&receiver).await.unwrap();
        assert_eq!(chunk.offset, offset);
        bytes.extend_from_slice(&chunk.bytes);
        offset += chunk.bytes.len() as u64;
    }
    assert_eq!(bytes, b"later");
    producer.abort().await.unwrap();
    assert!(receive(&receiver).await.is_none());
    let result = wire::FileProgress::decode(instance.await.unwrap().as_slice()).unwrap();
    assert_eq!(result.length, 11);
    assert_eq!(result.phase, wire::FilePhase::Abandoned as i32);
    fixture.finish().await;
}
