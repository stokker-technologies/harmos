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
async fn snapshot_payload_boundary_restores_segmented_state() {
    for segments in [1, 2] {
        // Measure One Snapshot, Then Reopen At And Just Below Its Payload Boundary.
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = start(policy(100), &storage).await;
        runtime.state.commit(deposit(4)).await.unwrap();
        storage.flush_recovery().await.unwrap();
        let replay = Store::new();
        *replay.recovery.entries.lock().unwrap() = store.recovery.entries.lock().unwrap().clone();
        replay.recovery.saved.store(store.recovery.saved.load(Ordering::SeqCst), Ordering::SeqCst);
        crate::support::app::stop(&runtime).await.unwrap();
        let mut bounded = policy(100);
        bounded.max_bytes = NonZeroUsize::new(store.snapshot().payload.len() - (segments - 1)).unwrap();
        let store = replay;
        let storage = store.manager(config());
        let runtime = start(bounded.clone(), &storage).await;
        crate::support::app::stop(&runtime).await.unwrap();
        let snapshot = store.snapshot();
        assert_eq!(snapshot.payload.manifest().count, segments);
        if segments == 1 {
            assert_eq!(snapshot.payload.len(), bounded.max_bytes.get());
        }
        assert_eq!(storage.status().pending_bytes, 0);

        // Restore Without A Journal Fallback, Then Save The Same Segmentation.
        let storage = store.manager(config());
        let runtime = start(bounded, &storage).await;
        assert_eq!(balance(&runtime).await, 4);
        assert_eq!(runtime.state.applied(), Position(1));
        assert_eq!(runtime.state.stored(), Position(1));
        crate::support::app::stop(&runtime).await.unwrap();
        assert_eq!(store.snapshot().payload.manifest().count, segments);
    }
}
