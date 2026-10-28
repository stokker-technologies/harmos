/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{Arc, atomic::Ordering};

use super::{CleanupCapture, closed};
use crate::storage::support::{Change, Fact, HOUSE, Noted, deposit};
use crate::storage::{Bank, Data, Ledger, Recovery, StorageBackends, StorageManager, catalog, config, history, schema};
use harmos_runtime::state::SnapshotPolicy;
use harmos_runtime::storage::state::RecoverySnapshot;
use harmos_runtime::storage::{ReadLimits, StorageSettings};
use harmos_runtime::{Runtime, state::Focus, state::Position, state::PrincipalId, state::RequestKey};

type Refusal = &'static str;

struct Store {
    data: Arc<Data>,
    recovery: Arc<Recovery>,
}

struct Model;

#[harmos_runtime::transaction(id = "snapshot.add")]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Add {
    amount: i64,
}

#[harmos_runtime::record(id = "snapshot.seen")]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Seen {
    before: i64,
}

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
    fn snapshot(&self) -> RecoverySnapshot {
        self.recovery.snapshot.lock().unwrap().clone().unwrap()
    }
}

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
    type Error = Refusal;
    fn check(&self, _: &i64) -> Result<(), Refusal> {
        Ok(())
    }
    fn records(&self, state: &i64, out: &mut harmos_runtime::state::Records) {
        out.push(Seen { before: *state }).unwrap();
    }
    fn apply(&self, state: &mut i64) {
        *state += self.amount;
    }
}

fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

fn policy(every: u64) -> SnapshotPolicy {
    SnapshotPolicy {
        schema: schema(),
        every: NonZeroU64::new(every).unwrap(),
        max_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

async fn balance(runtime: &Runtime<Bank>) -> i64 {
    runtime.state.state().read(|state| state.balances.get(&HOUSE).copied().unwrap_or(0)).await.unwrap().value
}

async fn launch(snapshot: SnapshotPolicy, storage: &StorageManager) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(snapshot),
            journal_tail: 2,
        });
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn snapshot_restores_state() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    let who = PrincipalId::new("editor");
    let journal = runtime.state.as_principal(who.clone());
    let key = RequestKey::new("first");
    let receipt = journal.commit_keyed(key.clone(), deposit(5)).await.unwrap();
    journal.commit(deposit(6)).await.unwrap();
    journal.undo(who.clone(), Focus::all()).await.unwrap();
    harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("tail".into())).await.unwrap();
    runtime.shutdown().await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 4);
    store.recovery.entries.lock().unwrap().clear();

    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_restores_at_payload_limit() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    runtime.shutdown().await.unwrap();

    let mut exact = policy(100);
    exact.max_bytes = NonZeroUsize::new(store.snapshot().payload.len()).unwrap();
    store.recovery.snapshot.lock().unwrap().take();
    let storage = store.manager(config());
    let runtime = launch(exact.clone(), &storage).await;
    runtime.shutdown().await.unwrap();
    let snapshot = store.snapshot();
    assert_eq!(snapshot.payload.len(), exact.max_bytes.get());
    assert!(serde_json::to_vec(&snapshot).unwrap().len() > exact.max_bytes.get());
    store.recovery.entries.lock().unwrap().clear();

    let storage = store.manager(config());
    let runtime = launch(exact, &storage).await;
    assert_eq!(balance(&runtime).await, 4);
    assert_eq!(runtime.state.applied(), Position(1));
    assert_eq!(runtime.state.stored(), Position(1));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_over_payload_limit_retains_journal() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = launch(policy(100), &storage).await;
    runtime.state.commit(deposit(4)).await.unwrap();
    runtime.shutdown().await.unwrap();

    let mut small = policy(100);
    small.max_bytes = NonZeroUsize::new(store.snapshot().payload.len() - 1).unwrap();
    store.recovery.snapshot.lock().unwrap().take();
    let storage = store.manager(config());
    let runtime = launch(small.clone(), &storage).await;
    runtime.shutdown().await.unwrap();
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
    assert_eq!(storage.status().pending_bytes, 0);

    let storage = store.manager(config());
    let runtime = launch(small, &storage).await;
    assert_eq!(balance(&runtime).await, 4);
    assert_eq!(runtime.state.applied(), Position(1));
    runtime.shutdown().await.unwrap();
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
}

#[tokio::test]
async fn snapshot_resumes_durable() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(2)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(2)).await.unwrap();
    runtime.state.commit(deposit(3)).await.unwrap();
    store.recovery.control.snapshot_written.notified().await;
    storage.flush_recovery().await.unwrap();
    let earlier = store.snapshot();
    assert_eq!(earlier.through.sequence, 2);
    runtime.state.commit(deposit(7)).await.unwrap();
    runtime.shutdown().await.unwrap();
    *store.recovery.snapshot.lock().unwrap() = Some(earlier);
    store.recovery.entries.lock().unwrap().retain(|at, _| *at > 2);

    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(2)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    assert_eq!(balance(&runtime).await, 12);
    assert_eq!(runtime.state.applied(), Position(3));
    assert_eq!(runtime.state.state().read(|state| state.applications).await.unwrap().value, 3);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn journal_replays_alone() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(9)).await.unwrap();
    runtime.shutdown().await.unwrap();
    store.recovery.snapshot.lock().unwrap().take();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    assert_eq!(balance(&runtime).await, 9);
    runtime.shutdown().await.unwrap();
    store.recovery.snapshot.lock().unwrap().as_mut().unwrap().payload = b"invalid snapshot".to_vec();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_keeps_tail() {
    let store = Store::new();

    let storage = store.manager(config());
    let runtime = Runtime::<Model>::launch_with(0, (), |setup| {
        setup.settings().storage(&storage).register(Transactions::catalog()).register(Records::catalog()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    runtime.state.commit(Add { amount: 3 }).await.unwrap();
    runtime.shutdown().await.unwrap();
    let storage = store.manager(config());
    let runtime = Runtime::<Model>::launch_with(0, (), |setup| {
        setup.settings().storage(&storage).register(Transactions::catalog()).register(Records::catalog()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 3);
    runtime.shutdown().await.unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&store.snapshot().payload).unwrap();
    assert_eq!(snapshot["tail"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["tail"][0]["version"], 1);
    store.recovery.entries.lock().unwrap().clear();
    let storage = store.manager(config());
    let runtime = Runtime::<Model>::launch_with(0, (), |setup| {
        setup.settings().storage(&storage).register(Transactions::catalog()).register(Records::catalog()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 3);
    let facts: Vec<_> = runtime.state.query::<Records>(Position::ORIGIN).await.collect();
    assert_eq!(facts.len(), 1);
    assert!(facts[0].is_ok());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn oversized_snapshot_skipped() {
    let store = Store::new();
    let storage = store.manager(config());
    let mut small = policy(1);
    small.max_bytes = NonZeroUsize::new(1).unwrap();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(small),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(4)).await.unwrap();
    runtime.shutdown().await.unwrap();
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
    assert_eq!(storage.status().pending_bytes, 0);
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    assert_eq!(balance(&runtime).await, 4);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn snapshot_failure_retryable() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(4)).await.unwrap();
    storage.flush_recovery().await.unwrap();
    store.recovery.control.permanent.store(true, Ordering::SeqCst);
    assert!(runtime.shutdown().await.is_err());
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    assert!(store.recovery.snapshot.lock().unwrap().is_none());
    assert!(storage.status().pending_bytes > 0);
    store.recovery.control.permanent.store(false, Ordering::SeqCst);
    runtime.shutdown().await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 1);
    assert_eq!(storage.status().pending_bytes, 0);
}

#[tokio::test]
async fn snapshot_waits_queue() {
    let store = Store::new();
    let mut config = config();
    config.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let storage = store.manager(config);
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(1)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(4)).await.unwrap();
    runtime.shutdown().await.unwrap();
    assert_eq!(store.snapshot().through.sequence, 1);
    assert_eq!(storage.status().pending_bytes, 0);
}

#[tokio::test]
async fn corrupt_snapshot_blocks() {
    for field in ["schema", "position", "undo", "receipts", "tail", "boundary"] {
        let store = Store::new();
        let storage = store.manager(config());
        let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 2,
            });
        })
        .await
        .unwrap();
        runtime.state.as_principal(PrincipalId::new("author")).commit_keyed(RequestKey::new("key"), deposit(4)).await.unwrap();
        runtime.shutdown().await.unwrap();
        {
            let mut snapshot = store.recovery.snapshot.lock().unwrap();
            let snapshot = snapshot.as_mut().unwrap();
            let mut value: serde_json::Value = serde_json::from_slice(&snapshot.payload).unwrap();
            match field {
                "schema" => value["schema"]["id"] = "other".into(),
                "position" => value["position"] = 0.into(),
                "undo" => value["undos"][0]["inverse"]["id"] = "unknown".into(),
                "receipts" => {
                    let duplicate = value["receipts"][0].clone();
                    value["receipts"].as_array_mut().unwrap().push(duplicate);
                }
                "tail" => value["tail"][0]["position"] = 2.into(),
                "boundary" => snapshot.through.sequence += 1,
                _ => unreachable!(),
            }
            snapshot.payload = serde_json::to_vec(&value).unwrap();
        }
        let storage = store.manager(config());
        assert!(
            Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
                setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
                    replay: limits(),
                    capture: limits(),
                    snapshots: Some(policy(100)),
                    journal_tail: 2,
                });
            })
            .await
            .is_err(),
            "{field}"
        );
        assert!(storage.status().closed);
        assert!(!store.data.control.open.load(Ordering::SeqCst));
        assert!(!store.recovery.control.open.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn cancelled_snapshot_releases() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(4)).await.unwrap();
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

#[tokio::test]
async fn snapshot_read_ignores_write_queue_budget() {
    let store = Store::new();
    let storage = store.manager(config());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    runtime.state.commit(deposit(4)).await.unwrap();
    runtime.shutdown().await.unwrap();
    let mut config = config();
    config.request.max_pending_write_bytes = NonZeroUsize::new(256 * 1024).unwrap();
    let storage = store.manager(config);
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(100)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    assert_eq!(balance(&runtime).await, 4);
    assert_eq!(storage.status().pending_bytes, 0);
    runtime.shutdown().await.unwrap();
    assert!(storage.status().closed);
}

#[tokio::test]
async fn snapshots_require_storage() {
    assert!(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1024,
            });
        })
        .await
        .is_err()
    );
}

#[tokio::test]
async fn snapshot_releases_unused() {
    let store = Store::new();
    let mut settings = config();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(1024 * 1024).unwrap();
    let storage = store.manager(settings);
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(policy(1)),
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    // Hold Every Save So The Written Snapshot Stays Charged Until Inspected.
    store.recovery.control.block_flush.store(true, Ordering::SeqCst);
    runtime.state.commit(deposit(5)).await.unwrap();
    store.recovery.control.snapshot_written.notified().await;
    assert!(storage.status().pending_bytes < 64 * 1024);
    assert_eq!(storage.status().recovery_saved.sequence, 0);
    store.recovery.control.block_flush.store(false, Ordering::SeqCst);
    store.recovery.control.flush_release.notify_one();
    runtime.shutdown().await.unwrap();
    assert!(!store.snapshot().payload.is_empty());
}
