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
mod native_streams;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use native_streams::Fixture;

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
async fn host_rows_reach_managed_plugin_in_order() {
    // Attach a Managed SDK Consumer to a Virtual Host
    let fixture = Fixture::new().await;

    // Deliver and Acknowledge the First Batch Across the Process Boundary
    fixture.publish(0..8, 4096);
    let acknowledged = fixture.acknowledged(8).await;
    assert_ne!(acknowledged.process_id, std::process::id());
    assert_eq!((acknowledged.rows, acknowledged.bytes), (8, 8 * 4096));

    // Preserve Stream Order and Byte Counts Across the Next Batch
    fixture.publish(8..16, 4096);
    let acknowledged = fixture.acknowledged(16).await;
    assert_eq!((acknowledged.rows, acknowledged.bytes), (16, 16 * 4096));

    // Settle the Host and Its Managed Consumer
    fixture.runtime.shutdown().await.unwrap();
}

/* === Tests === */
