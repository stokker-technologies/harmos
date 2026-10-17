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
use harmos::operations::{OperationOutcome, OperationPhase, OperationStatus};
use harmos::services::ServiceDefinition;
use harmos::{Runtime, services::Fault, services::Resident, services::Service};

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

    async fn run(&mut self, mut context: Service<App>) -> Result<(), Fault<Self::Error>> {
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

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
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
) -> (
    Runtime<App>,
    harmos::operations::OperationInstance<harmos::operations::resident::Activation>,
    Arc<Trace>,
) {
    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
    let trace = trace(mode, block_cleanup, fail_cleanup);
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "worker".into(),
                policy: harmos::operations::OperationPolicy {
                    restart: harmos::operations::RestartPolicy::OnTransient {
                        credit: 3,
                        sustained_health: Duration::from_millis(250),
                        backoff: harmos::operations::Backoff::Exponential {
                            initial: backoff,
                            maximum: backoff,
                            jitter: harmos::operations::Jitter::Deterministic,
                        },
                    },
                    ..harmos::services::POLICY
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
    let lease = runtime.operations.submit_resident("worker").await.unwrap();
    (runtime, lease, trace)
}

async fn observed(
    receiver: &mut watch::Receiver<OperationStatus>,
    wanted: impl Fn(&OperationStatus) -> bool,
) -> OperationStatus {
    tokio::time::timeout(Duration::from_secs(1), receiver.wait_for(wanted))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

/* === Tests === */

#[tokio::test]
async fn service_skips_factory() {
    let (runtime, lease, trace) =
        runtime(Mode::Complete, false, false, Duration::from_millis(1)).await;
    assert_eq!(lease.status().phase(), OperationPhase::Queued);
    let mut lifecycle = lease.subscribe();
    lease.release().await.unwrap();
    let status = observed(&mut lifecycle, |status| {
        status.phase() == OperationPhase::Stopped
    })
    .await;
    assert_eq!(status.outcome(), Some(OperationOutcome::Cancelled));
    assert!(status.cancellation_requested());
    assert!(status.started_at().is_none());
    assert!(status.settled_at().is_some());
    assert_eq!(trace.factories.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn settlement_survives_cancel() {
    let (runtime, lease, trace) =
        runtime(Mode::Complete, true, false, Duration::from_millis(1)).await;
    let snapshots = runtime.operations.watch_residents();
    trace.cleanup_entered.notified().await;
    let pending = lease.status();
    assert_eq!(pending.phase(), OperationPhase::Finalizing);
    assert_eq!(pending.outcome(), Some(OperationOutcome::Succeeded));
    assert!(pending.cleanup_pending());
    assert!(pending.settled_at().is_none());
    let snapshot = runtime
        .operations
        .resident(&harmos::operations::resident::Key::root("worker"))
        .unwrap();
    assert_eq!(snapshot.lifecycle.phase(), OperationPhase::Finalizing);
    assert_eq!(
        snapshots
            .borrow()
            .get(&harmos::operations::resident::Key::root("worker"))
            .unwrap()
            .lifecycle
            .phase(),
        OperationPhase::Finalizing
    );
    let mut lifecycle = lease.subscribe();
    assert!(runtime.stop().now_or_never().is_none());
    // The Original Driver Continues After Its Waiter Is Dropped.
    // Plugin Shutdown Settles Before Application Services Are Signalled.
    let cancelled = observed(&mut lifecycle, |status| status.cancellation_requested()).await;
    assert!(lease.status().cancellation_requested());
    assert_eq!(cancelled.phase(), OperationPhase::Finalizing);
    assert_eq!(cancelled.outcome(), Some(OperationOutcome::Succeeded));
    assert!(cancelled.cleanup_pending());
    assert_eq!(trace.drops.load(Ordering::SeqCst), 0);
    trace.cleanup_release.add_permits(1);
    runtime.stop().await.unwrap();
    let settled = lease.status();
    assert_eq!(settled.phase(), OperationPhase::Stopped);
    assert_eq!(settled.outcome(), Some(OperationOutcome::Succeeded));
    assert!(!settled.cleanup_pending());
    assert!(settled.settled_at().is_some());
    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cleanup_retains_success() {
    let (runtime, lease, trace) =
        runtime(Mode::Complete, false, true, Duration::from_millis(1)).await;
    let mut lifecycle = lease.subscribe();
    let failed = observed(&mut lifecycle, |status| status.cleanup_failure().is_some()).await;
    assert_eq!(failed.phase(), OperationPhase::Finalizing);
    assert_eq!(failed.outcome(), Some(OperationOutcome::Succeeded));
    assert!(failed.cleanup_pending());
    assert!(failed.settled_at().is_none());
    assert_eq!(trace.drops.load(Ordering::SeqCst), 0);
    assert!(runtime.stop().await.is_err());
    trace.fail_cleanup.store(false, Ordering::SeqCst);
    runtime.stop().await.unwrap();
    assert_eq!(lease.status().phase(), OperationPhase::Stopped);
    assert_eq!(lease.status().outcome(), Some(OperationOutcome::Succeeded));
    assert!(lease.status().cleanup_failure().is_none());
    assert_eq!(trace.factories.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 2);
    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn cancel_wakes_backoff() {
    let (runtime, lease, trace) =
        runtime(Mode::Retry, false, false, Duration::from_secs(100)).await;
    let mut lifecycle = lease.subscribe();
    let waiting = observed(&mut lifecycle, |status| {
        status.phase() == OperationPhase::Restarting
    })
    .await;
    assert_eq!(waiting.attempt(), 0);
    assert_eq!(waiting.outcome(), None);
    assert!(!waiting.cleanup_pending());
    tokio::time::timeout(Duration::from_secs(1), runtime.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.status().outcome(), Some(OperationOutcome::Cancelled));
    assert_eq!(lease.status().phase(), OperationPhase::Stopped);
    assert_eq!(trace.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 1);
    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn restart_uses_control() {
    let (runtime, lease, trace) =
        runtime(Mode::Retry, false, false, Duration::from_millis(1)).await;
    let mut lifecycle = lease.subscribe();
    let running = observed(&mut lifecycle, |status| {
        status.phase() == OperationPhase::Running
    })
    .await;
    assert_eq!(running.attempt(), 1);
    assert_eq!(running.outcome(), None);
    assert_eq!(trace.factories.load(Ordering::SeqCst), 1);
    assert_eq!(trace.attempts.load(Ordering::SeqCst), 2);
    assert!(matches!(
        runtime
            .operations
            .resident(&runtime.operations.resident_key(lease.id()).unwrap())
            .unwrap()
            .health,
        Some(harmos::services::ServiceStatus::Running { credit: 2 })
    ));
    // Cancellation Must Not Wait For The Pending 250 ms Health Refill.
    tokio::time::timeout(Duration::from_millis(50), runtime.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.status().outcome(), Some(OperationOutcome::Cancelled));
    assert_eq!(lease.status().attempt(), 1);
    assert_eq!(lease.status().phase(), OperationPhase::Stopped);
    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cancelled_queued_resident() {
    let trace = trace(Mode::Complete, false, false);
    let runtime = Runtime::<App>::builder((), ())
        .service_with("owned", harmos::services::POLICY, Worker(trace.clone()))
        .runtime()
        .await
        .unwrap();
    let snapshot = runtime
        .operations
        .resident(&harmos::operations::resident::Key::root("owned"))
        .unwrap();
    assert_eq!(snapshot.lifecycle.phase(), OperationPhase::Queued);
    runtime.stop().await.unwrap();
    assert_eq!(trace.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(trace.finalizers.load(Ordering::SeqCst), 1);
    assert_eq!(trace.drops.load(Ordering::SeqCst), 1);
}
