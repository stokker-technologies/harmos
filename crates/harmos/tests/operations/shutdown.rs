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
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::{Error, Refusal, Runtime};

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
enum Mode {
    Hold,
    Count,
    Sleep,
}

#[harmos::operation(
        request = Mode,
        response = (),
        id = "shutdown.wait",
        policy = OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(2),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct WaitContract;

#[harmos::catalog]
enum Operations {
    WaitContract(WaitContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Wait {
    type Declaration = WaitContract;
    type Resources = Trace;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        trace: Arc<Trace>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            holding: false,
            released: false,
            trace,
        })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        mode: &mut Mode,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.holding = matches!(mode, Mode::Hold);
            match mode {
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
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Wait>("app", Lanes::Serial, trace.clone())
        .unwrap();
    (runtime, trace)
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("cancellation_observable_queued", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let first = runtime
                        .operations
                        .submit_as::<WaitContract>(Mode::Hold)
                        .unwrap();
                    trace.started.notified().await;
                    let queued = runtime
                        .operations
                        .submit_as::<WaitContract>(Mode::Count)
                        .unwrap();
                    queued.cancel();
                    assert!(matches!(
                        queued.await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(trace.counted.load(Ordering::SeqCst), 0);
                    trace.release.notify_one();
                    first.await.unwrap();

                    let running = runtime
                        .operations
                        .submit_as::<WaitContract>(Mode::Sleep)
                        .unwrap();
                    trace.started.notified().await;

                    running.cancel();
                    assert!(matches!(
                        running.await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));

                    // The cancelled queued instance was never constructed.
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_retains_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let instance = runtime
                        .operations
                        .submit_as::<WaitContract>(Mode::Hold)
                        .unwrap();
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
                        assert!(matches!(
                            runtime.operations.submit_resident("late").await,
                            Err(Error::Closing { domain: "resident" })
                        ));
                        assert_eq!(trace.finalizations.load(Ordering::SeqCst), 1);
                        assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    }
                    trace.release.notify_one();
                    runtime.stop().await.unwrap();
                    assert!(matches!(
                        instance.await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(trace.finalizations.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
}
