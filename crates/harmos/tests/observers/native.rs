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
use std::io::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Semaphore;
// Local Imports
use super::{phase, runtime};
use harmos::observers::{ObserverLifecyclePhase, ObserverPhase};
use harmos::plugin::sidecar::{Instance, Observer, ObserverContext, ObserverExit};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Counts {
    initialized: AtomicUsize,
    stopped: AtomicUsize,
    ticks: AtomicUsize,
    cleanup: Semaphore,
    refuse_stop: AtomicBool,
    refuse_finalize: AtomicBool,
    finalization: Semaphore,
    finalizations: AtomicUsize,
    stop_attempts: AtomicUsize,
    dropped: AtomicUsize,
}

struct Watch<const PANIC: bool>(Arc<Counts>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Counts {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            initialized: AtomicUsize::new(0),
            stopped: AtomicUsize::new(0),
            ticks: AtomicUsize::new(0),
            cleanup: Semaphore::new(0),
            refuse_stop: AtomicBool::new(false),
            refuse_finalize: AtomicBool::new(false),
            finalization: Semaphore::new(1),
            finalizations: AtomicUsize::new(0),
            stop_attempts: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        })
    }
}

impl<const PANIC: bool> Observer<()> for Watch<PANIC> {
    type Resources = Arc<Counts>;
    type Error = Error;
    const ID: &'static str = "native";
    const INTERVAL: Duration = Duration::from_millis(5);
    async fn initialize(resources: &Arc<Counts>) -> Result<Self, Error> {
        resources.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(Self(resources.clone()))
    }
    async fn observe(&mut self, _: &ObserverContext<'_, Arc<Counts>, ()>) -> Result<(), Error> {
        self.0.ticks.fetch_add(1, Ordering::SeqCst);
        assert!(!PANIC, "native callback panic");
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Error> {
        self.0.stop_attempts.fetch_add(1, Ordering::SeqCst);
        if self.0.refuse_stop.swap(false, Ordering::SeqCst) {
            return Err(Error::other("native cleanup refusal"));
        }
        self.0.cleanup.acquire().await.unwrap().forget();
        self.0.stopped.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Error> {
        self.0.finalizations.fetch_add(1, Ordering::SeqCst);
        if self.0.refuse_finalize.swap(false, Ordering::SeqCst) {
            return Err(Error::other("native finalization refusal"));
        }
        self.0.finalization.acquire().await.unwrap().forget();
        Ok(())
    }
}

impl<const PANIC: bool> Drop for Watch<PANIC> {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("leases_share_existing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner_thread = std::thread::current().id();
                    runtime
                        .observers
                        .register_native(
                            Instance::<Arc<Counts>, ()>::of::<Watch<false>>(),
                            move |_| {
                                assert_ne!(std::thread::current().id(), owner_thread);
                            },
                        )
                        .unwrap();
                    let counts = Counts::new();
                    let scope = runtime.observers.scope(Arc::new(counts.clone())).unwrap();
                    let first = runtime.observers.activate("native", &scope).await.unwrap();
                    let second = runtime.observers.activate("native", &scope).await.unwrap();
                    phase(&runtime.observers, first.key(), ObserverPhase::Running).await;
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while counts.ticks.load(Ordering::SeqCst) < 2 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    first.release().await.unwrap();
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
                    counts.cleanup.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    let ticks = counts.ticks.load(Ordering::SeqCst);
                    second.release().await.unwrap();
                    assert_eq!(counts.ticks.load(Ordering::SeqCst), ticks);
                });
        });
    }
    {
        scenario_runtime::check("native_panic_closes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime
                        .observers
                        .register_native(Instance::<Arc<Counts>, ()>::of::<Watch<true>>(), |_| ())
                        .unwrap();
                    let counts = Counts::new();
                    let scope = runtime.observers.scope(Arc::new(counts.clone())).unwrap();
                    let lease = runtime.observers.activate("native", &scope).await.unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Stopping).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            runtime.observers.activate("native", &scope)
                        )
                        .await
                        .is_err()
                    );
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
                    counts.cleanup.add_permits(1);
                    let failure = lease.release().await.unwrap_err();
                    let harmos::Error::TaskFailed { source, .. } = failure else {
                        panic!("managed failure")
                    };
                    let Some(harmos::Error::TaskFailed { source, .. }) =
                        source.downcast_ref::<harmos::Error>()
                    else {
                        panic!("native failure remains a root error");
                    };
                    assert_eq!(
                        source.downcast_ref::<ObserverExit>(),
                        Some(&ObserverExit::CallbackPanicked)
                    );
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
    {
        scenario_runtime::check("binding_owns_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let counts = Counts::new();
                    let scope = runtime.observers.scope(Arc::new(counts.clone())).unwrap();
                    let bindings = Arc::new(AtomicUsize::new(0));
                    let bound = bindings.clone();
                    let owner_thread = std::thread::current().id();
                    let candidate = runtime
                        .observers
                        .stage_native(
                            &scope,
                            [Instance::<Arc<Counts>, ()>::of::<Watch<false>>()],
                            move |_| {
                                assert_ne!(std::thread::current().id(), owner_thread);
                                bound.fetch_add(1, Ordering::SeqCst);
                            },
                        )
                        .unwrap();
                    candidate.commit().unwrap();
                    assert_eq!(bindings.load(Ordering::SeqCst), 0);
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 0);
                    let first = runtime.observers.activate("native", &scope).await.unwrap();
                    let second = runtime.observers.activate("native", &scope).await.unwrap();
                    phase(&runtime.observers, first.key(), ObserverPhase::Running).await;
                    assert_eq!(first.id(), second.id());
                    assert_eq!(bindings.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
                    counts.cleanup.add_permits(1);
                    runtime.observers.stop_scope(&scope).await.unwrap();
                    first.release().await.unwrap();
                    second.release().await.unwrap();
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert!(runtime.observers.definition("native").is_none());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_keeps_manager", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    cleanup_recovery(false).await;
                });
        });
    }
    {
        scenario_runtime::check("failure_preserves_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    cleanup_recovery(true).await;
                });
        });
    }
    {
        scenario_runtime::check("handles_keep_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime
                        .observers
                        .register_native(Instance::<Arc<Counts>, ()>::of::<Watch<false>>(), |_| ())
                        .unwrap();
                    let counts = Counts::new();
                    counts.refuse_stop.store(true, Ordering::SeqCst);
                    let scope = runtime.observers.scope(Arc::new(counts.clone())).unwrap();
                    let lease = runtime.observers.activate("native", &scope).await.unwrap();
                    phase(&runtime.observers, lease.key(), ObserverPhase::Running).await;
                    let instance = lease.instance();
                    instance.cancel();
                    assert!(instance.join().await.is_err());
                    drop(instance);
                    drop(lease);
                    drop(scope);
                    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
                    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 1);

                    counts.cleanup.add_permits(1);
                    assert!(runtime.observers.shutdown().await.is_err());
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.finalizations.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.dropped.load(Ordering::SeqCst), 1);
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
}

async fn cleanup_recovery(finalizing: bool) {
    let runtime = runtime().await;
    runtime
        .observers
        .register_native(Instance::<Arc<Counts>, ()>::of::<Watch<false>>(), |_| ())
        .unwrap();
    let counts = Counts::new();
    let expected = if finalizing {
        counts.cleanup.add_permits(1);
        counts.finalization.try_acquire().unwrap().forget();
        counts.refuse_finalize.store(true, Ordering::SeqCst);
        ObserverLifecyclePhase::Finalizing
    } else {
        counts.refuse_stop.store(true, Ordering::SeqCst);
        ObserverLifecyclePhase::Stopping
    };
    let scope = runtime.observers.scope(Arc::new(counts.clone())).unwrap();
    let lease = runtime.observers.activate("native", &scope).await.unwrap();
    phase(&runtime.observers, lease.key(), ObserverPhase::Running).await;
    let instance = lease.instance();
    instance.cancel();
    assert!(instance.join().await.is_err());
    assert!(!instance.is_settled());
    assert!(instance.lifecycle().cleanup_pending());
    assert!(instance.lifecycle().cleanup_failure().is_some());
    assert_eq!(instance.lifecycle().phase(), expected);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), instance.join())
            .await
            .is_err()
    );
    assert_eq!(
        counts.stop_attempts.load(Ordering::SeqCst),
        if finalizing { 1 } else { 2 }
    );
    assert_eq!(
        counts.finalizations.load(Ordering::SeqCst),
        if finalizing { 2 } else { 0 }
    );
    assert_eq!(instance.lifecycle().phase(), expected);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    if finalizing {
        counts.finalization.add_permits(1);
    } else {
        counts.cleanup.add_permits(1);
    }
    assert!(
        instance.join().await.is_err(),
        "the original logical failure remains observable"
    );
    assert!(instance.is_settled());
    assert!(!instance.lifecycle().cleanup_pending());
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(lease.release().await.is_err());
    assert!(runtime.stop().await.is_err());
}

/* === Tests === */
