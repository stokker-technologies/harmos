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
async fn oversized_snapshot_skipped() {
    let store = Store::new();
    let storage = store.manager(config());
    let mut small = policy(1);
    small.max_bytes = NonZeroUsize::new(1).unwrap();
    let runtime = launch(small, &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
    assert_eq!(storage.status().pending_bytes, 0);
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    assert_eq!(balance(&runtime).await, 4);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
