/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "../support/stream_batches.rs"]
pub mod batches;

#[tokio::test]
async fn raw_summary_delivery() {
    batches::raw_and_summary_delivery().await;
}
