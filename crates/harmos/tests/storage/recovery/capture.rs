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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Data, Recovery, config, history, record, schema, schema_query};
use harmos::storage::*;
use harmos::{Error, Identifier, Runtime, state::Envelope, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Refusal = std::convert::Infallible;

/* === Structs === */
struct Model;

#[harmos::transaction(id = "capture.advance", version = 1, irreversible)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Advance {
    panic: bool,
}

#[harmos::record(id = "capture.observed", version = 1)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Observed {
    before: u64,
}

struct Projection {
    calls: Arc<AtomicUsize>,
    duplicate: bool,
}

struct IncrementalProjection {
    calls: Arc<AtomicUsize>,
    data: Arc<Data>,
    cancel: bool,
}

struct Store {
    data: Arc<Data>,
    recovery: Arc<Recovery>,
    calls: Arc<AtomicUsize>,
}

/* === Enums === */
#[harmos::transactions(state = u64, error = Refusal)]
#[derive(Clone)]
enum Transactions {
    Advance(Advance),
}

#[harmos::records]
#[derive(Clone)]
enum Records {
    Observed(Observed),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Model {
    type State = u64;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

impl harmos::state::Transaction<u64> for Advance {
    type Error = Refusal;
    fn check(&self, _: &u64) -> Result<(), Refusal> {
        Ok(())
    }
    fn records(&self, state: &u64, out: &mut harmos::state::Records) {
        out.push(Observed { before: *state }).unwrap();
    }
    fn apply(&self, state: &mut u64) {
        assert!(!self.panic, "deliberate mutation panic");
        *state += 1;
    }
}

impl RecordProjection for Projection {
    fn project<'a>(
        &'a self,
        entry: &'a Envelope<&serde_json::Value>,
        output: &'a mut ProjectedRecords<'_>,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let bytes = serde_json::to_vec(entry).unwrap();
            let item = Identifier::new(format!("record-{}", entry.position.0)).unwrap();
            output.push(record(&schema(), &item, &bytes)).await?;
            if self.duplicate {
                // Ignoring a producer error must never admit half of a projection.
                let _ = output.push(record(&schema(), &item, &bytes)).await;
            }
            Ok(())
        })
    }
}

impl RecordProjection for IncrementalProjection {
    fn project<'a>(
        &'a self,
        _: &'a Envelope<&serde_json::Value>,
        output: &'a mut ProjectedRecords<'_>,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let payload = vec![0; 100 * 1024];
            for index in 0..4 {
                let item = Identifier::new(format!("projected-{index}")).unwrap();
                let key = schema();
                let input = record(&key, &item, &payload);
                if self.cancel && index == 1 {
                    tokio::select! {
                        result = output.push(input) => panic!("capacity must wait: {result:?}"),
                        _ = self.data.control.write_entered.notified() => {}
                    }
                    // Abandon the pending push and ignore it: the sink must
                    // still refuse the complete transaction, including record 0.
                    return Ok(());
                }
                output.push(input).await?;
            }
            Ok(())
        })
    }
}

impl Store {
    fn new() -> Self {
        Self {
            data: Arc::new(Data::new(history())),
            recovery: Arc::new(Recovery::new(history())),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn manager(&self, config: StorageSettings) -> StorageManager {
        StorageManager::new(
            StorageBackends {
                primary: self.data.clone(),
                recovery: self.recovery.clone(),
                replicas: Vec::new(),
            },
            config,
        )
        .unwrap()
    }
    async fn boot(
        &self,
        storage: &StorageManager,
        capture: ReadLimits,
        duplicate: bool,
    ) -> Runtime<Model> {
        Runtime::<Model>::builder(0, ())
            .register(Transactions::catalog())
            .register(Records::catalog())
            .tail(1)
            .managed_state(limits(1, 64 * 1024))
            .store_records(
                capture,
                Projection {
                    calls: self.calls.clone(),
                    duplicate,
                },
            )
            .runtime_with_storage(storage)
            .await
            .unwrap()
    }
    async fn ready(&self, storage: &StorageManager) {
        storage
            .write_schema(
                &schema(),
                &harmos::schema::graph::Schema {
                    root: 0,
                    definitions: vec![vec![]],
                },
            )
            .await
            .unwrap();
        storage.flush().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn limits(rows: usize, bytes: usize) -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(rows).unwrap(),
        bytes: NonZeroUsize::new(bytes).unwrap(),
    }
}

fn captured_query() -> RecordQuery<Vec<u8>> {
    let mut query = RecordQuery::new(schema(), |bytes, allowance| {
        let owned = bytes
            .len()
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or(Error::ResourceExhausted {
                resource: "capture payload decode",
            })?;
        if owned > allowance.get() {
            return Err(Error::ResourceExhausted {
                resource: "capture payload decode",
            });
        }
        Ok(bytes.to_vec())
    });
    query.options.page = Some(limits(1, 16 * 1024));
    query
}

async fn read(storage: &StorageManager) -> Vec<StoredRecord> {
    let query = captured_query();
    let mut reader = storage.query(query).await.unwrap();
    let mut records = Vec::new();
    while let Some(page) = reader.next_page().await.unwrap() {
        records.extend(page.iter().cloned());
    }
    records
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("eviction_never_replays", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    for _ in 0..4 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    runtime
                        .records
                        .publish(Observed { before: 4 })
                        .await
                        .unwrap();
                    assert_eq!(store.calls.load(Ordering::SeqCst), 5);
                    assert!(read(&storage).await.is_empty());
                    storage.flush_recovery().await.unwrap();
                    assert_eq!(runtime.state.stored(), Position(9));
                    assert_eq!(storage.status().saved.sequence, 6);
                    assert_eq!(read(&storage).await.len(), 5);
                    storage.flush().await.unwrap();
                    let captured = read(&storage).await;
                    assert_eq!(captured.len(), 5);
                    let entry: Envelope<serde_json::Value> =
                        serde_json::from_slice(&captured[0].payload).unwrap();
                    assert_eq!(entry.position, Position(2));
                    assert_eq!(entry.metadata.commit, Some((Position(1), Position(2))));
                    assert_eq!(entry.payload["before"], 0);
                    runtime.stop().await.unwrap();

                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        4
                    );
                    assert_eq!(store.calls.load(Ordering::SeqCst), 5);
                    assert_eq!(read(&storage).await, captured);
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    assert_eq!(read(&storage).await.len(), 5);
                    storage.flush().await.unwrap();
                    assert_eq!(read(&storage).await.len(), 6);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("overflow_refuses_whole", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for budget in [limits(1, 16 * 1024), limits(4, 1)] {
                        let store = Store::new();
                        let storage = store.manager(config());
                        let runtime = store.boot(&storage, budget, true).await;
                        store.ready(&storage).await;
                        assert!(
                            runtime
                                .state
                                .commit(Advance { panic: false })
                                .await
                                .is_err()
                        );
                        assert_eq!(
                            runtime
                                .state
                                .state()
                                .read(|value| *value)
                                .await
                                .unwrap()
                                .value,
                            0
                        );
                        assert_eq!(storage.status().accepted.sequence, 1);
                        assert_eq!(storage.status().recovery_accepted.sequence, 0);
                        assert_eq!(storage.status().pending_bytes, 0);
                        assert_eq!(storage.status().pending_batches, 0);
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("primary_queue_backpressure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut config = config();
                    config.queue_capacity = NonZeroUsize::new(2).unwrap();
                    let storage = store.manager(config);
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    store.data.control.block_write.store(true, Ordering::SeqCst);
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    let next = runtime.state.commit(Advance { panic: false });
                    tokio::pin!(next);
                    tokio::select! {
                        result = &mut next => panic!("capacity must block: {result:?}"),
                        _ = store.data.control.write_entered.notified() => {}
                    }
                    assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        1
                    );
                    assert_eq!(storage.status().pending_batches, 2);
                    store
                        .data
                        .control
                        .block_write
                        .store(false, Ordering::SeqCst);
                    store.data.control.write_release.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), next)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(store.calls.load(Ordering::SeqCst), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("mutation_releases_reserved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    assert!(runtime.state.commit(Advance { panic: true }).await.is_err());
                    assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(storage.status().accepted.sequence, 1);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    assert_eq!(storage.status().pending_bytes, 0);
                    assert_eq!(storage.status().pending_batches, 0);
                    assert!(store.data.records.lock().unwrap().is_empty());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("primary_save_retries", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    store.data.control.flush_failures.store(1, Ordering::SeqCst);
                    assert!(runtime.stop().await.is_err());
                    runtime.stop().await.unwrap();
                    assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(store.data.records.lock().unwrap().len(), 1);
                    assert_eq!(store.recovery.entries.lock().unwrap().len(), 1);
                    assert_eq!(storage.status().saved.sequence, 2);
                    assert_eq!(storage.status().recovery_saved.sequence, 1);
                });
        });
    }
    {
        scenario_runtime::check("capture_requires_managed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert!(
                        Runtime::<Model>::builder(0, ())
                            .store_records(
                                limits(1, 4096),
                                Projection {
                                    calls: Arc::new(AtomicUsize::new(0)),
                                    duplicate: false
                                }
                            )
                            .runtime()
                            .await
                            .is_err()
                    );
                });
        });
    }
    {
        scenario_runtime::check("capacity_refuses_projection", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut config = config();
                    config.queue_capacity = NonZeroUsize::new(1).unwrap();
                    let storage = store.manager(config);
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    assert!(
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .is_err()
                    );
                    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    assert_eq!(storage.status().accepted.sequence, 1);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("primary_write_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    store.data.control.failures.store(1, Ordering::SeqCst);
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    assert!(storage.flush().await.is_err());
                    assert!(
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        1
                    );
                    assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(storage.status().recovery_saved.sequence, 0);
                    runtime.stop().await.unwrap();
                    assert_eq!(store.data.records.lock().unwrap().len(), 1);
                    assert_eq!(store.recovery.entries.lock().unwrap().len(), 1);
                });
        });
    }
    {
        scenario_runtime::check("reconciliation_saves_capture", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let observed = Arc::new(tokio::sync::Notify::new());
                    let notify = observed.clone();
                    let check = storage.clone();
                    let runtime = Runtime::<Model>::builder(0, ())
                        .register(Transactions::catalog())
                        .register(Records::catalog())
                        .managed_state(limits(1, 64 * 1024))
                        .store_records(
                            limits(4, 16 * 1024),
                            Projection {
                                calls: store.calls.clone(),
                                duplicate: false,
                            },
                        )
                        .reconcile(|_, storage| async move {
                            storage
                                .write_schema(
                                    &schema(),
                                    &harmos::schema::graph::Schema {
                                        root: 0,
                                        definitions: vec![vec![]],
                                    },
                                )
                                .await?;
                            Ok(())
                        })
                        .service(
                            "producer",
                            move |context: harmos::services::Service<Model>| {
                                let check = check.clone();
                                let notify = notify.clone();
                                async move {
                                    assert_eq!(check.status().saved.sequence, 1);
                                    context
                                        .records()
                                        .publish(Observed { before: 0 })
                                        .await
                                        .unwrap();
                                    notify.notify_one();
                                    Ok::<(), harmos::services::Fault<String>>(())
                                }
                            },
                        )
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    tokio::time::timeout(Duration::from_secs(1), observed.notified())
                        .await
                        .unwrap();
                    assert!(read(&storage).await.is_empty());
                    storage.flush().await.unwrap();
                    assert_eq!(read(&storage).await.len(), 1);
                    assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("capture_groups_reach", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut config = config();
                    config.queue_capacity = NonZeroUsize::new(4096).unwrap();
                    let storage = store.manager(config);
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    store.data.control.write_sizes.lock().unwrap().clear();
                    store.recovery.control.write_sizes.lock().unwrap().clear();
                    for _ in 0..2000 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    storage.flush().await.unwrap();
                    storage.flush_recovery().await.unwrap();

                    assert_eq!(store.data.records.lock().unwrap().len(), 2000);
                    assert_eq!(runtime.state.stored(), Position(4000));
                    assert_eq!(storage.status().pending_bytes, 0);
                    for control in [&store.data.control, &store.recovery.control] {
                        let sizes = control.write_sizes.lock().unwrap();
                        assert!(
                            sizes.len() < 100,
                            "uncoalesced backend calls: {}",
                            sizes.len()
                        );
                        assert!(sizes.iter().all(|size| *size <= 64));
                        assert!(
                            control
                                .writes
                                .lock()
                                .unwrap()
                                .windows(2)
                                .all(|pair| pair[0] < pair[1])
                        );
                    }
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_retries_capture", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    for _ in 0..20 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    let admitted = storage.status().pending_bytes;
                    store
                        .recovery
                        .control
                        .block_write
                        .store(true, Ordering::SeqCst);
                    let flushing = storage.clone();
                    let flushing = tokio::spawn(async move { flushing.flush_recovery().await });
                    store.recovery.control.write_entered.notified().await;
                    assert!(storage.status().pending_bytes > admitted);
                    assert_eq!(storage.status().saved.sequence, 21);
                    assert_eq!(store.data.records.lock().unwrap().len(), 20);
                    store.recovery.control.failures.store(1, Ordering::SeqCst);
                    store
                        .recovery
                        .control
                        .block_write
                        .store(false, Ordering::SeqCst);
                    store.recovery.control.write_release.notify_one();
                    assert!(flushing.await.unwrap().is_err());
                    assert!(storage.status().pending_bytes <= admitted);
                    storage.flush().await.unwrap();
                    storage.flush_recovery().await.unwrap();
                    assert_eq!(store.calls.load(Ordering::SeqCst), 20);
                    assert_eq!(store.data.records.lock().unwrap().len(), 20);
                    assert_eq!(store.recovery.entries.lock().unwrap().len(), 20);
                    assert_eq!(storage.status().pending_bytes, 0);
                    let writes = store.data.control.write_sizes.lock().unwrap().clone();
                    assert_eq!(writes.iter().filter(|size| **size == 20).count(), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("policy_bounds_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for (entries, expected) in [(32, vec![32; 4]), (usize::MAX, vec![128])] {
                        let store = Store::new();
                        let mut policy = config();
                        policy.queue_capacity = NonZeroUsize::new(512).unwrap();
                        policy.write_batch_entries = NonZeroUsize::new(entries).unwrap();
                        policy.write_batch_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
                        policy.target_batch_bytes = policy.queue_capacity_bytes;
                        policy.max_batch_bytes = policy.queue_capacity_bytes;
                        let storage = store.manager(policy);
                        let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                        store.ready(&storage).await;
                        store.data.control.write_sizes.lock().unwrap().clear();
                        store.recovery.control.write_sizes.lock().unwrap().clear();
                        for _ in 0..128 {
                            runtime
                                .state
                                .commit(Advance { panic: false })
                                .await
                                .unwrap();
                        }
                        storage.flush_recovery().await.unwrap();
                        storage.flush().await.unwrap();
                        assert_eq!(*store.data.control.write_sizes.lock().unwrap(), expected);
                        assert_eq!(store.recovery.entries.lock().unwrap().len(), 128);
                        assert_eq!(runtime.state.stored(), Position(256));
                        assert_eq!(storage.status().pending_bytes, 0);
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("barrier_never_wait", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for entries in [1, 64] {
                        let store = Store::new();
                        let mut policy = config();
                        policy.write_batch_entries = NonZeroUsize::new(entries).unwrap();
                        let storage = store.manager(policy);
                        let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                        store.ready(&storage).await;
                        for _ in 0..20 {
                            runtime
                                .state
                                .commit(Advance { panic: false })
                                .await
                                .unwrap();
                        }
                        store
                            .recovery
                            .control
                            .block_write
                            .store(true, Ordering::SeqCst);
                        let saved = tokio::select! {
                            saved = storage.flush() => saved.unwrap(),
                            _ = store.recovery.control.write_entered.notified() => {
                                panic!("a primary-only barrier must not wait for recovery I/O")
                            }
                        };
                        assert_eq!(saved.sequence, 21);
                        assert_eq!(storage.status().recovery_saved.sequence, 0);
                        assert_eq!(storage.status().pending_batches, 20);
                        assert_eq!(runtime.state.stored(), Position(0));
                        store
                            .recovery
                            .control
                            .block_write
                            .store(false, Ordering::SeqCst);
                        storage.flush_recovery().await.unwrap();
                        assert_eq!(runtime.state.stored(), Position(40));
                        assert_eq!(storage.status().pending_bytes, 0);
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("entry_limit_refuses", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut policy = config();
                    policy.max_batch_bytes = NonZeroUsize::new(256).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let storage = store.manager(policy);
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    assert!(
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        0
                    );
                    assert_eq!(storage.cursor_accepted().sequence, 1);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    assert_eq!(storage.status().pending_bytes, 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("follow_replays_unsaved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    for _ in 0..4 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    assert!(read(&storage).await.is_empty());
                    let mut following = storage.follow(captured_query()).await.unwrap();
                    assert_eq!(following.saved_boundary().sequence, 1);
                    assert_eq!(following.accepted_boundary().sequence, 5);
                    for before in 0..4 {
                        let page = following.next_page().await.unwrap();
                        assert_eq!(page.len(), 1);
                        let entry: Envelope<serde_json::Value> =
                            serde_json::from_slice(&page[0].payload).unwrap();
                        assert_eq!(entry.payload["before"], before);
                    }
                    assert_eq!(storage.cursor_saved().sequence, 1);
                    storage.flush().await.unwrap();
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    let page = following.next_page().await.unwrap();
                    assert_eq!(page[0].position.sequence, 6);
                    let entry: Envelope<serde_json::Value> =
                        serde_json::from_slice(&page[0].payload).unwrap();
                    assert_eq!(entry.payload["before"], 4);
                    drop(following);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("capture_replay_outlives", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    store.ready(&storage).await;
                    for _ in 0..4 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    let mut replay = storage.replay(captured_query()).await.unwrap();
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    assert_eq!(replay.boundary().sequence, 5);
                    for before in 0..4 {
                        let page = replay.next_page().await.unwrap().unwrap();
                        let entry: Envelope<serde_json::Value> =
                            serde_json::from_slice(&page[0].payload).unwrap();
                        assert_eq!(entry.payload["before"], before);
                    }
                    assert!(replay.next_page().await.unwrap().is_none());
                    assert_eq!(storage.cursor_saved().sequence, 1);
                    drop(replay);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("ceiling_never_reserve", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(512 * 1024).unwrap();
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let storage = store.manager(policy);
                    let runtime = store
                        .boot(&storage, limits(4, 4 * 1024 * 1024), false)
                        .await;
                    store.ready(&storage).await;

                    for _ in 0..4 {
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .unwrap();
                    }
                    assert_eq!(store.calls.load(Ordering::SeqCst), 4);
                    assert_eq!(storage.cursor_accepted().sequence, 5);
                    assert_eq!(storage.cursor_saved().sequence, 1);
                    assert!(storage.status().pending_bytes < 32 * 1024);
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        4
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("partial_projection_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for cancel in [false, true] {
                        let store = Store::new();
                        let mut policy = config();
                        policy.queue_capacity_bytes = NonZeroUsize::new(256 * 1024).unwrap();
                        policy.max_batch_bytes = policy.queue_capacity_bytes;
                        policy.target_batch_bytes = policy.max_batch_bytes;
                        let storage = store.manager(policy);
                        let runtime = Runtime::<Model>::builder(0, ())
                            .register(Transactions::catalog())
                            .register(Records::catalog())
                            .managed_state(limits(1, 64 * 1024))
                            .store_records(
                                limits(8, 4 * 1024 * 1024),
                                IncrementalProjection {
                                    calls: store.calls.clone(),
                                    data: store.data.clone(),
                                    cancel,
                                },
                            )
                            .runtime_with_storage(&storage)
                            .await
                            .unwrap();
                        store.ready(&storage).await;
                        if cancel {
                            store.data.control.block_write.store(true, Ordering::SeqCst);
                            let item = Identifier::new("already-accepted").unwrap();
                            storage
                                .write_records(&[record(&schema(), &item, &vec![0; 100 * 1024])])
                                .await
                                .unwrap();
                        }
                        let accepted = storage.cursor_accepted();
                        assert!(
                            tokio::time::timeout(
                                Duration::from_secs(2),
                                runtime.state.commit(Advance { panic: false })
                            )
                            .await
                            .unwrap()
                            .is_err()
                        );
                        assert_eq!(store.calls.load(Ordering::SeqCst), 1);
                        assert_eq!(
                            runtime
                                .state
                                .state()
                                .read(|state| *state)
                                .await
                                .unwrap()
                                .value,
                            0
                        );
                        assert_eq!(storage.cursor_accepted(), accepted);
                        assert_eq!(storage.status().recovery_accepted.sequence, 0);
                        store
                            .data
                            .control
                            .block_write
                            .store(false, Ordering::SeqCst);
                        store.data.control.write_release.notify_one();
                        storage.flush().await.unwrap();
                        assert_eq!(storage.status().pending_bytes, 0);
                        assert_eq!(storage.status().pending_batches, 0);
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("registration_uses_managed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::schema::{Described, Validator};

                    #[harmos::message]
                    struct Message {
                        value: String,
                    }

                    let store = Store::new();
                    let storage = store.manager(config());
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), false).await;
                    let key = schema();
                    let definition = runtime
                        .records
                        .register(
                            "plugin",
                            &key,
                            &harmos::schema::graph::Schema::from_declaration(
                                Message::DEFINITION,
                                &harmos::config::SchemaSettings::default(),
                            )
                            .unwrap(),
                        )
                        .await
                        .unwrap();
                    runtime
                        .records
                        .clone()
                        .register(
                            "plugin",
                            &key,
                            &harmos::schema::graph::Schema::from_declaration(
                                Message::DEFINITION,
                                &harmos::config::SchemaSettings::default(),
                            )
                            .unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    storage.flush().await.unwrap();
                    let mut query = schema_query(limits(8, 16 * 1024));
                    query.schema = Some(key.clone());
                    let mut reader = storage.query(query).await.unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(page.len(), 1);
                    assert_eq!(page[0].key, key);
                    assert_eq!(page[0].schema, *definition.graph());
                    let payload = harmos::Message::encode_to_vec(&Message {
                        value: "retained".into(),
                    });
                    drop(definition);
                    Validator::from_schema(
                        &page[0].schema,
                        &harmos::config::SchemaSettings::default(),
                    )
                    .unwrap()
                    .validate(&payload)
                    .unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    drop(reader);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("replica_lag_rolls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let store = Store::new();
                    let target = Arc::new(Data::new(history()));
                    target.control.permanent.store(true, Ordering::SeqCst);
                    let mut policy = config();
                    policy.replica_lag_positions = std::num::NonZeroU64::new(2).unwrap();
                    let id = Identifier::new("capture-replica").unwrap();
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: store.data.clone(),
                            recovery: store.recovery.clone(),
                            replicas: vec![ReplicaTarget {
                                id: id.clone(),
                                backend: target.clone(),
                            }],
                        },
                        policy,
                    )
                    .unwrap();
                    let runtime = store.boot(&storage, limits(4, 16 * 1024), true).await;
                    store.ready(&storage).await;
                    assert!(
                        runtime
                            .state
                            .commit(Advance { panic: false })
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        0
                    );
                    assert_eq!(storage.status().accepted.sequence, 1);
                    assert_eq!(storage.status().recovery_accepted.sequence, 0);
                    assert!(store.data.records.lock().unwrap().is_empty());
                    assert!(storage.status().failure.is_none());
                    target.control.permanent.store(false, Ordering::SeqCst);
                    storage
                        .wait_replicas(&storage.cursor_saved(), &[id])
                        .await
                        .unwrap();
                    runtime
                        .state
                        .commit(Advance { panic: false })
                        .await
                        .unwrap();
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|value| *value)
                            .await
                            .unwrap()
                            .value,
                        1
                    );
                    assert_eq!(storage.status().accepted.sequence, 3);
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
