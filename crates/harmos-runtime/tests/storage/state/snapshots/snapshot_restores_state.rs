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
async fn snapshot_restores_state() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    let who = PrincipalId::new("editor");
    let journal = runtime.state.as_principal(who.clone());
    let key = RequestKey::new("first");
    let receipt = journal.commit_keyed(key.clone(), deposit(5)).await.unwrap();
    journal.commit(deposit(6)).await.unwrap();
    journal.undo(who.clone(), Focus::all()).await.unwrap();
    harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("tail".into())).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 4);
    store.recovery.entries.lock().unwrap().clear();

    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    assert_eq!(balance(&runtime).await, 5);
    assert_eq!(runtime.state.applied(), Position(4));
    assert_eq!(runtime.state.stored(), Position(4));
    let facts: Vec<_> = runtime.state.query::<Fact>(Position(2)).await.collect();
    assert_eq!(facts.len(), 1);
    assert!(facts[0].is_ok());
    let journal = runtime.state.as_principal(who.clone());
    assert_eq!(journal.commit_keyed(key, Change::Explode).await.unwrap(), receipt);
    journal.redo(who.clone(), Focus::all()).await.unwrap();
    assert_eq!(balance(&runtime).await, 11);
    journal.undo(who, Focus::all()).await.unwrap();
    assert_eq!(balance(&runtime).await, 5);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
