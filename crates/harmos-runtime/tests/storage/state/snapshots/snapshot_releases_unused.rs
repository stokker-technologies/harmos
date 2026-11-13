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
async fn snapshot_releases_unused() {
    let store = Store::new();
    let mut settings = config();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(1024 * 1024).unwrap();
    let storage = store.manager(settings);
    let runtime = launch(policy(1), &storage).await;
    // Hold Every Save So The Written Snapshot Stays Charged Until Inspected.
    store.recovery.control.block_flush.store(true, Ordering::SeqCst);
    runtime.state.commit(deposit(5)).await.unwrap();
    store.recovery.control.snapshot_written.notified().await;
    assert!(storage.status().pending_bytes < 64 * 1024);
    assert_eq!(storage.status().recovery_saved.sequence, 0);
    store.recovery.control.block_flush.store(false, Ordering::SeqCst);
    store.recovery.control.flush_release.notify_one();
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(!store.snapshot().payload.is_empty());
}
