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
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::{Notify, Semaphore, watch};

// Local Imports
use super::app::App;
use harmos::services::{
    ServiceDefinition, ServiceKey, ServiceLease, ServiceLifecycle, ServiceOutcome, ServicePhase,
};
use harmos::{
    Runtime, services::Fault, services::Resident, services::Service, services::ServicePolicy,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Trace {
    factories: AtomicUsize,
    attempts: AtomicUsize,
    finalizers: AtomicUsize,
    drops: AtomicUsize,
    fail_cleanup: AtomicBool,
    cleanup_entered: Notify,
    cleanup_release: Semaphore,
    mode: Mode,
    block_cleanup: bool,
}

struct Worker(Arc<Trace>);

/* === Enums === */
#[derive(Clone, Copy)]
enum Mode {
    Complete,
    Retry,
}

/* === Traits === */

/* === Implementations === */
impl Resident<App> for Worker {
    type Error = std::io::Error;

    async fn attempt(&mut self, mut context: Service<App>) -> Result<(), Fault<Self::Error>> {
        let attempt = self.0.attempts.fetch_add(1, Ordering::SeqCst);
        context.ready();
        match self.0.mode {
            Mode::Complete => Ok(()),
            Mode::Retry if attempt == 0 => {
                Err(Fault::Environmental(std::io::Error::other("disconnected")))
            }
            Mode::Retry => {
                context.stopping().await;
                Ok(())
            }
        }
    }

    async fn finalize(&mut self) -> Result<(), Self::Error> {
        self.0.finalizers.fetch_add(1, Ordering::SeqCst);
        self.0.cleanup_entered.notify_one();
        if self.0.fail_cleanup.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("retained cleanup"));
        }
        if self.0.block_cleanup {
            self.0.cleanup_release.acquire().await.unwrap().forget();
        }
        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn trace(mode: Mode, block_cleanup: bool, fail_cleanup: bool) -> Arc<Trace> {
    Arc::new(Trace {
        factories: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
        finalizers: AtomicUsize::new(0),
        drops: AtomicUsize::new(0),
        fail_cleanup: AtomicBool::new(fail_cleanup),
        cleanup_entered: Notify::new(),
        cleanup_release: Semaphore::new(0),
        mode,
        block_cleanup,
    })
}

async fn runtime(
    mode: Mode,
    block_cleanup: bool,
    fail_cleanup: bool,
    backoff: Duration,
) -> (Runtime<App>, ServiceLease<App>, Arc<Trace>) {
    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
    let trace = trace(mode, block_cleanup, fail_cleanup);
    runtime
        .services
        .register(
            ServiceDefinition {
                id: "worker".into(),
                policy: ServicePolicy {
                    backoff_base: backoff,
                    backoff_cap: backoff,
                    ..ServicePolicy::default()
                },
            },
            {
                let trace = trace.clone();
                move |_: Arc<()>| {
                    trace.factories.fetch_add(1, Ordering::SeqCst);
                    Worker(trace.clone())
                }
            },
        )
        .unwrap();
    let lease = runtime
        .services
        .activate("worker", &runtime.services.root_scope())
        .await
        .unwrap();
    (runtime, lease, trace)
}

async fn observed(
    receiver: &mut watch::Receiver<ServiceLifecycle>,
    wanted: impl Fn(&ServiceLifecycle) -> bool,
) -> ServiceLifecycle {
    tokio::time::timeout(Duration::from_secs(1), receiver.wait_for(wanted))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("service_skips_factory", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, lease, trace) =
                        runtime(Mode::Complete, false, false, Duration::ZERO).await;
                    assert_eq!(lease.lifecycle().phase(), ServicePhase::Queued);
                    let mut lifecycle = lease.subscribe_lifecycle();
                    lease.release().await.unwrap();
                    let status = observed(&mut lifecycle, |status| {
                        status.phase() == ServicePhase::Settled
                    })
                    .await;
                    assert_eq!(status.outcome(), Some(ServiceOutcome::Cancelled));
                    assert!(status.cancellation_requested());
                    assert!(status.started_at().is_none());
                    assert!(status.settled_at().is_some());
                    assert_eq!(trace.factories.load(Ordering::SeqCst), 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("settlement_survives_cancel", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, lease, trace) =
                        runtime(Mode::Complete, true, false, Duration::ZERO).await;
                    let snapshots = runtime.services.subscribe();
                    trace.cleanup_entered.notified().await;
                    let pending = lease.lifecycle();
                    assert_eq!(pending.phase(), ServicePhase::Finalizing);
                    assert_eq!(pending.outcome(), Some(ServiceOutcome::Succeeded));
                    assert!(pending.cleanup_pending());
                    assert!(pending.settled_at().is_none());
                    let snapshot = runtime
                        .services
                        .status(&ServiceKey::root("worker"))
                        .unwrap();
                    assert_eq!(snapshot.lifecycle.phase(), ServicePhase::Finalizing);
                    assert_eq!(
                        snapshots
                            .borrow()
                            .get(&ServiceKey::root("worker"))
                            .unwrap()
                            .lifecycle
                            .phase(),
                        ServicePhase::Finalizing
                    );
                    assert!(runtime.stop().now_or_never().is_none());
                    assert!(lease.lifecycle().cancellation_requested());
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 0);
                    trace.cleanup_release.add_permits(1);
                    runtime.stop().await.unwrap();
                    let settled = lease.lifecycle();
                    assert_eq!(settled.phase(), ServicePhase::Settled);
                    assert_eq!(settled.outcome(), Some(ServiceOutcome::Succeeded));
                    assert!(!settled.cleanup_pending());
                    assert!(settled.settled_at().is_some());
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cleanup_retains_success", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, lease, trace) =
                        runtime(Mode::Complete, false, true, Duration::ZERO).await;
                    let mut lifecycle = lease.subscribe_lifecycle();
                    let failed =
                        observed(&mut lifecycle, |status| status.cleanup_failure().is_some()).await;
                    assert_eq!(failed.phase(), ServicePhase::Finalizing);
                    assert_eq!(failed.outcome(), Some(ServiceOutcome::Succeeded));
                    assert!(failed.cleanup_pending());
                    assert!(failed.settled_at().is_none());
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 0);
                    assert!(runtime.stop().await.is_err());
                    trace.fail_cleanup.store(false, Ordering::SeqCst);
                    runtime.stop().await.unwrap();
                    assert_eq!(lease.lifecycle().phase(), ServicePhase::Settled);
                    assert_eq!(lease.lifecycle().outcome(), Some(ServiceOutcome::Succeeded));
                    assert!(lease.lifecycle().cleanup_failure().is_none());
                    assert_eq!(trace.factories.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 2);
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cancel_wakes_backoff", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, lease, trace) =
                        runtime(Mode::Retry, false, false, Duration::from_secs(100)).await;
                    let mut lifecycle = lease.subscribe_lifecycle();
                    let waiting = observed(&mut lifecycle, |status| {
                        status.phase() == ServicePhase::RetryWaiting
                    })
                    .await;
                    assert_eq!(waiting.attempt(), 0);
                    assert_eq!(waiting.outcome(), None);
                    assert!(!waiting.cleanup_pending());
                    tokio::time::timeout(Duration::from_secs(1), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(lease.lifecycle().outcome(), Some(ServiceOutcome::Cancelled));
                    assert_eq!(lease.lifecycle().phase(), ServicePhase::Settled);
                    assert_eq!(trace.attempts.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("restart_uses_control", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, lease, trace) =
                        runtime(Mode::Retry, false, false, Duration::from_millis(1)).await;
                    let mut lifecycle = lease.subscribe_lifecycle();
                    let running = observed(&mut lifecycle, |status| {
                        status.phase() == ServicePhase::Running
                    })
                    .await;
                    assert_eq!(running.attempt(), 1);
                    assert_eq!(running.outcome(), None);
                    assert_eq!(trace.factories.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.attempts.load(Ordering::SeqCst), 2);
                    assert!(matches!(
                        runtime.services.status(lease.key()).unwrap().status,
                        harmos::services::ServiceStatus::Running { credit: 2 }
                    ));
                    // Cancellation Must Not Wait For The Pending 250 ms Health Refill.
                    tokio::time::timeout(Duration::from_millis(50), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(lease.lifecycle().outcome(), Some(ServiceOutcome::Cancelled));
                    assert_eq!(lease.lifecycle().attempt(), 1);
                    assert_eq!(lease.lifecycle().phase(), ServicePhase::Settled);
                    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 2);
                });
        });
    }
    {
        scenario_runtime::check("cancelled_queued_resident", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let trace = trace(Mode::Complete, false, false);
                    let runtime = Runtime::<App>::builder((), ())
                        .service_with("owned", ServicePolicy::default(), Worker(trace.clone()))
                        .runtime()
                        .await
                        .unwrap();
                    let snapshot = runtime.services.status(&ServiceKey::root("owned")).unwrap();
                    assert_eq!(snapshot.lifecycle.phase(), ServicePhase::Queued);
                    runtime.stop().await.unwrap();
                    assert_eq!(trace.attempts.load(Ordering::SeqCst), 0);
                    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
                });
        });
    }
}
