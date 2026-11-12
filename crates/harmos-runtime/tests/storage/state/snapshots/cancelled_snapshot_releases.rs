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
async fn cancelled_snapshot_releases() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    store.recovery.control.block_read.store(true, Ordering::SeqCst);
    let storage = store.manager(config());
    let cleanup = CleanupCapture::default();
    {
        let opening = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            cleanup.capture(setup);
            setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 2,
            });
        });
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("snapshot load must wait"),
            _ = store.recovery.control.read_entered.notified() => {}
        }
        assert!(store.recovery.control.open.load(Ordering::SeqCst));
        assert!(store.data.control.open.load(Ordering::SeqCst));
        assert_eq!(storage.status().pending_bytes, 0);
    }
    closed(&storage, cleanup.get()).await;
    assert_eq!(storage.status().pending_bytes, 0);
    assert!(!store.data.control.open.load(Ordering::SeqCst));
    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
}
