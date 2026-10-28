/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "../support/native_streams.rs"]
#[allow(dead_code, reason = "the stream bench drives the executable carrier")]
mod native_streams;

use native_streams::{Fixture, Payload};

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
    fixture.runtime.shutdown().await.unwrap();
}
