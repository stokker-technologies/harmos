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
        let runtime = launch(policy(100), &storage).await;
        runtime.state.commit(deposit(4)).await.unwrap();
        crate::support::app::shutdown(&runtime).await.unwrap();
        let mut bounded = policy(100);
        bounded.max_bytes = NonZeroUsize::new(store.snapshot().payload.len() - (segments - 1)).unwrap();
        store.recovery.snapshot.lock().unwrap().take();
        let storage = store.manager(config());
        let runtime = launch(bounded.clone(), &storage).await;
        crate::support::app::shutdown(&runtime).await.unwrap();
        let snapshot = store.snapshot();
        assert_eq!(snapshot.payload.manifest().count, segments);
        if segments == 1 {
            assert_eq!(snapshot.payload.len(), bounded.max_bytes.get());
        }
        assert_eq!(storage.status().pending_batches, 0);

        // Restore Without A Journal Fallback, Then Save The Same Segmentation.
        store.recovery.entries.lock().unwrap().clear();
        let storage = store.manager(config());
        let runtime = launch(bounded, &storage).await;
        assert_eq!(balance(&runtime).await, 4);
        assert_eq!(runtime.state.applied(), Position(1));
        assert_eq!(runtime.state.stored(), Position(1));
        crate::support::app::shutdown(&runtime).await.unwrap();
        assert_eq!(store.snapshot().payload.manifest().count, segments);
    }
}
