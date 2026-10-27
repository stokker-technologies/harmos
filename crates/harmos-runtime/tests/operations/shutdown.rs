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
use futures_util::FutureExt;
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{Handler, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Trace {
    started: Notify,
    release: Notify,
    finalizing: Notify,
    counted: AtomicUsize,
    finalizations: AtomicUsize,
    dropped: AtomicUsize,
}

struct Wait {
    holding: bool,
    released: bool,
    trace: Arc<Trace>,
}

/* === Enums === */

#[harmos_runtime::message]
enum Mode {
    Hold,
    Count,
    Sleep,
}

#[harmos_runtime::operation(response = (),
        id = "shutdown_wait",
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

impl Handler for Wait {
    type Request = WaitContractDeclaration;
    type Response = <WaitContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::convert::Infallible;
    type StreamEndpoints = ();

    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Self::Error> {
        let trace = context.resource::<Trace>().expect("fixture registers its dependency");
        Ok(Self { holding: false, released: false, trace })
    }

    async fn run(
        &mut self, _context: harmos_runtime::ExecutionContext, request: &mut WaitContractDeclaration,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.holding = matches!(request.mode, Mode::Hold);
            match request.mode {
                Mode::Hold => {
                    self.trace.started.notify_one();
                    self.trace.release.notified().await;
                    self.released = true;
                }
                Mode::Count => {
                    self.trace.counted.fetch_add(1, Ordering::SeqCst);
                }
                Mode::Sleep => {
                    self.trace.started.notify_one();
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.trace.finalizations.fetch_add(1, Ordering::SeqCst);
        self.trace.finalizing.notify_one();
        if self.holding && !self.released {
            self.trace.release.notified().await;
            self.released = true;
        }
        Ok(())
    }
}

impl Drop for Wait {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace::default());
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(trace.clone()).unwrap();
                dependencies
            })
            .implementation::<WaitContractDeclaration, Wait>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_retains_operation() {
    let (runtime, trace) = runtime().await;
    let instance = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    {
        let stop = runtime.stop();
        tokio::pin!(stop);
        tokio::select! {
            _ = &mut stop => panic!("the original finalizer is still waiting"),
            _ = trace.finalizing.notified() => {},
        }
    }
    for _ in 0..2 {
        assert!(runtime.stop().now_or_never().is_none());
        assert!(matches!(runtime.operations.submit_resident("late").await, Err(Error::Closing { domain: "resident" })));
        assert_eq!(trace.finalizations.load(Ordering::SeqCst), 1);
        assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    }
    trace.release.notify_one();
    runtime.stop().await.unwrap();
    assert!(matches!(instance.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(trace.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}
