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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::closed;
use crate::storage::support::{HOUSE, Noted, deposit};
use crate::storage::{Bank, Data, Ledger, Recovery, StorageBackends, StorageManager, catalog, config, history, record, schema};
use harmos_runtime::storage::ReadLimits;
use harmos_runtime::{Builder, Error, Identifier, Runtime, services::ServiceContext, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
struct Balance {
    value: i64,
}

struct Store {
    data: Arc<Data>,
    recovery: Arc<Recovery>,
}

#[derive(Default)]
struct Gate {
    entered: Notify,
    release: Notify,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Store {
    fn new() -> Self {
        Self { data: Arc::new(Data::new(history())), recovery: Arc::new(Recovery::new(history())) }
    }
    fn manager(&self) -> StorageManager {
        StorageManager::new(StorageBackends { primary: self.data.clone(), state: self.recovery.clone() }, config()).unwrap()
    }
    async fn seed(&self) {
        let storage = self.manager();
        let runtime = builder().runtime_with_storage(&storage).await.unwrap();
        storage
            .write_schema(
                schema(),
                harmos_runtime::schema::graph::Schema::from_declaration(
                    <Balance as harmos_runtime::schema::Described>::DEFINITION,
                    &harmos_runtime::config::SchemaSettings::default(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let payload = harmos_runtime::Message::encode(&Balance { value: 8 });
        storage.write_records(vec![record(&schema(), &Identifier::new("saved-balance").unwrap(), &payload)]).await.unwrap();
        runtime.state.commit(deposit(3)).await.unwrap();
        runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn builder() -> Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
        replay: limits(),
        capture: limits(),
        snapshots: None,
        journal_tail: 1024,
    })
}

fn limits() -> ReadLimits {
    ReadLimits { rows: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(64 * 1024).unwrap() }
}

fn failure() -> Error {
    Error::InvalidInput { field: "reconciliation", reason: "controlled failure" }
}

async fn restore(
    journal: harmos_runtime::state::StateManager<Bank>, storage: StorageManager<harmos_runtime::storage::ErasedCatalog>,
) -> harmos_runtime::Result<()> {
    use harmos_runtime::storage::query::transform::{
        Transform,
        project::{Project, Projection},
    };
    use harmos_runtime::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
    let mut reader = storage
        .query(harmos_runtime::storage::Query {
            at: storage.cursor_saved(),
            source: Source::Records(RecordSelection {
                schema: schema(),
                scope: None,
                item: Some(Identifier::new("saved-balance").unwrap()),
                source_start: None,
                source_end: None,
            }),
            transforms: vec![Transform::Project(Project {
                fields: vec![Projection { field: "payload.value".into(), name: "value".into() }],
            })],
            slice: RowSlice::default(),
        })
        .await?;
    let mut saved = Vec::new();
    while let Some(output) = reader.next().await? {
        if let QueryArrow::Batch(batch) = output {
            let values =
                batch.as_record_batch().column_by_name("value").unwrap().as_any().downcast_ref::<arrow_array::Int64Array>().unwrap();
            saved.extend(values.values().iter().copied());
        }
    }
    assert_eq!(saved.len(), 1);
    let saved = saved[0];
    let current = journal.state().read(|state| state.balances.get(&HOUSE).copied().unwrap_or(0)).await?.value;
    if saved > current {
        journal.commit(deposit(saved - current)).await.unwrap();
    }
    Ok(())
}

#[tokio::test]
async fn saved_facts_reconcile() {
    let store = Store::new();
    store.seed().await;
    for _ in 0..2 {
        let storage = store.manager();
        let started = Arc::new(Notify::new());
        let signal = started.clone();
        let saved = storage.clone();
        let runtime = builder()
            .reconcile(restore)
            .service("application", move |context: ServiceContext<Bank>| {
                let signal = signal.clone();
                let saved = saved.clone();
                async move {
                    assert_eq!(context.state().state().read(|state| state.balances[&HOUSE]).await.unwrap().value, 8);
                    assert_eq!(context.state().stored(), Position(2));
                    assert_eq!(saved.status().recovery_saved.sequence, 2);
                    signal.notify_one();
                    Ok::<(), std::io::Error>(())
                }
            })
            .runtime_with_storage(&storage)
            .await
            .unwrap();
        started.notified().await;
        assert_eq!(runtime.state.applied(), Position(2));
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn reconciliation_precedes_services() {
    let store = Store::new();
    store.seed().await;
    let storage = store.manager();
    store.data.control.block_flush.store(true, Ordering::SeqCst);
    store.recovery.control.block_flush.store(true, Ordering::SeqCst);
    let started = Arc::new(AtomicBool::new(false));
    let mark = started.clone();
    let opening = builder()
        .reconcile(|journal, storage| async move {
            storage.write_records(vec![record(&schema(), &Identifier::new("repair-marker").unwrap(), b"saved")]).await?;
            restore(journal, storage).await
        })
        .service("application", move |_context: ServiceContext<Bank>| {
            mark.store(true, Ordering::SeqCst);
            async { Ok::<(), std::io::Error>(()) }
        })
        .runtime_with_storage(&storage);
    tokio::pin!(opening);
    tokio::select! {
        _ = &mut opening => panic!("primary save must block boot"),
        _ = store.data.control.flush_entered.notified() => {}
    }
    assert!(!started.load(Ordering::SeqCst));
    assert_eq!(storage.status().saved.sequence, 2);
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    store.data.control.block_flush.store(false, Ordering::SeqCst);
    store.data.control.flush_release.notify_one();
    tokio::select! {
        _ = &mut opening => panic!("recovery save must block boot"),
        _ = store.recovery.control.flush_entered.notified() => {}
    }
    assert!(!started.load(Ordering::SeqCst));
    assert_eq!(storage.status().saved.sequence, 3);
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    store.recovery.control.block_flush.store(false, Ordering::SeqCst);
    store.recovery.control.flush_release.notify_one();
    let runtime = opening.await.unwrap();
    assert_eq!(runtime.state.stored(), Position(2));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn failed_reconcile_repairs() {
    let store = Store::new();
    let storage = store.manager();
    let started = Arc::new(AtomicBool::new(false));
    let mark = started.clone();
    let result = builder()
        .reconcile(|journal, _| async move {
            journal.commit(deposit(2)).await.unwrap();
            Err(failure())
        })
        .service("application", move |_context: ServiceContext<Bank>| {
            mark.store(true, Ordering::SeqCst);
            async { Ok::<(), std::io::Error>(()) }
        })
        .runtime_with_storage(&storage)
        .await;
    assert!(result.err().unwrap().to_string().contains("reconciliation"));
    assert!(!started.load(Ordering::SeqCst));
    assert!(storage.status().closed);
    assert!(!store.data.control.open.load(Ordering::SeqCst));
    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
    // Boot failure is not a rollback of admitted repairs.
    let storage = store.manager();
    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
    assert_eq!(runtime.state.state().read(|state| state.balances[&HOUSE]).await.unwrap().value, 2);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_reconcile_seals() {
    let store = Store::new();
    let storage = store.manager();
    let gate = Arc::new(Gate::default());
    let entered = gate.clone();
    let retained = Arc::new(std::sync::Mutex::new(None));
    let keep = retained.clone();
    let cleanup;
    {
        let opening = builder()
            .reconcile(move |journal, _| async move {
                journal.commit(deposit(2)).await.unwrap();
                *keep.lock().unwrap() = Some(journal);
                entered.entered.notify_one();
                entered.release.notified().await;
                Ok(())
            })
            .runtime_with_storage(&storage);
        cleanup = opening.cleanup();
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("callback must wait"),
            _ = gate.entered.notified() => {}
        }
    }
    closed(&storage, &cleanup).await;
    let journal = retained.lock().unwrap().take().unwrap();
    assert!(journal.commit(deposit(2)).await.is_err());
    assert_eq!(storage.status().recovery_saved.sequence, 1);
    assert!(!store.data.control.open.load(Ordering::SeqCst));
    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
}

#[tokio::test]
async fn repair_save_blocks() {
    let store = Store::new();
    let storage = store.manager();
    let started = Arc::new(AtomicBool::new(false));
    let mark = started.clone();
    store.recovery.control.flush_failures.store(1, Ordering::SeqCst);
    assert!(
        builder()
            .reconcile(|journal, _| async move {
                harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("repair".into())).await?;
                Ok(())
            })
            .service("application", move |_context: ServiceContext<Bank>| {
                mark.store(true, Ordering::SeqCst);
                async { Ok::<(), std::io::Error>(()) }
            })
            .runtime_with_storage(&storage)
            .await
            .is_err()
    );
    assert!(!started.load(Ordering::SeqCst));
    assert!(storage.status().closed);
    assert_eq!(storage.status().recovery_saved.sequence, 1);
}

#[tokio::test]
async fn reconcile_requires_managed() {
    let store = Store::new();
    let storage = store.manager();
    assert!(
        Runtime::<Bank>::builder(Ledger::default(), ()).reconcile(|_, _| async { Ok(()) }).runtime_with_storage(&storage).await.is_err()
    );
    assert!(!store.data.control.open.load(Ordering::SeqCst));
    assert!(!store.recovery.control.open.load(Ordering::SeqCst));
    let runtime = builder().runtime_with_storage(&storage).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn reconcile_panic_saves() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let store = Store::new();
            let storage = store.manager();
            let startup = builder()
                .reconcile(|journal, _| async move {
                    harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("before panic".into())).await?;
                    panic!("controlled reconciliation panic");
                })
                .runtime_with_storage(&storage);
            let cleanup = startup.cleanup();
            let task = tokio::task::spawn_local(startup);
            assert!(task.await.err().unwrap().is_panic());
            closed(&storage, &cleanup).await;
            assert_eq!(storage.status().recovery_saved.sequence, 1);
        })
        .await;
}

#[tokio::test]
async fn cancelled_boot_settles() {
    let store = Store::new();
    let storage = store.manager();
    store.recovery.control.block_flush.store(true, Ordering::SeqCst);
    let cleanup;
    {
        let opening = builder()
            .reconcile(|journal, _| async move {
                harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("repair".into())).await?;
                Ok(())
            })
            .runtime_with_storage(&storage);
        cleanup = opening.cleanup();
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("save must wait"),
            _ = store.recovery.control.flush_entered.notified() => {}
        }
    }
    assert!(!storage.status().closed);
    store.recovery.control.block_flush.store(false, Ordering::SeqCst);
    store.recovery.control.flush_release.notify_one();
    closed(&storage, &cleanup).await;
    assert_eq!(storage.status().recovery_saved.sequence, 1);
}

/* === Tests === */
