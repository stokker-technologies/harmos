/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "../support/allocation.rs"]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
mod scenarios;

#[tokio::test]
async fn publish_allocation_independent() {
    scenarios::streams_r004_publish_cost_is_independent_of_the_window_size().await;
}
