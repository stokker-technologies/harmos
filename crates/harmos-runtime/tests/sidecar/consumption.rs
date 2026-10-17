/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../support/native_streams.rs"]
#[allow(dead_code, reason = "the stream bench drives the executable carrier")]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
mod native_streams;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use native_streams::{Fixture, Payload};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn native_consumption_settles() {
    let fixture = Fixture::new().await;
    let size = 4096;
    for batch in 0..2 {
        for offset in 0..8 {
            let sequence = batch * 8 + offset;
            fixture.runtime.streams.publish(Payload {
                sequence,
                bytes: vec![(sequence % 251) as u8; size],
            });
        }
        let expected = (batch + 1) * 8;
        let acknowledged = fixture.acknowledged(expected).await;
        assert_eq!(acknowledged.rows, expected);
        assert_eq!(acknowledged.bytes, expected * size as u64);
    }
    fixture.runtime.stop().await.unwrap();
}

/* === Tests === */
