/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../tests/support/snapshot_store.rs"]
mod store_fixture;

/* === Imports === */

use super::backend::{Data, Recovery, history, storage_settings as config};
use super::support::{Bank, Change, Fact, Ledger, Noted, catalog, deposit};
use harmos_runtime::state::{Focus, Position, PrincipalId, RequestKey, SnapshotPolicy};
use harmos_runtime::storage::{PayloadEncoding, ReadLimits, SchemaKey};
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::atomic::Ordering;
use store_fixture::Store;

/* === Private Functions === */

fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

fn policy(every: u64) -> SnapshotPolicy {
    SnapshotPolicy {
        schema: SchemaKey {
            payload_encoding: PayloadEncoding::PostcardV1,
            id: harmos_runtime::Identifier::new("snapshot").unwrap(),
            version: std::num::NonZeroU32::new(1).unwrap(),
        },
        every: NonZeroU64::new(every).unwrap(),
        max_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

pub(super) async fn segmented_recovery() {
    let store = Store::new();
    let storage = store.manager(config());
    let mut policy = policy(10000);
    policy.max_bytes = NonZeroUsize::new(64 * 1024).unwrap();
    let initial = Ledger::bulky(100_000);
    let runtime = crate::support::app::launch::<Bank>(initial.clone(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy.clone()),
            journal_tail: 64,
        });
    })
    .await
    .unwrap();
    let who = PrincipalId::new("editor");
    let journal = runtime.state.as_principal(who.clone());
    let key = RequestKey::new("first");
    let receipt = journal.commit_keyed(key.clone(), deposit(1)).await.unwrap();
    for _ in 1..666 {
        journal.commit(deposit(1)).await.unwrap();
    }
    for _ in 0..64 {
        harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("x".repeat(16 * 1024))).await.unwrap();
    }
    crate::support::app::shutdown(&runtime).await.unwrap();
    let snapshot = store.snapshot();
    assert_eq!(snapshot.payload.manifest().position, Some(Position(730)));
    assert!(snapshot.payload.manifest().count > 3);
    assert!(snapshot.payload.manifest().segments.iter().all(|segment| segment.bytes <= policy.max_bytes.get()));
    // Retain The Full Journal For An Independent Replay And Add Work Beyond The Checkpoint.
    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Bank>(initial.clone(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: None,
            journal_tail: 64,
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
                journal_tail: 64,
            });
        })
        .await
        .unwrap();
        assert_eq!(runtime.state.applied(), Position(731));
        let restored = runtime.state.state().read(Clone::clone).await.unwrap().value;
        let facts: Vec<_> = runtime.state.query::<Fact>(Position(667)).await.collect();
        assert_eq!(facts.len(), 63);
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
