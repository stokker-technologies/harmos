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
async fn snapshot_failure_retryable() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    storage.flush_recovery().await.unwrap();
    store.recovery.control.permanent.store(true, Ordering::SeqCst);
    assert!(crate::support::app::shutdown(&runtime).await.is_err());
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
    assert!(storage.status().pending_bytes > 0);
    store.recovery.control.permanent.store(false, Ordering::SeqCst);
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 1);
    assert_eq!(storage.status().pending_bytes, 0);
}
