/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn snapshot_read_ignores_write_queue_budget() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    let mut config = config();
    config.request.max_write_batch_bytes = NonZeroUsize::new(256 * 1024).unwrap();
    let storage = store.manager(config);
    let runtime = launch(policy(100), &storage).await;
    assert_eq!(balance(&runtime).await, 4);
    assert_eq!(storage.status().pending_batches, 0);
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(storage.status().closed);
}
