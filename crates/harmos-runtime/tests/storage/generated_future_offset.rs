/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Refuse Following Beyond the Produced File Frontier.

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
async fn future_offset_refused() {
    let (fixture, mut producer, request) = setup().await;
    let beyond = wire::FollowProduction { offset: 7, ..request };
    let (instance, receiver) = fixture.runtime.operations.submit_protobuf::<FollowProduction>(beyond, limits()).unwrap();
    let receiver = receiver.unwrap();
    assert!(instance.await.is_err());
    assert!(receive(&receiver).await.is_none());
    producer.abort().await.unwrap();
    fixture.finish().await;
}
