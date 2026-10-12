/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[cfg(feature = "plugin-sidecar")]
mod execution;
#[cfg(feature = "plugin-sidecar")]
mod lifecycle;
#[cfg(feature = "plugin-sidecar")]
mod native;
mod registration;

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Semaphore;
// Local Imports
use harmos::observers::{
    ObserverActivation, ObserverBackend, ObserverDefinition, ObserverKey, ObserverManager,
    ObserverPhase,
};
use harmos::{Error, Runtime};
use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct FinalPublication;

struct ReentrantBackend(Reentrant);

struct Control {
    starts: Semaphore,
    initialize: Semaphore,
    stops: Semaphore,
    finish: Semaphore,
    completed: AtomicUsize,
}

struct Backend {
    id: &'static str,
    mode: Mode,
}

struct Reentrant {
    manager: ObserverManager,
    dropped: Arc<AtomicBool>,
}

/* === Enums === */
#[derive(Clone, Copy)]
enum Mode {
    Cooperative,
    Finishing,
    Failed,
    Panicked,
}

/* === Traits === */

/* === Implementations === */
impl Control {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            starts: Semaphore::new(0),
            initialize: Semaphore::new(1),
            stops: Semaphore::new(0),
            finish: Semaphore::new(0),
            completed: AtomicUsize::new(0),
        })
    }
}

impl ObserverBackend for Backend {
    type Resources = Control;
    fn definition(&self) -> ObserverDefinition {
        ObserverDefinition {
            id: self.id.into(),
            interval: Duration::from_millis(5),
            streams: vec![],
            demand_keys: vec![],
        }
    }

    async fn run(
        &self,
        resources: Arc<Control>,
        mut activation: ObserverActivation,
    ) -> harmos::Result<()> {
        resources.starts.add_permits(1);
        tokio::select! {
            _ = activation.stopping() => return Ok(()),
            initialized = resources.initialize.acquire() => initialized.unwrap().forget(),
        }
        activation.ready();
        assert!(!matches!(self.mode, Mode::Panicked), "backend panicked");
        if matches!(self.mode, Mode::Finishing) {
            activation.begin_stopping();
        } else {
            activation.stopping().await;
        }
        resources.stops.add_permits(1);
        resources.finish.acquire().await.unwrap().forget();
        resources.completed.fetch_add(1, Ordering::SeqCst);
        if matches!(self.mode, Mode::Failed) {
            Err(Error::backend(
                "observer cleanup",
                harmos::BackendFailure::Conflict,
                std::io::Error::other("cleanup conflict"),
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for Reentrant {
    fn drop(&mut self) {
        let _ = self.manager.definitions();
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl ObserverBackend for FinalPublication {
    type Resources = harmos::state::StateManager<Bank>;
    fn definition(&self) -> ObserverDefinition {
        ObserverDefinition {
            id: "final".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        }
    }
    async fn run(
        &self,
        journal: Arc<Self::Resources>,
        mut activation: ObserverActivation,
    ) -> harmos::Result<()> {
        activation.ready();
        activation.stopping().await;
        journal
            .commit(support::deposit(9))
            .await
            .map_err(|source| Error::TaskFailed {
                domain: "observer final publication",
                source: Arc::new(source),
            })?;
        Ok(())
    }
}

impl ObserverBackend for ReentrantBackend {
    type Resources = ();
    fn definition(&self) -> ObserverDefinition {
        let _ = self.0.manager.definitions();
        ObserverDefinition {
            id: "retained".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        }
    }
    async fn run(&self, _: Arc<()>, _: ObserverActivation) -> harmos::Result<()> {
        unreachable!("definition stays inert")
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime()
        .await
        .unwrap()
}

async fn reached(signal: &Semaphore) {
    tokio::time::timeout(Duration::from_secs(2), signal.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
}

async fn phase(manager: &ObserverManager, key: &ObserverKey, phase: ObserverPhase) {
    let mut states = manager.subscribe();
    tokio::time::timeout(
        Duration::from_secs(2),
        states.wait_for(|states| states.get(key).is_some_and(|state| state.phase == phase)),
    )
    .await
    .unwrap()
    .unwrap();
}

fn register(manager: &ObserverManager, id: &'static str, mode: Mode) {
    manager.register(Backend { id, mode }).unwrap();
}

fn own_contract() {
    {
        scenario_runtime::check("cancellation_skips_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "queued", Mode::Cooperative);
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let lease = manager.activate("queued", &scope).await.unwrap();
                    let instance = lease.instance();
                    // The Current-Thread Executor Has Not Polled The Queued Backend Yet.
                    instance.cancel();
                    instance.join().await.unwrap();
                    let lifecycle = instance.lifecycle();
                    assert_eq!(
                        lifecycle.outcome(),
                        Some(harmos::observers::ObserverOutcome::Cancelled)
                    );
                    assert!(lifecycle.started_at().is_none());
                    assert!(lifecycle.settled_at().is_some());
                    assert_eq!(resources.starts.available_permits(), 0);
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 0);
                    lease.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("clones_share_activation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = runtime.handle().observers;
                    register(&manager, "watch", Mode::Cooperative);
                    assert_eq!(runtime.observers.definitions().len(), 1);
                    assert!(matches!(
                        manager.register(Backend {
                            id: "watch",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let first = manager.activate("watch", &scope).await.unwrap();
                    let second = runtime
                        .observers
                        .activate("watch", &scope.clone())
                        .await
                        .unwrap();
                    let key = first.key().clone();
                    assert_eq!(first.id(), second.id());
                    assert_eq!(manager.status(&key).unwrap().id, first.id());
                    reached(&resources.starts).await;
                    phase(&manager, &key, ObserverPhase::Running).await;
                    assert_eq!(manager.status(&key).unwrap().leases, 2);

                    first.release().await.unwrap();
                    assert_eq!(manager.status(&key).unwrap().leases, 1);
                    assert!(resources.stops.try_acquire().is_err());
                    drop(second);
                    reached(&resources.stops).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), manager.shutdown())
                            .await
                            .is_err()
                    );
                    resources.finish.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
                    assert_eq!(manager.status(&key).unwrap().phase, ObserverPhase::Stopped);
                    assert!(matches!(
                        manager.activate("watch", &scope).await,
                        Err(Error::Closing { .. })
                    ));
                });
        });
    }
    {
        scenario_runtime::check("activation_waits_cancelled", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let first = manager.activate("watch", &scope).await.unwrap();
                    let key = first.key().clone();
                    let generation = first.id();
                    phase(manager, &key, ObserverPhase::Running).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), first.release())
                            .await
                            .is_err()
                    );
                    reached(&resources.stops).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            manager.activate("watch", &scope)
                        )
                        .await
                        .is_err()
                    );
                    resources.initialize.add_permits(1);
                    resources.finish.add_permits(1);
                    let next = manager.activate("watch", &scope).await.unwrap();
                    assert_ne!(next.id(), generation);
                    phase(manager, &key, ObserverPhase::Running).await;
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("instance_cancel_joins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let first = manager.activate("watch", &scope).await.unwrap();
                    let second = manager.activate("watch", &scope).await.unwrap();
                    let original = first.instance();
                    let key = original.key().clone();
                    reached(&resources.starts).await;
                    phase(manager, &key, ObserverPhase::Running).await;
                    drop(original.clone());
                    assert_eq!(manager.status(&key).unwrap().leases, 2);
                    assert_eq!(original.id(), second.id());
                    original.cancel();
                    original.cancel();
                    reached(&resources.stops).await;
                    {
                        let joining = original.join();
                        tokio::pin!(joining);
                        std::future::poll_fn(|cx| {
                            assert!(std::future::Future::poll(joining.as_mut(), cx).is_pending());
                            std::task::Poll::Ready(())
                        })
                        .await;
                    }
                    assert!(!original.is_settled());
                    assert!(original.lifecycle().cancellation_requested());
                    assert!(original.lifecycle().settled_at().is_none());
                    resources.finish.add_permits(1);
                    original.join().await.unwrap();
                    assert!(original.is_settled());
                    assert!(original.lifecycle().settled_at().is_some());
                    assert_eq!(
                        original.lifecycle().outcome(),
                        Some(harmos::observers::ObserverOutcome::Cancelled)
                    );
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);

                    resources.initialize.add_permits(1);
                    let replacement = manager.activate("watch", &scope).await.unwrap();
                    phase(manager, &key, ObserverPhase::Running).await;
                    assert_ne!(replacement.id(), original.id());
                    original.cancel();
                    original.join().await.unwrap();
                    first.release().await.unwrap();
                    second.release().await.unwrap();
                    assert_eq!(manager.status(&key).unwrap().leases, 1);
                    assert_eq!(manager.status(&key).unwrap().phase, ObserverPhase::Running);
                    resources.finish.add_permits(1);
                    replacement.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cannot_release_replacement", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "watch", Mode::Finishing);
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let old = manager.activate("watch", &scope).await.unwrap();
                    let key = old.key().clone();
                    reached(&resources.stops).await;
                    phase(manager, &key, ObserverPhase::Stopping).await;
                    resources.finish.add_permits(1);
                    phase(manager, &key, ObserverPhase::Stopped).await;
                    resources.initialize.add_permits(1);
                    let next = manager.activate("watch", &scope).await.unwrap();
                    assert_ne!(old.id(), next.id());
                    old.release().await.unwrap();
                    assert_eq!(manager.status(&key).unwrap().leases, 1);
                    reached(&resources.stops).await;
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scope_stop_revokes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "watch", Mode::Cooperative);
                    let first = Control::new();
                    let second = Control::new();
                    let one = manager.scope(first.clone()).unwrap();
                    let two = manager.scope(second.clone()).unwrap();
                    let a = manager.activate("watch", &one).await.unwrap();
                    let b = manager.activate("watch", &two).await.unwrap();
                    phase(manager, a.key(), ObserverPhase::Running).await;
                    phase(manager, b.key(), ObserverPhase::Running).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), manager.stop_scope(&one))
                            .await
                            .is_err()
                    );
                    assert!(matches!(
                        manager.activate("watch", &one).await,
                        Err(Error::Closing {
                            domain: "observer scope"
                        })
                    ));
                    assert_eq!(
                        manager.status(b.key()).unwrap().phase,
                        ObserverPhase::Running
                    );
                    first.finish.add_permits(1);
                    manager.stop_scope(&one).await.unwrap();
                    assert!(manager.status(a.key()).is_none());
                    a.release().await.unwrap();
                    second.finish.add_permits(1);
                    b.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("unregister_reserves_name", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let scope = manager.scope(resources.clone()).unwrap();
                    let old = manager.activate("watch", &scope).await.unwrap();
                    phase(manager, old.key(), ObserverPhase::Running).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            manager.unregister("watch")
                        )
                        .await
                        .is_err()
                    );
                    assert!(manager.definition("watch").is_some());
                    assert!(matches!(
                        manager.activate("watch", &scope).await,
                        Err(Error::Closing {
                            domain: "observer definition"
                        })
                    ));
                    assert!(matches!(
                        manager.register(Backend {
                            id: "watch",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    resources.finish.add_permits(1);
                    manager.unregister("watch").await.unwrap();
                    assert!(manager.definition("watch").is_none());
                    register(manager, "watch", Mode::Cooperative);
                    resources.initialize.add_permits(1);
                    let next = manager.activate("watch", &scope).await.unwrap();
                    old.release().await.unwrap();
                    phase(manager, next.key(), ObserverPhase::Running).await;
                    assert_eq!(manager.status(next.key()).unwrap().leases, 1);
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scopes_reject_foreigners", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let first = runtime().await;
                    let second = runtime().await;
                    register(&first.observers, "watch", Mode::Cooperative);
                    register(&second.observers, "watch", Mode::Cooperative);
                    let one = first.observers.scope(Control::new()).unwrap();
                    let two = second.observers.scope(Control::new()).unwrap();
                    assert_ne!(one.id(), two.id());
                    assert!(matches!(
                        first.observers.activate("watch", &two).await,
                        Err(Error::InvalidInput { .. })
                    ));
                    let wrong = first.observers.scope(Arc::new(())).unwrap();
                    assert!(matches!(
                        first.observers.activate("watch", &wrong).await,
                        Err(Error::InvalidInput { .. })
                    ));
                    assert!(matches!(
                        first.observers.activate("absent", &one).await,
                        Err(Error::NotRegistered { .. })
                    ));
                    assert!(matches!(
                        first.observers.register(Backend {
                            id: "",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::InvalidInput { .. })
                    ));
                    first.stop().await.unwrap();
                    second.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("runtime_retains_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    register(manager, "bad", Mode::Failed);
                    register(manager, "good", Mode::Cooperative);
                    let bad = Control::new();
                    let good = Control::new();
                    let a = manager
                        .activate("bad", &manager.scope(bad.clone()).unwrap())
                        .await
                        .unwrap();
                    let b = manager
                        .activate("good", &manager.scope(good.clone()).unwrap())
                        .await
                        .unwrap();
                    phase(manager, a.key(), ObserverPhase::Running).await;
                    phase(manager, b.key(), ObserverPhase::Running).await;
                    bad.finish.add_permits(1);
                    good.finish.add_permits(1);
                    for _ in 0..2 {
                        assert!(matches!(
                            runtime.stop().await,
                            Err(Error::TaskFailed {
                                domain: "observer",
                                ..
                            })
                        ));
                    }
                    assert_eq!(good.completed.load(Ordering::SeqCst), 1);
                    assert_eq!(bad.completed.load(Ordering::SeqCst), 1);
                    assert!(matches!(
                        manager.status(a.key()).unwrap().failure.as_deref(),
                        Some(Error::Backend {
                            kind: harmos::BackendFailure::Conflict,
                            ..
                        })
                    ));
                    assert!(a.release().await.is_err());
                    b.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("backend_panic_joined", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "panic", Mode::Panicked);
                    let scope = runtime.observers.scope(Control::new()).unwrap();
                    let lease = runtime.observers.activate("panic", &scope).await.unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Stopping).await;
                    let status = runtime.observers.status(lease.key()).unwrap();
                    let Some(Error::TaskFailed { source, .. }) = status.failure.as_deref() else {
                        panic!("typed panic cause")
                    };
                    assert!(
                        source
                            .downcast_ref::<tokio::task::JoinError>()
                            .unwrap()
                            .is_panic()
                    );
                    assert!(lease.release().await.is_err());
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
    {
        scenario_runtime::check("shutdown_drops_unused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let dropped = Arc::new(AtomicBool::new(false));
                    let resources = Arc::new(Reentrant {
                        manager: runtime.observers.clone(),
                        dropped: dropped.clone(),
                    });
                    let weak = Arc::downgrade(&resources);
                    drop(runtime.observers.scope(resources).unwrap());
                    runtime.stop().await.unwrap();
                    assert!(weak.upgrade().is_none());
                    assert!(dropped.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("shutdown_cancels_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    resources.initialize.acquire().await.unwrap().forget();
                    let scope = runtime.observers.scope(resources.clone()).unwrap();
                    let lease = runtime.observers.activate("watch", &scope).await.unwrap();
                    reached(&resources.starts).await;
                    assert_eq!(
                        runtime.observers.status(lease.key()).unwrap().phase,
                        ObserverPhase::Starting
                    );
                    runtime.stop().await.unwrap();
                    assert_eq!(
                        runtime.observers.status(lease.key()).unwrap().phase,
                        ObserverPhase::Stopped
                    );
                    assert!(resources.stops.try_acquire().is_err());
                    lease.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("runtime_joins_final", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime.observers.register(FinalPublication).unwrap();
                    let scope = runtime
                        .observers
                        .scope(Arc::new(runtime.state.clone()))
                        .unwrap();
                    let lease = runtime.observers.activate("final", &scope).await.unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Running).await;
                    let before = runtime.state.applied();
                    let stopped = runtime.stop().await.unwrap();
                    assert!(stopped > before);
                    assert_eq!(stopped, runtime.state.applied());
                    assert!(matches!(
                        runtime.state.commit(support::deposit(1)).await,
                        Err(Error::Unavailable)
                    ));
                    lease.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("consumers_share_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let scope = runtime.observers.scope(resources.clone()).unwrap();
                    let mut consumers = Vec::new();
                    for _ in 0..32 {
                        let manager = runtime.observers.clone();
                        let scope = scope.clone();
                        consumers.push(tokio::spawn(async move {
                            manager.activate("watch", &scope).await.unwrap()
                        }));
                    }
                    let mut leases = Vec::new();
                    for consumer in consumers {
                        leases.push(consumer.await.unwrap());
                    }
                    let last = leases.pop().unwrap();
                    phase(&runtime.observers, last.key(), ObserverPhase::Running).await;
                    assert_eq!(runtime.observers.status(last.key()).unwrap().leases, 32);
                    reached(&resources.starts).await;
                    assert!(resources.starts.try_acquire().is_err());
                    for lease in leases {
                        assert_eq!(lease.id(), last.id());
                        lease.release().await.unwrap();
                    }
                    assert_eq!(runtime.observers.status(last.key()).unwrap().leases, 1);
                    resources.finish.add_permits(1);
                    last.release().await.unwrap();
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_releases_factories", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let dropped = Arc::new(AtomicBool::new(false));
                    runtime
                        .observers
                        .register(ReentrantBackend(Reentrant {
                            manager: runtime.handle().observers,
                            dropped: dropped.clone(),
                        }))
                        .unwrap();
                    runtime.stop().await.unwrap();
                    assert!(dropped.load(Ordering::SeqCst));
                    assert!(runtime.observers.definitions().is_empty());
                });
        });
    }
    {
        scenario_runtime::check("closing_preserves_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "watch", Mode::Failed);
                    let resources = Control::new();
                    let scope = runtime.observers.scope(resources.clone()).unwrap();
                    let first = runtime.observers.activate("watch", &scope).await.unwrap();
                    let key = first.key().clone();
                    phase(&runtime.observers, &key, ObserverPhase::Running).await;
                    drop(first);
                    reached(&resources.stops).await;
                    let manager = runtime.observers.clone();
                    let mut activating =
                        tokio::spawn(async move { manager.activate("watch", &scope).await });
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), &mut activating)
                            .await
                            .is_err()
                    );
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), runtime.stop())
                            .await
                            .is_err()
                    );
                    resources.finish.add_permits(1);
                    assert!(matches!(
                        activating.await.unwrap(),
                        Err(Error::Closing { .. })
                    ));
                    assert!(matches!(
                        runtime.stop().await,
                        Err(Error::TaskFailed {
                            domain: "observer",
                            ..
                        })
                    ));
                    assert!(runtime.observers.status(&key).unwrap().failure.is_some());
                });
        });
    }
    {
        scenario_runtime::check("retirement_replays_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "returned-failure", Mode::Failed);
                    let control = Control::new();
                    let scope = runtime.observers.scope(control.clone()).unwrap();
                    let lease = runtime
                        .observers
                        .activate("returned-failure", &scope)
                        .await
                        .unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Running).await;
                    control.finish.add_permits(1);
                    for _ in 0..2 {
                        assert!(runtime.observers.stop_scope(&scope).await.is_err());
                    }
                    assert_eq!(control.completed.load(Ordering::Acquire), 1);
                    assert!(runtime.observers.status(lease.key()).is_none());
                    assert!(lease.release().await.is_err());
                    for _ in 0..2 {
                        let error = runtime
                            .stop()
                            .await
                            .expect_err("retired failure reaches root shutdown");
                        let Error::TaskFailed {
                            domain: "observer",
                            source,
                        } = &error
                        else {
                            panic!("root retains observer failure: {error:?}");
                        };
                        let Some(Error::TaskFailed {
                            domain: "retired observer",
                            source,
                        }) = source.downcast_ref::<Error>()
                        else {
                            panic!("retired scope retains its cause: {error:?}");
                        };
                        let Some(Error::TaskFailed {
                            domain: "observer",
                            source,
                        }) = source.downcast_ref::<Error>()
                        else {
                            panic!("original observer outcome is preserved: {error:?}");
                        };
                        assert!(matches!(
                            source.downcast_ref::<Error>(),
                            Some(Error::Backend {
                                operation: "observer cleanup",
                                kind: harmos::BackendFailure::Conflict,
                                ..
                            })
                        ));
                    }
                    assert_eq!(control.completed.load(Ordering::Acquire), 1);
                });
        });
    }
    {
        scenario_runtime::check("retirement_retains_panicked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.observers, "panicked-scope", Mode::Panicked);
                    let scope = runtime.observers.scope(Control::new()).unwrap();
                    let lease = runtime
                        .observers
                        .activate("panicked-scope", &scope)
                        .await
                        .unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Stopping).await;
                    let generation = lease.id();
                    for _ in 0..2 {
                        assert!(runtime.observers.stop_scope(&scope).await.is_err());
                        let lifecycle = lease.instance().lifecycle();
                        assert_eq!(
                            lifecycle.outcome(),
                            Some(harmos::observers::ObserverOutcome::Failed)
                        );
                        assert!(lifecycle.cleanup_pending());
                        assert!(lifecycle.cleanup_failure().is_some());
                        assert!(lifecycle.settled_at().is_none());
                        assert_eq!(
                            runtime.observers.status(lease.key()).unwrap().id,
                            generation
                        );
                        assert!(
                            runtime
                                .observers
                                .activate("panicked-scope", &scope)
                                .await
                                .is_err()
                        );
                    }
                    assert!(lease.release().await.is_err());
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
}

/* === Tests === */

#[test]
fn observer_lifecycle() {
    own_contract();
    registration::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    lifecycle::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    execution::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    native::contract_scenario();
}
