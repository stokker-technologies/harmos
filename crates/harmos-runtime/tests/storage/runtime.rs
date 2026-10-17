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
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::{
    Bank, Data, Fixture, Ledger, Recovery, Runtime, StorageBackends, StorageManager, catalog,
    config, history, write,
};
use harmos_runtime::{
    Error, services::Fault, services::Service, state::Entry, state::Position, state::Recorded,
    storage::JournalConsumer,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct FinalDrain(StorageManager);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl JournalConsumer<Bank> for FinalDrain {
    fn resume(&self) -> Position {
        Position::ORIGIN
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        applied: Position,
    ) -> Position {
        applied
    }

    async fn flush(&mut self) {
        write(&self.0, "journal-final-flush").await;
        self.0.flush().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn dormant() -> Fixture {
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime()
        .await
        .unwrap();
    let data = Arc::new(Data::new(history()));
    let recovery = Arc::new(Recovery::new(history()));
    let settings = config();
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        settings,
    )
    .unwrap();
    Fixture {
        runtime,
        storage,
        data,
        recovery,
    }
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("runtime_owns_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    let handle = f.runtime.handle();
                    assert!(handle.storage().is_none());
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    assert!(matches!(
                        f.runtime.operations.submit_resident("harmos-storage").await,
                        Err(Error::InvalidInput {
                            field: "service definition",
                            ..
                        })
                    ));
                    write(handle.storage().unwrap(), "final").await;
                    f.storage.write_recovery(&[b"logical-state"]).await.unwrap();
                    let settled = f.runtime.stop().await.unwrap();
                    assert_eq!(f.runtime.stop().await.unwrap(), settled);
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(f.recovery.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 1);
                    assert_eq!(f.recovery.control.closes.load(Ordering::SeqCst), 1);
                    assert!(!f.storage.status().running);
                });
        });
    }
    {
        scenario_runtime::check("producers_flush_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    let storage = f.storage.clone();
                    let entered = Arc::new(tokio::sync::Notify::new());
                    let started = entered.clone();
                    f.runtime
                        .operations
                        .register_service(
                            harmos_runtime::services::ServiceDefinition {
                                id: "final-evidence".into(),
                                policy: harmos_runtime::services::POLICY,
                            },
                            move |_: Arc<()>| {
                                let storage = storage.clone();
                                let started = started.clone();
                                move |mut context: Service<Bank>| {
                                    let storage = storage.clone();
                                    let started = started.clone();
                                    async move {
                                        started.notify_one();
                                        context.stopping().await;
                                        write(&storage, "service").await;
                                        storage.flush().await.unwrap();
                                        Ok::<(), Fault<String>>(())
                                    }
                                }
                            },
                        )
                        .unwrap();
                    let _lease = f
                        .runtime
                        .operations
                        .submit_resident("final-evidence")
                        .await
                        .unwrap();
                    entered.notified().await;
                    let mut participant = f.runtime.stop_participant();
                    let storage = f.storage.clone();
                    let task = tokio::spawn(async move {
                        participant.stopping().await;
                        assert_eq!(storage.status().saved.sequence, 1);
                        write(&storage, "participant").await;
                        storage.flush().await.unwrap();
                        participant.finish();
                    });
                    f.runtime.stop().await.unwrap();
                    task.await.unwrap();
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 2);
                    assert_eq!(f.data.records.lock().unwrap().len(), 2);
                });
        });
    }
    {
        scenario_runtime::check("stop_retains_frontier", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.data.control.permanent.store(true, Ordering::SeqCst);
                    write(&f.storage, "retry").await;
                    assert!(f.runtime.stop().await.is_err());
                    assert!(f.data.control.open.load(Ordering::SeqCst));
                    assert!(f.storage.status().closing);
                    assert!(matches!(
                        f.runtime.start_storage(f.storage.clone()).await,
                        Err(Error::Closing { domain: "runtime" })
                    ));
                    f.data.control.permanent.store(false, Ordering::SeqCst);
                    assert_eq!(f.runtime.stop().await.unwrap(), Position::ORIGIN);
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(f.data.records.lock().unwrap().len(), 1);
                });
        });
    }
    {
        scenario_runtime::check("close_retries_journal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.recovery.control.fail_close.store(true, Ordering::SeqCst);
                    write(&f.storage, "saved").await;
                    assert!(f.runtime.stop().await.is_err());
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 1);
                    f.recovery.control.fail_close.store(false, Ordering::SeqCst);
                    let (first, second) = tokio::join!(f.runtime.stop(), f.runtime.stop());
                    assert_eq!(first.unwrap(), second.unwrap());
                    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 1);
                    assert_eq!(f.recovery.control.closes.load(Ordering::SeqCst), 2);
                });
        });
    }
    {
        scenario_runtime::check("cancelled_stop_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.data.control.block_flush.store(true, Ordering::SeqCst);
                    write(&f.storage, "pending").await;
                    {
                        let stop = f.runtime.stop();
                        tokio::pin!(stop);
                        tokio::select! {
                            _ = &mut stop => panic!("flush is blocked"),
                            _ = f.data.control.flush_entered.notified() => {}
                        }
                    }
                    assert!(f.data.control.open.load(Ordering::SeqCst));
                    assert!(f.storage.status().running);
                    f.data.control.block_flush.store(false, Ordering::SeqCst);
                    f.data.control.flush_release.notify_one();
                    f.runtime.stop().await.unwrap();
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cancelled_acquisition_retained", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for recovery in [false, true] {
                        let f = dormant().await;
                        let handle = f.runtime.handle();
                        let control = if recovery {
                            &f.recovery.control
                        } else {
                            &f.data.control
                        };
                        control.block_open.store(true, Ordering::SeqCst);
                        {
                            let start = f.runtime.start_storage(f.storage.clone());
                            tokio::pin!(start);
                            tokio::select! {
                                _ = &mut start => panic!("acquisition is blocked"),
                                _ = control.open_entered.notified() => {}
                            }
                        }
                        assert!(handle.storage().is_some());
                        f.runtime.stop().await.unwrap();
                        assert!(!f.data.control.open.load(Ordering::SeqCst));
                        assert!(!f.recovery.control.open.load(Ordering::SeqCst));
                    }
                });
        });
    }
    {
        scenario_runtime::check("failure_retains_acquisition", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.recovery.control.fail_open.store(true, Ordering::SeqCst);
                    assert!(f.runtime.start_storage(f.storage.clone()).await.is_err());
                    assert!(f.runtime.storage().is_some());
                    f.runtime.stop().await.unwrap();
                    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 1);
                    assert_eq!(f.recovery.control.closes.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("managers_cannot_replaced", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let first = dormant().await;
                    let second = dormant().await;
                    first
                        .runtime
                        .start_storage(first.storage.clone())
                        .await
                        .unwrap();
                    assert!(
                        first
                            .runtime
                            .start_storage(second.storage.clone())
                            .await
                            .is_err()
                    );
                    assert!(
                        second
                            .runtime
                            .start_storage(first.storage.clone())
                            .await
                            .is_err()
                    );
                    assert!(second.runtime.storage().is_none());
                    second.runtime.stop().await.unwrap();
                    write(&first.storage, "still-owned").await;
                    first.runtime.stop().await.unwrap();
                    assert_eq!(first.data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(second.data.control.closes.load(Ordering::SeqCst), 0);
                });
        });
    }
    {
        scenario_runtime::check("cancelled_acquisition_retried", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.recovery.control.block_open.store(true, Ordering::SeqCst);
                    {
                        let start = f.runtime.start_storage(f.storage.clone());
                        tokio::pin!(start);
                        tokio::select! {
                            _ = &mut start => panic!("acquisition is blocked"),
                            _ = f.recovery.control.open_entered.notified() => {}
                        }
                    }
                    f.recovery.control.block_open.store(false, Ordering::SeqCst);
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    write(&f.storage, "after-retry").await;
                    f.runtime.stop().await.unwrap();
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("shutdown_keeps_worker", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let data = Arc::new(Data::new(history()));
                    let recovery = Arc::new(Recovery::new(history()));
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: data.clone(),
                            state: recovery,
                        },
                        config(),
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .attach(FinalDrain(storage.clone()))
                        .runtime()
                        .await
                        .unwrap();
                    runtime.start_storage(storage).await.unwrap();
                    runtime.stop().await.unwrap();
                    assert_eq!(data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(data.records.lock().unwrap().len(), 1);
                });
        });
    }
    {
        scenario_runtime::check("stop_seals_registration", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    let mut participant = f.runtime.stop_participant();
                    f.recovery.control.block_open.store(true, Ordering::SeqCst);
                    {
                        let start = f.runtime.start_storage(f.storage.clone());
                        tokio::pin!(start);
                        tokio::select! {
                            _ = &mut start => panic!("acquisition is blocked"),
                            _ = f.recovery.control.open_entered.notified() => {}
                        }
                        let stop = f.runtime.stop();
                        tokio::pin!(stop);
                        let pending = std::future::poll_fn(|cx| {
                            std::task::Poll::Ready(
                                std::future::Future::poll(stop.as_mut(), cx).is_pending(),
                            )
                        })
                        .await;
                        assert!(
                            pending,
                            "shutdown retains the participant and startup owners"
                        );
                    }
                    tokio::time::timeout(std::time::Duration::from_secs(2), participant.stopping())
                        .await
                        .expect("the original shutdown driver advances without another waiter");
                    participant.finish();
                    assert!(matches!(
                        f.runtime.start_storage(f.storage.clone()).await,
                        Err(Error::Closing { domain: "runtime" })
                    ));
                    f.runtime.stop().await.unwrap();
                    assert!(!f.data.control.open.load(Ordering::SeqCst));
                    assert!(!f.recovery.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("workers_not_started", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.runtime.stop().await.unwrap();
                    let status = f.storage.status();
                    assert!(status.closed);
                    assert!(!status.running);
                    assert!(
                        f.runtime
                            .operations
                            .resident(&harmos_runtime::operations::resident::Key::root("harmos-storage"))
                            .and_then(|item| item.health)
                            .is_none()
                    );
                });
        });
    }
    {
        scenario_runtime::check("batch_retries_producer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = dormant().await;
                    f.runtime.start_storage(f.storage.clone()).await.unwrap();
                    f.data.control.flush_failures.store(1, Ordering::SeqCst);
                    write(&f.storage, "uncertain").await;
                    assert!(f.runtime.stop().await.is_err());
                    assert_eq!(f.data.saved.load(Ordering::SeqCst), 1);
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    assert!(f.storage.status().pending_bytes > 0);
                    f.runtime.stop().await.unwrap();
                    assert_eq!(f.storage.status().saved.sequence, 1);
                    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1, 1]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                });
        });
    }
}

/* === Tests === */
