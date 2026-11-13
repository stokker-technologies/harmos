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
async fn snapshot_waits_queue() {
    let store = Store::new();
    let mut config = config();
    config.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let storage = store.manager(config);
    let runtime = launch(policy(1), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 1);
    assert_eq!(storage.status().pending_batches, 0);
}
