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
async fn journal_replays_alone() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(9)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    store.recovery.snapshot.lock().unwrap().take();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    assert_eq!(balance(&runtime).await, 9);
    crate::support::app::shutdown(&runtime).await.unwrap();
    store.recovery.snapshot.lock().unwrap().as_mut().unwrap().payload =
        SnapshotPayload::from_bytes(b"invalid snapshot", policy(100).max_bytes, Position(1)).unwrap();
    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: None,
            journal_tail: 1024,
        });
    })
    .await
    .unwrap();
    assert_eq!(balance(&runtime).await, 9);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
