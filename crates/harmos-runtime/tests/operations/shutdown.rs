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
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Trace {
    started: Notify,
    release: Notify,
    counted: AtomicUsize,
    dropped: AtomicUsize,
}

struct AttemptGuard(Arc<Trace>);

/* === Enums === */

#[harmos_runtime::message]
enum Mode {
    Hold,
    Count,
    Sleep,
}

#[harmos_runtime::operation(resources = [Trace], response = (),
        id = "shutdown.wait",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(2),
            retries: RetryPolicy::Never,
        },
    )]
struct WaitContractDeclaration {
    mode: Mode,
}

#[harmos_runtime::catalog]
enum Operations {
    WaitContractDeclaration(WaitContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for WaitContractDeclaration {
    type Error = std::convert::Infallible;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let trace = ctx.resource::<Trace>();
        let _guard = AttemptGuard(trace.clone());
        match self.mode {
            Mode::Hold => {
                trace.started.notify_one();
                trace.release.notified().await;
            }
            Mode::Count => {
                trace.counted.fetch_add(1, Ordering::SeqCst);
            }
            Mode::Sleep => {
                trace.started.notify_one();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        Ok(().into())
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<Operations>(Lanes::Serial).await.unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn cancellation_observable_queued() {
    let (runtime, trace) = runtime().await;
    let first = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    let queued = runtime.operation(WaitContractDeclaration { mode: Mode::Count }).start().await.unwrap();
    queued.cancel();
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(trace.counted.load(Ordering::SeqCst), 0);
    trace.release.notify_one();
    first.await.unwrap();

    let running = runtime.operation(WaitContractDeclaration { mode: Mode::Sleep }).start().await.unwrap();
    trace.started.notified().await;

    running.cancel();
    assert!(matches!(running.await, Err(Error::OperationRefused(Refusal::Cancelled))));

    // The cancelled queued instance was never constructed.
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_attempts() {
    let (runtime, trace) = runtime().await;
    let instance = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    runtime.shutdown().await.unwrap();
    assert!(matches!(instance.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(runtime.operations.submit_resident("late").await, Err(Error::Closing { domain: "resident" })));
}
