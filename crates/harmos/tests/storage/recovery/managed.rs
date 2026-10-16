/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::support::{Change, Fact, HOUSE, Noted, deposit};
use super::{
    Bank, Data, Ledger, Recovery, Runtime, StorageBackends, StorageManager, catalog, config,
    history,
};
use harmos::storage::{ReadLimits, StorageSettings};
use harmos::{state::Focus, state::Position, state::PrincipalId, state::RequestKey};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Refusal = &'static str;

/* === Structs === */
struct Model;

#[harmos::transaction(id = "managed.add", version = 2, irreversible, history(OldAdd))]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Add {
    amount: i64,
}

#[harmos::transaction(version = 1)]
#[derive(serde::Serialize, serde::Deserialize)]
struct OldAdd {
    amount: i64,
}

#[harmos::record(id = "managed.seen", version = 1)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Seen {
    before: i64,
}

struct Store {
    data: Arc<Data>,
    recovery: Arc<Recovery>,
}

/* === Enums === */
#[harmos::transactions(state = i64, error = Refusal)]
#[derive(Clone)]
enum Transactions {
    Add(Add),
}

#[harmos::records]
#[derive(Clone)]
enum Records {
    Seen(Seen),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Model {
    type State = i64;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

impl harmos::state::Transaction<i64> for Add {
    type Error = &'static str;
    fn check(&self, _: &i64) -> Result<(), Self::Error> {
        if self.amount < 0 {
            Err("negative amount")
        } else {
            Ok(())
        }
    }
    fn records(&self, state: &i64, out: &mut harmos::state::Records) {
        out.push(Seen { before: *state }).unwrap();
        out.push_system(harmos::state::SystemFact {
            source: harmos::state::FactSource::Work("probe".into()),
            name: "observed".into(),
            version: 1,
            payload: vec![1, 2],
        })
        .unwrap();
    }
    fn apply(&self, state: &mut i64) {
        *state += self.amount;
    }
}

impl harmos::state::Transaction<i64> for OldAdd {
    type Error = &'static str;
    fn check(&self, _: &i64) -> Result<(), Self::Error> {
        panic!("replay must not check")
    }
    fn records(&self, _: &i64, _: &mut harmos::state::Records) {
        panic!("replay must not emit records")
    }
    fn apply(&self, state: &mut i64) {
        *state += 2 * self.amount;
    }
}

impl From<OldAdd> for Add {
    fn from(old: OldAdd) -> Self {
        Self { amount: old.amount }
    }
}

impl Store {
    fn new() -> Self {
        Self {
            data: Arc::new(Data::new(history())),
            recovery: Arc::new(Recovery::new(history())),
        }
    }
    fn manager(&self, config: StorageSettings) -> StorageManager {
        StorageManager::new(
            StorageBackends {
                primary: self.data.clone(),
                state: self.recovery.clone(),
            },
            config,
        )
        .unwrap()
    }
    async fn boot(&self, storage: &StorageManager) -> Runtime<Bank> {
        Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .tail(2)
            .managed_state(limits())
            .runtime_with_storage(storage)
            .await
            .unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(256 * 1024).unwrap(),
    }
}

async fn balance(runtime: &Runtime<Bank>) -> i64 {
    runtime
        .state
        .state()
        .read(|state| state.balances.get(&HOUSE).copied().unwrap_or(0))
        .await
        .unwrap()
        .value
}

async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("replay_restores_state", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
                    for _ in 0..5 {
                        journal.commit(deposit(3)).await.unwrap();
                    }
                    harmos::records::RecordManager::new(journal.clone())
                        .publish(Noted("saved".into()))
                        .await
                        .unwrap();
                    assert_eq!(runtime.state.stored(), Position::ORIGIN);
                    storage.flush_recovery().await.unwrap();
                    assert_eq!(runtime.state.stored(), Position(6));
                    runtime.stop().await.unwrap();

                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert_eq!(balance(&runtime).await, 15);
                    assert_eq!(runtime.state.applied(), Position(6));
                    let records: Vec<_> = runtime.state.query::<Fact>(Position(5)).await.collect();
                    assert_eq!(records.len(), 1);
                    assert!(records[0].is_ok());
                    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
                    journal
                        .undo(PrincipalId::new("editor"), Focus::all())
                        .await
                        .unwrap();
                    assert_eq!(balance(&runtime).await, 12);
                    runtime.stop().await.unwrap();

                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert_eq!(balance(&runtime).await, 12);
                    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
                    journal
                        .redo(PrincipalId::new("editor"), Focus::all())
                        .await
                        .unwrap();
                    assert_eq!(balance(&runtime).await, 15);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("storage_backpressure_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut config = config();
                    config.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
                    let storage = store.manager(config);
                    let runtime = store.boot(&storage).await;
                    store
                        .recovery
                        .control
                        .block_write
                        .store(true, Ordering::SeqCst);
                    runtime.state.commit(deposit(2)).await.unwrap();
                    let next = {
                        let journal = runtime.state.clone();
                        tokio::spawn(async move { journal.commit(deposit(7)).await })
                    };
                    store.recovery.control.write_entered.notified().await;
                    assert_eq!(balance(&runtime).await, 2);
                    assert!(!next.is_finished());
                    assert_eq!(storage.status().pending_batches, 1);
                    store
                        .recovery
                        .control
                        .block_write
                        .store(false, Ordering::SeqCst);
                    store.recovery.control.write_release.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), next)
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                    assert_eq!(balance(&runtime).await, 9);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("oversized_group_refuses", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let mut tiny = limits();
                    tiny.bytes = NonZeroUsize::new(4096).unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .managed_state(tiny)
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    assert!(
                        runtime
                            .records
                            .publish(Noted("x".repeat(4096)))
                            .await
                            .is_err()
                    );
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    assert_eq!(storage.status().pending_bytes, 0);
                    runtime.state.commit(deposit(2)).await.unwrap();
                    assert_eq!(balance(&runtime).await, 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("mutation_panic_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert!(runtime.state.commit(Change::Explode).await.is_err());
                    assert_eq!(storage.status().pending_bytes, 0);
                    assert_eq!(storage.status().pending_batches, 0);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("save_keeps_mutation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    runtime.state.commit(deposit(8)).await.unwrap();
                    store
                        .recovery
                        .control
                        .flush_failures
                        .store(1, Ordering::SeqCst);
                    assert!(runtime.stop().await.is_err());
                    assert_eq!(storage.status().recovery_accepted.sequence, 1);
                    assert_eq!(storage.status().recovery_saved.sequence, 0);
                    assert!(store.recovery.control.open.load(Ordering::SeqCst));
                    runtime.stop().await.unwrap();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert_eq!(balance(&runtime).await, 8);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("entry_refuses_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    runtime.state.commit(deposit(1)).await.unwrap();
                    runtime.state.commit(deposit(2)).await.unwrap();
                    runtime.stop().await.unwrap();
                    store.recovery.entries.lock().unwrap().remove(&1);
                    let storage = store.manager(config());
                    assert!(
                        Runtime::<Bank>::builder(Ledger::default(), ())
                            .register(catalog())
                            .managed_state(limits())
                            .runtime_with_storage(&storage)
                            .await
                            .is_err()
                    );
                    assert!(storage.status().closed);
                    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
                    assert!(!store.data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("corrupt_group_transaction", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    runtime.state.commit(deposit(1)).await.unwrap();
                    runtime.stop().await.unwrap();
                    let original = store.recovery.entries.lock().unwrap()[&1].payload.clone();
                    for field in ["version", "position", "id", "commit"] {
                        let mut group: serde_json::Value =
                            serde_json::from_slice(&original).unwrap();
                        match field {
                            "version" => group["version"] = 7.into(),
                            "position" => group["entries"][0]["position"] = 9.into(),
                            "id" => group["entries"][0]["id"] = "unknown.transaction".into(),
                            "commit" => {
                                group["entries"][0]["metadata"]["commit"] = serde_json::Value::Null
                            }
                            _ => unreachable!(),
                        }
                        store
                            .recovery
                            .entries
                            .lock()
                            .unwrap()
                            .get_mut(&1)
                            .unwrap()
                            .payload = serde_json::to_vec(&group).unwrap();
                        let storage = store.manager(config());
                        assert!(
                            Runtime::<Bank>::builder(Ledger::default(), ())
                                .register(catalog())
                                .managed_state(limits())
                                .runtime_with_storage(&storage)
                                .await
                                .is_err(),
                            "{field}"
                        );
                        assert!(storage.status().closed);
                    }
                });
        });
    }
    {
        scenario_runtime::check("owner_refuses_raw", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert!(
                        Runtime::<Bank>::builder(Ledger::default(), ())
                            .register(catalog())
                            .managed_state(limits())
                            .runtime()
                            .await
                            .is_err()
                    );
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert!(storage.write_recovery(&[b"foreign"]).await.is_err());
                    assert!(
                        storage
                            .write_snapshot(&harmos::storage::state::RecoverySnapshot {
                                required_data: storage.cursor_saved(),
                                through: storage.status().recovery_accepted,
                                payload: vec![],
                            })
                            .await
                            .is_err()
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("periodic_save_advances", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    let receipt = runtime.state.commit(deposit(1)).await.unwrap();
                    until(|| runtime.state.stored() == receipt.frontier).await;
                    assert_eq!(storage.status().saved.sequence, 0);
                    assert_eq!(storage.status().recovery_saved.sequence, 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("whole_group_replays", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let builder = || {
                        Runtime::<Model>::builder(0, ())
                            .register(Transactions::catalog())
                            .register(Records::catalog())
                            .tail(2)
                            .managed_state(limits())
                    };
                    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
                    let receipt = runtime.state.commit(Add { amount: 3 }).await.unwrap();
                    assert_eq!(receipt.frontier, Position(3));
                    assert_eq!(storage.status().recovery_accepted.sequence, 1);
                    runtime.stop().await.unwrap();

                    {
                        let mut entries = store.recovery.entries.lock().unwrap();
                        let entry = entries.get_mut(&1).unwrap();
                        let mut group: serde_json::Value =
                            serde_json::from_slice(&entry.payload).unwrap();
                        group["entries"][0]["version"] = 1.into();
                        group["entries"][0]["payload"]["amount"] = (-3).into();
                        entry.payload = serde_json::to_vec(&group).unwrap();
                    }
                    let storage = store.manager(config());
                    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|state| *state)
                            .await
                            .unwrap()
                            .value,
                        -6
                    );
                    assert_eq!(runtime.state.applied(), Position(3));
                    assert_eq!(runtime.state.stored(), Position(3));
                    assert_eq!(storage.status().recovery_accepted.sequence, 1);
                    let facts: Vec<_> = runtime
                        .state
                        .query::<Records>(Position::ORIGIN)
                        .await
                        .collect();
                    assert_eq!(facts.len(), 1);
                    assert!(facts[0].is_ok());
                    let system: Vec<_> = runtime
                        .state
                        .query::<harmos::state::SystemFact>(Position::ORIGIN)
                        .await
                        .collect();
                    assert_eq!(system.len(), 1);
                    assert!(system[0].is_ok());
                    runtime.state.commit(Add { amount: 2 }).await.unwrap();
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|state| *state)
                            .await
                            .unwrap()
                            .value,
                        -4
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_replay_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    runtime.state.commit(deposit(1)).await.unwrap();
                    runtime.stop().await.unwrap();
                    store
                        .recovery
                        .control
                        .block_read
                        .store(true, Ordering::SeqCst);
                    let storage = store.manager(config());
                    {
                        let opening = Runtime::<Bank>::builder(Ledger::default(), ())
                            .register(catalog())
                            .managed_state(limits())
                            .runtime_with_storage(&storage);
                        tokio::pin!(opening);
                        tokio::select! {
                            _ = &mut opening => panic!("replay must wait for the backend"),
                            _ = store.recovery.control.read_entered.notified() => {}
                        }
                        assert!(storage.status().pending_bytes > 0);
                    }
                    until(|| storage.status().closed).await;
                    assert_eq!(storage.status().pending_bytes, 0);
                    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
                    assert!(!store.data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("storage_refuses_mutation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    runtime.state.commit(deposit(1)).await.unwrap();
                    store
                        .recovery
                        .control
                        .flush_failures
                        .store(1, Ordering::SeqCst);
                    assert!(storage.flush_recovery().await.is_err());
                    assert!(runtime.state.commit(deposit(9)).await.is_err());
                    assert!(runtime.state.capture_status().is_err());
                    assert_eq!(storage.status().recovery_accepted.sequence, 1);
                    runtime.stop().await.unwrap();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert_eq!(balance(&runtime).await, 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("file_finish_data", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut policy = config();
                    policy.request.max_pending_write_batches = NonZeroUsize::new(4).unwrap();
                    let storage = store.manager(policy);
                    let runtime = store.boot(&storage).await;
                    let mutations = {
                        let journal = runtime.state.clone();
                        tokio::spawn(async move {
                            for _ in 0..20 {
                                journal.commit(deposit(1)).await.unwrap();
                            }
                        })
                    };
                    let data = {
                        let storage = storage.clone();
                        tokio::spawn(async move {
                            for n in 0..20 {
                                super::write(&storage, &format!("item-{n}")).await;
                            }
                        })
                    };
                    let mut file = storage
                        .create_file(harmos::storage::FileMetadata {
                            id: harmos::Identifier::new("managed-file").unwrap(),
                            scope: None,
                            media_type: "application/octet-stream".into(),
                            expected_length: Some(3),
                            expected_sha256: None,
                        })
                        .await
                        .unwrap();
                    file.write_all(b"abc").await.unwrap();
                    let reference = file.finish().await.unwrap();
                    mutations.await.unwrap();
                    data.await.unwrap();
                    assert_eq!(balance(&runtime).await, 20);
                    runtime.stop().await.unwrap();
                    assert_eq!(storage.status().saved.sequence, 21);
                    assert_eq!(storage.status().recovery_saved.sequence, 20);
                    assert_eq!(store.data.records.lock().unwrap().len(), 20);
                    assert_eq!(
                        store
                            .data
                            .files
                            .lock()
                            .unwrap()
                            .get(&reference.position.sequence),
                        Some(&reference)
                    );
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    assert_eq!(balance(&runtime).await, 20);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("budget_refuses_transition", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let mut tiny = limits();
                    tiny.bytes = NonZeroUsize::new(512).unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .managed_state(tiny)
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    assert!(runtime.state.commit(deposit(9)).await.is_err());
                    assert_eq!(balance(&runtime).await, 0);
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    assert_eq!(storage.status().pending_bytes, 0);
                    assert!(runtime.state.capture_status().is_ok());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("request_survives_tail", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    let key = RequestKey::new("durable-deposit");
                    let receipt = runtime
                        .state
                        .commit_keyed(key.clone(), deposit(7))
                        .await
                        .unwrap();
                    for _ in 0..5 {
                        runtime.state.commit(deposit(1)).await.unwrap();
                    }
                    runtime.stop().await.unwrap();

                    for _ in 0..2 {
                        let storage = store.manager(config());
                        let runtime = store.boot(&storage).await;
                        let retry = runtime
                            .state
                            .commit_keyed(key.clone(), Change::Explode)
                            .await
                            .unwrap();
                        assert_eq!(retry, receipt);
                        assert_eq!(balance(&runtime).await, 12);
                        assert_eq!(runtime.state.applied(), Position(6));
                        assert_eq!(storage.status().recovery_accepted.sequence, 6);
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("request_keeps_group", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let builder = || {
                        Runtime::<Model>::builder(0, ())
                            .register(Transactions::catalog())
                            .register(Records::catalog())
                            .managed_state(limits())
                    };
                    let key = RequestKey::new("group");
                    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
                    let receipt = runtime
                        .state
                        .caused_by(Position(41))
                        .commit_keyed(key.clone(), Add { amount: 3 })
                        .await
                        .unwrap();
                    assert_eq!(receipt.position, Position(1));
                    assert_eq!(receipt.frontier, Position(3));
                    runtime.stop().await.unwrap();

                    let storage = store.manager(config());
                    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
                    let retry = runtime
                        .state
                        .commit_keyed(key, Add { amount: -1 })
                        .await
                        .unwrap();
                    assert_eq!(retry, receipt);
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|state| *state)
                            .await
                            .unwrap()
                            .value,
                        3
                    );
                    assert_eq!(runtime.state.applied(), receipt.frontier);
                    assert_eq!(storage.status().recovery_accepted.sequence, 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("keys_refuse_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage).await;
                    for key in ["one", "two"] {
                        runtime
                            .state
                            .commit_keyed(RequestKey::new(key), deposit(1))
                            .await
                            .unwrap();
                    }
                    runtime.stop().await.unwrap();
                    {
                        let mut entries = store.recovery.entries.lock().unwrap();
                        let entry = entries.get_mut(&2).unwrap();
                        let mut group: serde_json::Value =
                            serde_json::from_slice(&entry.payload).unwrap();
                        group["entries"][0]["metadata"]["request"] = "one".into();
                        entry.payload = serde_json::to_vec(&group).unwrap();
                    }
                    let storage = store.manager(config());
                    assert!(
                        Runtime::<Bank>::builder(Ledger::default(), ())
                            .register(catalog())
                            .managed_state(limits())
                            .runtime_with_storage(&storage)
                            .await
                            .is_err()
                    );
                    assert!(storage.status().closed);
                    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
                    assert!(!store.data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("commits_never_require", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for explicit_barrier in [false, true] {
                        let store = Store::new();
                        let storage = store.manager(config());
                        let runtime = store.boot(&storage).await;
                        tokio::task::yield_now().await;

                        let receipt = runtime.state.commit(deposit(7)).await.unwrap();
                        tokio::task::yield_now().await;
                        assert_eq!(balance(&runtime).await, 7);
                        assert_eq!(runtime.state.applied(), receipt.frontier);
                        assert_eq!(runtime.state.stored(), Position::ORIGIN);
                        assert!(store.recovery.control.writes.lock().unwrap().is_empty());
                        assert_eq!(storage.status().recovery_accepted.sequence, 1);
                        assert_eq!(storage.status().recovery_saved.sequence, 0);

                        if explicit_barrier {
                            storage.flush_recovery().await.unwrap();
                            runtime.state.wait_until_stored(receipt.frontier).await;
                        }
                        runtime.stop().await.unwrap();

                        assert_eq!(*store.recovery.control.write_sizes.lock().unwrap(), [1]);
                        assert_eq!(storage.status().recovery_saved.sequence, 1);
                        assert_eq!(storage.status().pending_bytes, 0);
                    }
                });
        });
    }
}

/* === Tests === */
