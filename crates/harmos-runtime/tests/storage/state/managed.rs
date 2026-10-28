/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::{CleanupCapture, closed};
use crate::storage::support::{Change, Fact, HOUSE, Noted, deposit};
use crate::storage::{Bank, Data, Ledger, Recovery, Runtime, StorageBackends, StorageManager, catalog, config, history};
use harmos_runtime::storage::{ReadLimits, StorageSettings};
use harmos_runtime::{state::Focus, state::Position, state::PrincipalId, state::RequestKey};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Refusal = &'static str;

/* === Structs === */

struct Model;

#[harmos_runtime::transaction(id = "managed.add", irreversible)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Add {
    amount: i64,
}

#[harmos_runtime::record(id = "managed.seen")]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Seen {
    before: i64,
}

struct Store {
    data: Arc<Data>,
    recovery: Arc<Recovery>,
}

/* === Enums === */

#[harmos_runtime::transactions(state = i64, error = Refusal)]
#[derive(Clone)]
enum Transactions {
    Add(Add),
}

#[harmos_runtime::records]
#[derive(Clone)]
enum Records {
    Seen(Seen),
}

/* === Traits === */

/* === Implementations === */

#[harmos_runtime::lanes]
pub enum ModelLanes {
    #[default]
    Open,
}
impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos_runtime::Application for Model {
    type Lanes = ModelLanes;
    type State = i64;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
}

impl harmos_runtime::state::Transaction<i64> for Add {
    type Error = &'static str;
    fn check(&self, _: &i64) -> Result<(), Self::Error> {
        if self.amount < 0 { Err("negative amount") } else { Ok(()) }
    }
    fn records(&self, state: &i64, out: &mut harmos_runtime::state::Records) {
        out.push(Seen { before: *state }).unwrap();
        out.push_system(harmos_runtime::state::SystemFact {
            source: harmos_runtime::state::FactSource::Work("probe".into()),
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
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().storage(storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 2,
            });
        })
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
    runtime.state.state().read(|state| state.balances.get(&HOUSE).copied().unwrap_or(0)).await.unwrap().value
}

#[tokio::test]
async fn replay_restores_state() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
    for _ in 0..5 {
        journal.commit(deposit(3)).await.unwrap();
    }
    harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("saved".into())).await.unwrap();
    assert_eq!(runtime.state.stored(), Position::ORIGIN);
    storage.flush_recovery().await.unwrap();
    assert_eq!(runtime.state.stored(), Position(6));
    runtime.shutdown().await.unwrap();

    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert_eq!(balance(&runtime).await, 15);
    assert_eq!(runtime.state.applied(), Position(6));
    let records: Vec<_> = runtime.state.query::<Fact>(Position(5)).await.collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].is_ok());
    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
    journal.undo(PrincipalId::new("editor"), Focus::all()).await.unwrap();
    assert_eq!(balance(&runtime).await, 12);
    runtime.shutdown().await.unwrap();

    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert_eq!(balance(&runtime).await, 12);
    let journal = runtime.state.as_principal(PrincipalId::new("editor"));
    journal.redo(PrincipalId::new("editor"), Focus::all()).await.unwrap();
    assert_eq!(balance(&runtime).await, 15);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn commit_awaits_capacity() {
    let store = Store::new();
    let mut config = config();
    config.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let storage = store.manager(config);
    let runtime = store.boot(&storage).await;
    store.recovery.control.block_write.store(true, Ordering::SeqCst);
    runtime.state.commit(deposit(2)).await.unwrap();
    let next = {
        let journal = runtime.state.clone();
        tokio::spawn(async move { journal.commit(deposit(7)).await })
    };
    store.recovery.control.write_entered.notified().await;
    assert_eq!(balance(&runtime).await, 2);
    assert!(!next.is_finished());
    assert_eq!(storage.status().pending_batches, 1);
    store.recovery.control.block_write.store(false, Ordering::SeqCst);
    store.recovery.control.write_release.notify_one();
    next.await.unwrap().unwrap();
    assert_eq!(balance(&runtime).await, 9);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn oversized_group_refused() {
    let store = Store::new();
    let storage = store.manager(config());
    let mut tiny = limits();
    tiny.bytes = NonZeroUsize::new(4096).unwrap();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: tiny,
            capture: tiny,
            snapshots: None,
            journal_tail: 1024,
        });
    })
    .await
    .unwrap();
    assert!(runtime.records.publish(Noted("x".repeat(4096))).await.is_err());
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    assert_eq!(storage.status().recovery_accepted.sequence, 0);
    assert_eq!(storage.status().pending_bytes, 0);
    assert!(runtime.state.capture_status().is_ok());
    runtime.state.commit(deposit(2)).await.unwrap();
    assert_eq!(balance(&runtime).await, 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn state_stop_retries() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    runtime.state.commit(deposit(8)).await.unwrap();
    store.recovery.control.flush_failures.store(1, Ordering::SeqCst);
    assert!(runtime.shutdown().await.is_err());
    assert_eq!(storage.status().recovery_accepted.sequence, 1);
    assert_eq!(storage.status().recovery_saved.sequence, 0);
    assert!(store.recovery.control.open.load(Ordering::SeqCst));
    runtime.shutdown().await.unwrap();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert_eq!(balance(&runtime).await, 8);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn recovery_history_admission() {
    {
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        runtime.state.commit(deposit(1)).await.unwrap();
        runtime.state.commit(deposit(2)).await.unwrap();
        runtime.shutdown().await.unwrap();
        let original = store.recovery.entries.lock().unwrap().remove(&1).unwrap();
        let storage = store.manager(config());
        assert!(
            Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
                setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                    replay: limits(),
                    capture: limits(),
                    snapshots: None,
                    journal_tail: 1024,
                });
            })
            .await
            .is_err()
        );
        assert!(storage.status().closed);
        assert!(!store.recovery.control.open.load(Ordering::SeqCst));
        assert!(!store.data.control.open.load(Ordering::SeqCst));

        store.recovery.entries.lock().unwrap().insert(1, original);
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        assert_eq!(balance(&runtime).await, 3);
        runtime.shutdown().await.unwrap();
    }
    {
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        runtime.state.commit(deposit(1)).await.unwrap();
        runtime.shutdown().await.unwrap();
        let original = store.recovery.entries.lock().unwrap()[&1].payload.clone();
        for field in ["position", "id", "commit"] {
            let mut group: serde_json::Value = serde_json::from_slice(&original).unwrap();
            match field {
                "position" => group["entries"][0]["position"] = 9.into(),
                "id" => group["entries"][0]["id"] = "unknown.transaction".into(),
                "commit" => group["entries"][0]["metadata"]["commit"] = serde_json::Value::Null,
                _ => unreachable!(),
            }
            store.recovery.entries.lock().unwrap().get_mut(&1).unwrap().payload = serde_json::to_vec(&group).unwrap();
            let storage = store.manager(config());
            assert!(
                Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
                    setup.settings().storage(&storage).register(catalog()).managed_storage(
                        harmos_runtime::config::ManagedStorageSettings {
                            replay: limits(),
                            capture: limits(),
                            snapshots: None,
                            journal_tail: 1024,
                        },
                    );
                })
                .await
                .is_err(),
                "{field}"
            );
            assert!(storage.status().closed);
        }

        store.recovery.entries.lock().unwrap().get_mut(&1).unwrap().payload = original;
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        assert_eq!(balance(&runtime).await, 1);
        runtime.shutdown().await.unwrap();
    }
    {
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        for key in ["one", "two"] {
            runtime.state.commit_keyed(RequestKey::new(key), deposit(1)).await.unwrap();
        }
        runtime.shutdown().await.unwrap();
        let original = store.recovery.entries.lock().unwrap()[&2].payload.clone();
        {
            let mut entries = store.recovery.entries.lock().unwrap();
            let entry = entries.get_mut(&2).unwrap();
            let mut group: serde_json::Value = serde_json::from_slice(&entry.payload).unwrap();
            group["entries"][0]["metadata"]["request"] = "one".into();
            entry.payload = serde_json::to_vec(&group).unwrap();
        }
        let storage = store.manager(config());
        assert!(
            Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
                setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                    replay: limits(),
                    capture: limits(),
                    snapshots: None,
                    journal_tail: 1024,
                });
            })
            .await
            .is_err()
        );
        assert!(storage.status().closed);
        assert!(!store.recovery.control.open.load(Ordering::SeqCst));
        assert!(!store.data.control.open.load(Ordering::SeqCst));

        store.recovery.entries.lock().unwrap().get_mut(&2).unwrap().payload = original;
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        assert_eq!(balance(&runtime).await, 2);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn managed_requires_storage() {
    assert!(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1024,
            });
        })
        .await
        .is_err()
    );
}

#[tokio::test]
async fn owner_refuses_raw() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert!(storage.write_recovery(vec![b"foreign".to_vec()]).await.is_err());
    assert!(
        storage
            .write_snapshot(harmos_runtime::storage::state::RecoverySnapshot {
                required_data: storage.cursor_saved(),
                through: storage.status().recovery_accepted,
                payload: vec![],
            })
            .await
            .is_err()
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn periodic_save_advances() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    let receipt = runtime.state.commit(deposit(1)).await.unwrap();
    runtime.state.wait_until_stored(receipt.frontier).await;
    assert_eq!(runtime.state.stored(), receipt.frontier);
    assert_eq!(storage.status().saved.sequence, 0);
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_replay_releases() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    runtime.state.commit(deposit(1)).await.unwrap();
    runtime.shutdown().await.unwrap();
    store.recovery.control.block_read.store(true, Ordering::SeqCst);
    let storage = store.manager(config());
    let cleanup = CleanupCapture::default();
    {
        let opening = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            cleanup.capture(setup);
            setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1024,
            });
        });
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("replay must wait for the backend"),
            _ = store.recovery.control.read_entered.notified() => {}
        }
        assert!(store.recovery.control.open.load(Ordering::SeqCst));
        assert!(store.data.control.open.load(Ordering::SeqCst));
        assert_eq!(storage.status().pending_bytes, 0);
    }
    closed(&storage, cleanup.get()).await;
    assert_eq!(storage.status().pending_bytes, 0);
    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
    assert!(!store.data.control.open.load(Ordering::SeqCst));
}

#[tokio::test]
async fn save_failure_halts() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    runtime.state.commit(deposit(1)).await.unwrap();
    store.recovery.control.flush_failures.store(1, Ordering::SeqCst);
    assert!(storage.flush_recovery().await.is_err());
    assert!(runtime.state.commit(deposit(9)).await.is_err());
    assert!(runtime.state.capture_status().is_err());
    assert_eq!(storage.status().recovery_accepted.sequence, 1);
    runtime.shutdown().await.unwrap();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert_eq!(balance(&runtime).await, 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn file_finish_interleaves() {
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
                crate::storage::write(&storage, &format!("item-{n}")).await;
            }
        })
    };
    let mut file = storage
        .create_file(harmos_runtime::storage::FileMetadata {
            id: harmos_runtime::Identifier::new("managed-file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(3),
            expected_sha256: None,
        })
        .await
        .unwrap();
    file.write_all((b"abc").to_vec()).await.unwrap();
    let reference = file.finish().await.unwrap();
    mutations.await.unwrap();
    data.await.unwrap();
    assert_eq!(balance(&runtime).await, 20);
    runtime.shutdown().await.unwrap();
    assert_eq!(storage.status().saved.sequence, 21);
    assert_eq!(storage.status().recovery_saved.sequence, 20);
    assert_eq!(store.data.records.lock().unwrap().len(), 20);
    assert_eq!(store.data.files.lock().unwrap().get(&reference.position.sequence), Some(&reference));
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    assert_eq!(balance(&runtime).await, 20);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn keyed_receipt_survives() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = store.boot(&storage).await;
    let key = RequestKey::new("durable-deposit");
    let receipt = runtime.state.commit_keyed(key.clone(), deposit(7)).await.unwrap();
    for _ in 0..5 {
        runtime.state.commit(deposit(1)).await.unwrap();
    }
    runtime.shutdown().await.unwrap();

    for _ in 0..2 {
        let storage = store.manager(config());
        let runtime = store.boot(&storage).await;
        let retry = runtime.state.commit_keyed(key.clone(), Change::Explode).await.unwrap();
        assert_eq!(retry, receipt);
        assert_eq!(balance(&runtime).await, 12);
        assert_eq!(runtime.state.applied(), Position(6));
        assert_eq!(storage.status().recovery_accepted.sequence, 6);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn keyed_group_retained() {
    let store = Store::new();
    let storage = store.manager(config());

    let key = RequestKey::new("group");
    let runtime = Runtime::<Model>::launch_with(0, (), |setup| {
        setup.settings().storage(&storage).register(Transactions::catalog()).register(Records::catalog()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1024,
            },
        );
    })
    .await
    .unwrap();
    let receipt = runtime.state.caused_by(Position(41)).commit_keyed(key.clone(), Add { amount: 3 }).await.unwrap();
    assert_eq!(receipt.position, Position(1));
    assert_eq!(receipt.frontier, Position(3));
    runtime.shutdown().await.unwrap();

    let storage = store.manager(config());
    let runtime = Runtime::<Model>::launch_with(0, (), |setup| {
        setup.settings().storage(&storage).register(Transactions::catalog()).register(Records::catalog()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1024,
            },
        );
    })
    .await
    .unwrap();
    let retry = runtime.state.commit_keyed(key, Add { amount: -1 }).await.unwrap();
    assert_eq!(retry, receipt);
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 3);
    assert_eq!(runtime.state.applied(), receipt.frontier);
    assert_eq!(storage.status().recovery_accepted.sequence, 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn commits_skip_save() {
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
        runtime.shutdown().await.unwrap();

        assert_eq!(*store.recovery.control.write_sizes.lock().unwrap(), [1]);
        assert_eq!(storage.status().recovery_saved.sequence, 1);
        assert_eq!(storage.status().pending_bytes, 0);
    }
}

/* === Tests === */
