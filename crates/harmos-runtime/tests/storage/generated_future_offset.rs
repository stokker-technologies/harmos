/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Refuse Following Beyond the Produced File Frontier.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn future_offset_refused() {
    let (fixture, mut producer, request) = setup().await;
    let beyond = v1::StorageWatchFileProductionRequest { offset: 7, ..request };
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<WatchFileProduction>(beyond, limits()).unwrap();
    let receiver = receiver.unwrap();
    assert!(instance.await.is_err());
    assert!(receive_production(&receiver).await.is_none());
    producer.abort().await.unwrap();
    fixture.finish().await;
}
