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
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use crate::runtime::boot::catalog;
use crate::runtime::probe::{Editor, Sheet};
use harmos_runtime::state::{Entry, Position, Recorded};
use harmos_runtime::storage::JournalConsumer;
use harmos_runtime::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

struct HeldDrain {
    attempts: Arc<AtomicUsize>,
    release: Arc<Notify>,
}
impl JournalConsumer<Editor> for HeldDrain {
    fn resume(&self) -> Position {
        Position::ORIGIN
    }
    async fn drain(&mut self, _: Vec<Arc<Entry<Recorded<Editor>>>>, applied: Position) -> Position {
        applied
    }
    async fn flush(&mut self) {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.release.notified().await;
    }
}

#[derive(Default)]
struct Cancellation {
    started: Notify,
    finalized: AtomicUsize,
}

struct PendingGuard(Arc<Cancellation>);

#[harmos_runtime::operation(resources = [Cancellation], id = "shutdown.pending",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(3),
        attempt_timeout: Duration::from_secs(2),
        retries: harmos_runtime::operations::RetryPolicy::Never,
    },
    response = (),
)]
struct PendingContractDeclaration {}

#[harmos_runtime::catalog]
enum Operations {
    PendingContractDeclaration(PendingContractDeclaration),
}

use crate::runtime::probe::Lanes;

impl harmos_runtime::Operation for PendingContractDeclaration {
    type Error = std::convert::Infallible;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let cancellation = ctx.resource::<Cancellation>();
        let _guard = PendingGuard(cancellation.clone());
        cancellation.started.notify_one();
        std::future::pending().await
    }
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.0.finalized.fetch_add(1, Ordering::SeqCst);
    }
}

struct StopNotification {
    operations: harmos_runtime::operations::OperationManager<crate::runtime::probe::Lanes>,
    operation: harmos_runtime::operations::OperationInstanceId,
    cancelled: Arc<std::sync::OnceLock<bool>>,
}

impl std::task::Wake for StopNotification {
    fn wake(self: Arc<Self>) {
        self.cancelled
            .get_or_init(|| self.operations.status(self.operation).expect("in-flight operation remains owned").cancellation_requested());
    }
}

#[tokio::test]
async fn services_stop_first() {
    let trace = Arc::new(Cancellation::default());
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.resource::<Cancellation>(trace.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<Operations>(Lanes::Open).await.unwrap();
    let operation = runtime.operations.submit::<PendingContractDeclaration>(PendingContractDeclaration {}).unwrap();
    trace.started.notified().await;
    runtime
        .operations
        .register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "caller".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<harmos_sdk::ExecutionContext>| {
                |mut context: harmos_runtime::services::ServiceContext<Editor>| async move {
                    context.ready();
                    context.stopping().await;
                    Ok::<(), std::convert::Infallible>(())
                }
            },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("caller").await.unwrap();
    lease.ready().await.unwrap();
    let mut status = lease.subscribe();
    status.borrow_and_update();
    let cancelled = Arc::new(std::sync::OnceLock::new());
    let waker = std::task::Waker::from(Arc::new(StopNotification {
        operations: runtime.operations.clone(),
        operation: operation.id(),
        cancelled: cancelled.clone(),
    }));
    let mut changed = std::pin::pin!(status.changed());
    let mut context = std::task::Context::from_waker(&waker);
    assert!(std::future::Future::poll(changed.as_mut(), &mut context).is_pending());

    // Observe The Stop Notification Synchronously, Before Scheduling Cleanup
    runtime.shutdown().await.unwrap();
    assert_eq!(cancelled.get(), Some(&false));
    assert!(lease.status().cancellation_requested());
    assert!(matches!(operation.await, Err(Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    lease.settle().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn budget_retains_journal() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().attach(HeldDrain {
            attempts: attempts.clone(),
            release: release.clone(),
        });
    })
    .await
    .unwrap();
    assert!(matches!(
        runtime.shutdown_with_timeout(Duration::ZERO).await,
        Err(Error::InvalidInput { field: "runtime shutdown timeout", .. })
    ));
    let accepted = runtime.state.commit(crate::runtime::probe::rename("open")).await.unwrap();
    assert_eq!(accepted.position, Position(1));
    for _ in 0..2 {
        assert!(matches!(runtime.shutdown_with_timeout(Duration::from_millis(10)).await, Err(Error::ShutdownTimeout { phase: "journal" })));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
    release.notify_one();
    assert_eq!(runtime.shutdown_with_timeout(Duration::from_secs(1)).await.unwrap(), Position(1));
    assert_eq!(runtime.shutdown().await.unwrap(), Position(1));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_budget_shared() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().attach(HeldDrain {
            attempts: attempts.clone(),
            release: release.clone(),
        });
    })
    .await
    .unwrap();
    let participant = runtime.stop_participant();
    let started = tokio::time::Instant::now();
    let mut stopping = pin!(runtime.shutdown_with_timeout(Duration::from_millis(10)));
    // The first poll fixes the shared deadline while the participant still holds.
    assert!(futures_util::FutureExt::now_or_never(stopping.as_mut()).is_none(), "the stop waits for its participant");
    tokio::time::advance(Duration::from_millis(6)).await;
    drop(participant);
    assert!(matches!(stopping.await, Err(Error::ShutdownTimeout { phase: "journal" })));
    // The journal phase spends what the participant phase left of the budget.
    assert_eq!(started.elapsed(), Duration::from_millis(10));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    release.notify_one();
    runtime.shutdown().await.unwrap();
}
