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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::{
    Bank, Data, Ledger, Recovery as Backend, Runtime, StorageBackends, StorageManager, catalog,
    config, history, write,
};
use harmos::{
    Builder, runtime::Incompatible, services::Fault, services::Service, state::Entry,
    state::Position, state::Recorded, storage::JournalConsumer, storage::Recovery, storage::Source,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Gate {
    entered: Notify,
    release: Notify,
}

struct Probe {
    storage: StorageManager,
    data: Arc<Data>,
    recovery: Arc<Backend>,
    load_gate: Option<Arc<Gate>>,
    drain_gate: Option<Arc<Gate>>,
    fail_load: bool,
    panic_drain: bool,
    panic_flush: bool,
    write_drain: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Probe {
    fn new() -> Self {
        let data = Arc::new(Data::new(history()));
        let recovery = Arc::new(Backend::new(history()));
        Self {
            storage: StorageManager::new(
                StorageBackends {
                    primary: data.clone(),
                    recovery: recovery.clone(),
                    replicas: Vec::new(),
                },
                config(),
            )
            .unwrap(),
            data,
            recovery,
            load_gate: None,
            drain_gate: None,
            fail_load: false,
            panic_drain: false,
            panic_flush: false,
            write_drain: false,
        }
    }
}

impl JournalConsumer<Bank> for Probe {
    fn resume(&self) -> Position {
        Position::ORIGIN
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        applied: Position,
    ) -> Position {
        if let Some(gate) = self.drain_gate.take() {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        assert!(!self.panic_drain, "controlled consumer panic");
        if std::mem::take(&mut self.write_drain) {
            write(&self.storage, "boot-consumer").await;
            self.storage.flush().await.unwrap();
        }
        applied
    }

    async fn flush(&mut self) {
        assert!(!self.panic_flush, "controlled final consumer panic");
    }
}

impl Source<Bank> for Probe {
    async fn load(&mut self) -> Result<Recovery<Bank>, Incompatible> {
        assert!(self.data.control.open.load(Ordering::SeqCst));
        assert!(self.recovery.control.open.load(Ordering::SeqCst));
        if let Some(gate) = self.load_gate.take() {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        if self.fail_load {
            return Err(Incompatible::unreadable("controlled load failure"));
        }
        Ok(Recovery::default())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn builder() -> Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog())
}

async fn closed(storage: &StorageManager) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !storage.status().closed || storage.status().running {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled boot cleanup is joined");
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("precedes_load_workers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    probe.write_drain = true;
                    let storage = probe.storage.clone();
                    let data = probe.data.clone();
                    let runtime = builder()
                        .recover(probe)
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    assert_eq!(data.saved.load(Ordering::SeqCst), 1);
                    assert!(runtime.storage().is_some());
                    assert!(runtime.handle().storage().is_some());
                    runtime.stop().await.unwrap();
                    assert!(!data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("starts_lacks_services", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for fail_recovery in [false, true] {
                        let probe = Probe::new();
                        let storage = probe.storage.clone();
                        let control = if fail_recovery {
                            &probe.recovery.control
                        } else {
                            &probe.data.control
                        };
                        control.fail_open.store(true, Ordering::SeqCst);
                        let started = Arc::new(AtomicBool::new(false));
                        let mark = started.clone();
                        let result = builder()
                            .service("application", move |_context: Service<Bank>| {
                                mark.store(true, Ordering::SeqCst);
                                async { Ok::<(), Fault<String>>(()) }
                            })
                            .runtime_with_storage(&storage)
                            .await;
                        assert!(result.is_err());
                        assert!(!started.load(Ordering::SeqCst));
                        assert!(storage.status().closed);
                        assert!(!probe.data.control.open.load(Ordering::SeqCst));
                        assert!(!probe.recovery.control.open.load(Ordering::SeqCst));
                    }
                });
        });
    }
    {
        scenario_runtime::check("acquisition_closes_touched", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for block_recovery in [false, true] {
                        let probe = Probe::new();
                        let storage = probe.storage.clone();
                        let control = if block_recovery {
                            &probe.recovery.control
                        } else {
                            &probe.data.control
                        };
                        control.block_open.store(true, Ordering::SeqCst);
                        {
                            let open = builder().runtime_with_storage(&storage);
                            tokio::pin!(open);
                            tokio::select! {
                                _ = &mut open => panic!("open is blocked"),
                                _ = control.open_entered.notified() => {}
                            }
                        }
                        closed(&storage).await;
                        assert!(!probe.data.control.open.load(Ordering::SeqCst));
                        assert!(!probe.recovery.control.open.load(Ordering::SeqCst));
                    }
                });
        });
    }
    {
        scenario_runtime::check("failure_closes_acquired", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    probe.fail_load = true;
                    let storage = probe.storage.clone();
                    let data = probe.data.clone();
                    let recovery = probe.recovery.clone();
                    assert!(
                        builder()
                            .recover(probe)
                            .runtime_with_storage(&storage)
                            .await
                            .is_err()
                    );
                    assert!(storage.status().closed);
                    assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
                    assert_eq!(recovery.control.closes.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cancellation_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    let gate = Arc::new(Gate::default());
                    probe.load_gate = Some(gate.clone());
                    let storage = probe.storage.clone();
                    let data = probe.data.clone();
                    {
                        let open = builder().recover(probe).runtime_with_storage(&storage);
                        tokio::pin!(open);
                        tokio::select! {
                            _ = &mut open => panic!("load is blocked"),
                            _ = gate.entered.notified() => {}
                        }
                    }
                    closed(&storage).await;
                    assert!(!data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("boot_keeps_runtime", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    let gate = Arc::new(Gate::default());
                    probe.drain_gate = Some(gate.clone());
                    let storage = probe.storage.clone();
                    let data = probe.data.clone();
                    {
                        let open = builder().recover(probe).runtime_with_storage(&storage);
                        tokio::pin!(open);
                        tokio::select! {
                            _ = &mut open => panic!("consumer boot is blocked"),
                            _ = gate.entered.notified() => {}
                        }
                    }
                    assert!(data.control.open.load(Ordering::SeqCst));
                    gate.release.notify_one();
                    closed(&storage).await;
                    assert!(!data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("consumer_panic_during", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    probe.panic_drain = true;
                    let storage = probe.storage.clone();
                    let data = probe.data.clone();
                    let result = builder()
                        .recover(probe)
                        .runtime_with_storage(&storage)
                        .await;
                    let error = result.err().expect("consumer panic refuses boot");
                    assert!(error.to_string().contains("controlled consumer panic"));
                    assert!(storage.status().closed);
                    assert!(!data.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("cleanup_remains_retryable", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut probe = Probe::new();
                    probe.fail_load = true;
                    probe
                        .recovery
                        .control
                        .fail_close
                        .store(true, Ordering::SeqCst);
                    let storage = probe.storage.clone();
                    let recovery = probe.recovery.clone();
                    let error = builder()
                        .recover(probe)
                        .runtime_with_storage(&storage)
                        .await
                        .err()
                        .unwrap();
                    assert!(error.to_string().contains("controlled load failure"));
                    assert!(error.to_string().contains("cleanup also failed"));
                    assert!(recovery.control.open.load(Ordering::SeqCst));
                    recovery.control.fail_close.store(false, Ordering::SeqCst);
                    storage.close().await.unwrap();
                    assert!(storage.status().closed);
                });
        });
    }
    {
        scenario_runtime::check("boot_never_close", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let runtime = builder()
                        .runtime_with_storage(&probe.storage)
                        .await
                        .unwrap();
                    assert!(
                        builder()
                            .runtime_with_storage(&probe.storage)
                            .await
                            .is_err()
                    );
                    assert_eq!(probe.data.control.closes.load(Ordering::SeqCst), 0);
                    write(&probe.storage, "original").await;
                    runtime.stop().await.unwrap();
                    assert_eq!(probe.data.saved.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cannot_replace_infrastructure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for name in ["harmos-storage", "harmos-storage-replica-remote"] {
                        let data = Arc::new(Data::new(history()));
                        let recovery = Arc::new(Backend::new(history()));
                        let target = Arc::new(Data::new(history()));
                        let policy = config();
                        let replicas = vec![harmos::storage::ReplicaTarget {
                            id: harmos::Identifier::new("remote").unwrap(),
                            backend: target.clone(),
                        }];
                        let storage = StorageManager::new(
                            StorageBackends {
                                primary: data.clone(),
                                recovery: recovery.clone(),
                                replicas,
                            },
                            policy,
                        )
                        .unwrap();
                        let result = builder()
                            .service(name, |_context: Service<Bank>| async {
                                Err::<(), Fault<String>>(Fault::Application(
                                    "application service unexpectedly started".to_owned(),
                                ))
                            })
                            .runtime_with_storage(&storage)
                            .await;
                        assert!(result.is_err());
                        assert!(storage.status().closed);
                        assert!(!data.control.open.load(Ordering::SeqCst));
                        assert!(!recovery.control.open.load(Ordering::SeqCst));
                        assert!(!target.control.open.load(Ordering::SeqCst));
                    }
                });
        });
    }
    {
        scenario_runtime::check("service_save_soon", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let storage = probe.storage.clone();
                    let entered = Arc::new(Notify::new());
                    let ready = entered.clone();
                    let runtime = builder()
                        .service("writer", move |mut context: Service<Bank>| {
                            let storage = storage.clone();
                            let ready = ready.clone();
                            async move {
                                write(&storage, "service-start").await;
                                storage.flush().await.unwrap();
                                ready.notify_one();
                                context.stopping().await;
                                Ok::<(), Fault<String>>(())
                            }
                        })
                        .runtime_with_storage(&probe.storage)
                        .await
                        .unwrap();
                    entered.notified().await;
                    assert_eq!(probe.data.saved.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("supervisor_retains_outcome", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for fail_close in [false, true] {
                        let mut probe = Probe::new();
                        probe.panic_flush = true;
                        let storage = probe.storage.clone();
                        let data = probe.data.clone();
                        let recovery = probe.recovery.clone();
                        let runtime = builder()
                            .recover(probe)
                            .runtime_with_storage(&storage)
                            .await
                            .unwrap();
                        recovery
                            .control
                            .fail_close
                            .store(fail_close, Ordering::SeqCst);
                        if fail_close {
                            assert!(runtime.stop().await.is_err());
                            assert!(recovery.control.open.load(Ordering::SeqCst));
                            recovery.control.fail_close.store(false, Ordering::SeqCst);
                        }
                        for _ in 0..2 {
                            let error = runtime.stop().await.unwrap_err();
                            assert!(matches!(
                                error,
                                harmos::Error::TaskFailed {
                                    domain: "journal supervisor",
                                    ..
                                }
                            ));
                            assert!(
                                error
                                    .to_string()
                                    .contains("controlled final consumer panic")
                            );
                        }
                        assert!(!data.control.open.load(Ordering::SeqCst));
                        assert!(!recovery.control.open.load(Ordering::SeqCst));
                        assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
                        assert!(storage.status().closed);
                    }
                });
        });
    }
    {
        scenario_runtime::check("failure_keeps_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    probe
                        .recovery
                        .control
                        .block_open
                        .store(true, Ordering::SeqCst);
                    probe.data.control.fail_close.store(true, Ordering::SeqCst);
                    let cleanup;
                    {
                        let open = builder().runtime_with_storage(&probe.storage);
                        cleanup = open.cleanup();
                        tokio::pin!(open);
                        tokio::select! {
                            _ = &mut open => panic!("acquisition is blocked"),
                            _ = probe.recovery.control.open_entered.notified() => {}
                        }
                    }
                    let failure = tokio::time::timeout(Duration::from_secs(2), cleanup.wait())
                        .await
                        .unwrap()
                        .unwrap_err();
                    assert!(matches!(
                        failure,
                        harmos::Error::TaskFailed {
                            domain: "bootstrap cleanup",
                            ..
                        }
                    ));
                    assert_eq!(cleanup.status(), harmos::runtime::BootstrapStatus::Failed);
                    assert!(cleanup.failure().is_some());
                    assert!(probe.data.control.open.load(Ordering::SeqCst));
                    assert!(probe.storage.status().closing);
                    assert!(!probe.storage.status().closed);
                    probe.data.control.fail_close.store(false, Ordering::SeqCst);
                    cleanup.retry().await.unwrap();
                    assert_eq!(cleanup.status(), harmos::runtime::BootstrapStatus::Settled);
                    assert!(!probe.data.control.open.load(Ordering::SeqCst));
                    assert!(!probe.recovery.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("budget_refuses_acquisition", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let error = builder()
                        .initialize(Duration::ZERO, |_| async { Ok::<(), std::io::Error>(()) })
                        .runtime_with_storage(&probe.storage)
                        .await
                        .err()
                        .unwrap();
                    assert!(
                        error
                            .to_string()
                            .contains("positive representable finite duration")
                    );
                    assert!(!probe.data.control.open.load(Ordering::SeqCst));
                    assert_eq!(probe.data.control.closes.load(Ordering::SeqCst), 0);
                    assert_eq!(probe.recovery.control.closes.load(Ordering::SeqCst), 0);
                });
        });
    }
    {
        scenario_runtime::check("failure_closes_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let mut error = builder()
                        .initialize(Duration::from_secs(1), |_| async {
                            Err::<(), _>(std::io::Error::other("controlled initialization failure"))
                        })
                        .runtime_with_storage(&probe.storage)
                        .await
                        .err()
                        .unwrap();
                    let cause = std::error::Error::source(&error).unwrap();
                    let source = std::error::Error::source(cause).unwrap();
                    assert_eq!(
                        source.downcast_ref::<std::io::Error>().unwrap().to_string(),
                        "controlled initialization failure"
                    );
                    assert!(error.cleanup_error().is_none());
                    error.retry_cleanup().await.unwrap();
                    assert!(probe.storage.status().closed);
                    assert_eq!(probe.data.control.closes.load(Ordering::SeqCst), 1);
                    assert_eq!(probe.recovery.control.closes.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("initialization_timeout_awaits", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let gate = Arc::new(Gate::default());
                    let resident_gate = Arc::clone(&gate);
                    let committed = Arc::new(AtomicBool::new(false));
                    let cleanup_commit = Arc::clone(&committed);
                    let startup = builder()
                        .service("cleanup", move |mut context: Service<Bank>| {
                            let gate = Arc::clone(&resident_gate);
                            let committed = Arc::clone(&cleanup_commit);
                            async move {
                                context.ready();
                                context.stopping().await;
                                gate.entered.notify_one();
                                gate.release.notified().await;
                                let receipt = context
                                    .state()
                                    .commit(super::support::Deposit {
                                        account: super::support::HOUSE,
                                        amount: 1,
                                    })
                                    .await;
                                committed.store(receipt.is_ok(), Ordering::SeqCst);
                                Ok::<(), Fault<std::io::Error>>(())
                            }
                        })
                        .initialize(Duration::from_millis(5), |_| async {
                            std::future::pending::<Result<(), std::io::Error>>().await
                        });
                    let open = startup.runtime_with_storage(&probe.storage);
                    tokio::pin!(open);
                    tokio::select! {
                        _ = &mut open => panic!("physical service cleanup is blocked"),
                        _ = gate.entered.notified() => {}
                    }
                    assert!(probe.data.control.open.load(Ordering::SeqCst));
                    gate.release.notify_one();
                    let error = open.await.err().unwrap();
                    assert!(error.to_string().contains("deadline has elapsed"));
                    assert!(probe.storage.status().closed);
                    assert!(
                        committed.load(Ordering::SeqCst),
                        "journal stays open through physical service cleanup"
                    );
                });
        });
    }
    {
        scenario_runtime::check("initialization_owns_service", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    let startup_gate = Arc::new(Gate::default());
                    let cleanup_gate = Arc::new(Gate::default());
                    let resident_gate = Arc::clone(&cleanup_gate);
                    let callback_gate = Arc::clone(&startup_gate);
                    let service_started = Arc::new(tokio::sync::Notify::new());
                    let resident_started = Arc::clone(&service_started);
                    let startup = builder()
                        .service("cleanup", move |mut context: Service<Bank>| {
                            let gate = Arc::clone(&resident_gate);
                            let started = Arc::clone(&resident_started);
                            async move {
                                context.ready();
                                started.notify_one();
                                context.stopping().await;
                                gate.entered.notify_one();
                                gate.release.notified().await;
                                Ok::<(), Fault<std::io::Error>>(())
                            }
                        })
                        .initialize(Duration::from_secs(1), move |_| {
                            let gate = Arc::clone(&callback_gate);
                            let started = Arc::clone(&service_started);
                            async move {
                                started.notified().await;
                                gate.entered.notify_one();
                                gate.release.notified().await;
                                Ok::<(), std::io::Error>(())
                            }
                        });
                    {
                        let open = startup.runtime_with_storage(&probe.storage);
                        tokio::pin!(open);
                        tokio::select! {
                            _ = &mut open => panic!("initialization is blocked"),
                            _ = startup_gate.entered.notified() => {}
                        }
                    }
                    tokio::time::timeout(Duration::from_secs(2), cleanup_gate.entered.notified())
                        .await
                        .unwrap();
                    assert!(probe.data.control.open.load(Ordering::SeqCst));
                    cleanup_gate.release.notify_one();
                    closed(&probe.storage).await;
                    assert_eq!(probe.data.control.closes.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("error_retains_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Probe::new();
                    probe.data.control.fail_close.store(true, Ordering::SeqCst);
                    let mut error = builder()
                        .initialize(Duration::from_secs(1), |_| async {
                            Err::<(), _>(std::io::Error::other("controlled startup failure"))
                        })
                        .runtime_with_storage(&probe.storage)
                        .await
                        .err()
                        .unwrap();
                    assert!(error.cleanup_error().is_some());
                    assert!(error.to_string().contains("controlled startup failure"));
                    assert!(probe.data.control.open.load(Ordering::SeqCst));
                    assert!(error.retry_cleanup().await.is_err());
                    probe.data.control.fail_close.store(false, Ordering::SeqCst);
                    error.retry_cleanup().await.unwrap();
                    assert!(error.cleanup_error().is_none());
                    assert!(probe.storage.status().closed);
                });
        });
    }
}

/* === Tests === */
