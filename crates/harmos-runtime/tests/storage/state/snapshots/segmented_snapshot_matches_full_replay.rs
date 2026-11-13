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
async fn segmented_snapshot_matches_full_replay() {
    let store = Store::new();
    let storage = store.manager(config());
    let mut policy = policy(100);
    policy.max_bytes = NonZeroUsize::new(1024).unwrap();
    let initial = Ledger::bulky(1024);
    let runtime = crate::support::app::launch::<Bank>(initial.clone(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy.clone()),
            journal_tail: 4,
        });
    })
    .await
    .unwrap();
    let who = PrincipalId::new("editor");
    let journal = runtime.state.as_principal(who.clone());
    let key = RequestKey::new("first");
    let receipt = journal.commit_keyed(key.clone(), deposit(1)).await.unwrap();
    for _ in 1..8 {
        journal.commit(deposit(1)).await.unwrap();
    }
    for _ in 0..4 {
        harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("x".repeat(512))).await.unwrap();
    }
    crate::support::app::shutdown(&runtime).await.unwrap();
    let snapshot = store.snapshot();
    assert_eq!(snapshot.payload.manifest().position, Some(Position(12)));
    assert!(snapshot.payload.manifest().count > 3);
    assert!(snapshot.payload.manifest().segments.iter().all(|segment| segment.bytes <= policy.max_bytes.get()));
    // Retain The Full Journal For An Independent Replay And Add Work Beyond The Checkpoint.
    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Bank>(initial.clone(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: None,
            journal_tail: 4,
        });
    })
    .await
    .unwrap();
    runtime.state.as_principal(who.clone()).commit(deposit(7)).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    let full = Store::new();
    *full.recovery.entries.lock().unwrap() = store.recovery.entries.lock().unwrap().clone();
    // The Fixture's Durable Frontier Is Reconstructed By Its Open Method.
    full.recovery.saved.store(store.recovery.saved.load(Ordering::SeqCst), Ordering::SeqCst);

    let mut results = Vec::new();
    for (store, snapshots) in [(&store, Some(policy.clone())), (&full, None)] {
        let storage = store.manager(config());
        let runtime = crate::support::app::launch::<Bank>(initial.clone(), (), |setup| {
            setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots,
                journal_tail: 4,
            });
        })
        .await
        .unwrap();
        assert_eq!(runtime.state.applied(), Position(13));
        let restored = runtime.state.state().read(Clone::clone).await.unwrap().value;
        let facts: Vec<_> = runtime.state.query::<Fact>(Position(9)).await.collect();
        assert_eq!(facts.len(), 3);
        assert!(facts.iter().all(Result::is_ok));
        let journal = runtime.state.as_principal(who.clone());
        assert_eq!(journal.commit_keyed(key.clone(), Change::Explode).await.unwrap(), receipt);
        journal.undo(who.clone(), Focus::all()).await.unwrap();
        let undone = runtime.state.state().read(Clone::clone).await.unwrap().value;
        journal.redo(who.clone(), Focus::all()).await.unwrap();
        let redone = runtime.state.state().read(Clone::clone).await.unwrap().value;
        results.push((restored, undone, redone));
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
    assert_eq!(results[0], results[1]);
}
