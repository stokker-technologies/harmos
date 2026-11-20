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
async fn snapshot_resumes_durable() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = start(policy(2), &storage).await;
    runtime.state.commit(deposit(2)).await.unwrap();
    runtime.state.commit(deposit(3)).await.unwrap();
    store.recovery.control.snapshot_written.notified().await;
    storage.flush_recovery().await.unwrap();
    let earlier = store.snapshot();
    assert_eq!(earlier.through.sequence, 2);
    runtime.state.commit(deposit(7)).await.unwrap();
    storage.flush_recovery().await.unwrap();
    let suffix = store.recovery.entries.lock().unwrap().get(&3).unwrap().clone();
    crate::support::app::stop(&runtime).await.unwrap();
    *store.recovery.snapshot.lock().unwrap() = Some(earlier);
    store.recovery.entries.lock().unwrap().insert(3, suffix);

    let storage = store.manager(config());
    let runtime = start(policy(2), &storage).await;
    assert_eq!(balance(&runtime).await, 12);
    assert_eq!(runtime.state.applied(), Position(3));
    assert_eq!(runtime.state.state().read(|state| state.applications).await.unwrap().value, 3);
    crate::support::app::stop(&runtime).await.unwrap();
}
