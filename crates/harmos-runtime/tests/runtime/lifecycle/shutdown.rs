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
use crate::boot::catalog;
use crate::probe::{Editor, Sheet};
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

struct PendingCall(Arc<Cancellation>);

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "shutdown.pending",
    version = 1,
    policy = harmos_runtime::operations::OperationPolicy {
        timeout: Duration::from_secs(3),
        attempt_timeout: Duration::from_secs(2),
        retries: harmos_runtime::operations::RetryPolicy::Never,
        restart: harmos_runtime::operations::RestartPolicy::Never,
    },
    response = (),
)]
struct PendingContract {}

#[harmos_runtime::catalog]
enum Operations {
    PendingContract(PendingContract),
}

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

impl harmos_runtime::operations::Operation for PendingCall {
    type Declaration = PendingContract;
    type Resources = Cancellation;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = harmos_runtime::operations::Context<'a>;

    async fn initialize<'ctx>(trace: Arc<Cancellation>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        Ok(Self(trace))
    }

    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, _: &mut PendingContract, _: &mut (),
    ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.0.started.notify_one();
            std::future::pending().await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct StopNotification {
    operations: harmos_runtime::operations::OperationManager,
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
    let runtime = Runtime::<Editor>::builder(Sheet::default(), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let trace = Arc::new(Cancellation::default());
    runtime.operations.register_operation::<PendingCall>("shutdown", Lanes::Open, trace.clone()).unwrap();
    let operation = runtime.operations.submit(PendingContract {}).unwrap();
    trace.started.notified().await;
    runtime
        .operations
        .register_service(harmos_runtime::services::ServiceDefinition { id: "caller".into(), policy: harmos_runtime::services::POLICY }, |_: Arc<()>| {
            |mut context: harmos_runtime::services::Service<Editor>| async move {
                context.ready();
                context.stopping().await;
                Ok::<(), harmos_runtime::services::Fault<std::convert::Infallible>>(())
            }
        })
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
    runtime.stop().await.unwrap();
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
    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
        .register(catalog())
        .attach(HeldDrain { attempts: attempts.clone(), release: release.clone() })
        .runtime()
        .await
        .unwrap();
    assert!(matches!(runtime.stop_with_timeout(Duration::ZERO).await, Err(Error::InvalidInput { field: "runtime shutdown timeout", .. })));
    let accepted = runtime.state.commit(crate::probe::rename("open")).await.unwrap();
    assert_eq!(accepted.position, Position(1));
    for _ in 0..2 {
        assert!(matches!(runtime.stop_with_timeout(Duration::from_millis(10)).await, Err(Error::ShutdownTimeout { phase: "journal" })));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
    release.notify_one();
    assert_eq!(runtime.stop_with_timeout(Duration::from_secs(1)).await.unwrap(), Position(1));
    assert_eq!(runtime.stop().await.unwrap(), Position(1));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_budget_shared() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
        .attach(HeldDrain { attempts: attempts.clone(), release: release.clone() })
        .runtime()
        .await
        .unwrap();
    let participant = runtime.stop_participant();
    let started = tokio::time::Instant::now();
    let mut stopping = pin!(runtime.stop_with_timeout(Duration::from_millis(10)));
    // The first poll fixes the shared deadline while the participant still holds.
    assert!(futures_util::FutureExt::now_or_never(stopping.as_mut()).is_none(), "the stop waits for its participant");
    tokio::time::advance(Duration::from_millis(6)).await;
    drop(participant);
    assert!(matches!(stopping.await, Err(Error::ShutdownTimeout { phase: "journal" })));
    // The journal phase spends what the participant phase left of the budget.
    assert_eq!(started.elapsed(), Duration::from_millis(10));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    release.notify_one();
    runtime.stop().await.unwrap();
}
