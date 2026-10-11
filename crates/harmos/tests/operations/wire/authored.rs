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
use std::cell::Cell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{
    Backoff, CacheKey, Context, Failure, Jitter, Operation, OperationInstanceId, OperationPhase,
    OperationPolicy, ReplaySafety, RetryPolicy,
};
use harmos::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Trace {
    constructed: AtomicUsize,
    fail_new: AtomicBool,
    fail_initialize: AtomicBool,
    wait_initialize: AtomicBool,
    initialized: AtomicUsize,
    ran: AtomicUsize,
    stopped: AtomicUsize,
    finalized: AtomicUsize,
    dropped: AtomicUsize,
    fail_stop: AtomicBool,
    fail_finalize: AtomicBool,
    block_finalize: AtomicBool,
    block_stop: AtomicBool,
    release_stop: Notify,
    initializing: Notify,
    running: Notify,
    finalizing: Notify,
    release: Notify,
    ids: Mutex<Vec<OperationInstanceId>>,
}

struct Resources(Arc<Trace>);

// Cell Deliberately Makes The Uniquely Owned Object Non-Sync.
struct Read {
    trace: Arc<Trace>,
    phase: Cell<usize>,
    attempts: usize,
}

struct Answer(usize);

/* === Enums === */
enum Mode {
    Complete,
    Cache,
    Retry,
    WaitRun,
    PanicRun,
}

#[harmos::catalog]
enum Operations {
    #[operation(
        request = Mode,
        response = Answer,
        id = "authored.read",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
            },
        },
        replay = ReplaySafety::Idempotent,
        key = cache_key,
    )]
    ReadContract,
}

/* === Traits === */

/* === Implementations === */
impl Operation for Read {
    type Declaration = ReadContract;
    type Resources = Resources;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(resources: Arc<Resources>) -> Result<Self, Self::Error> {
        resources.0.constructed.fetch_add(1, Ordering::SeqCst);
        if resources.0.fail_new.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("inert resource refusal"));
        }
        Ok(Self {
            trace: resources.0.clone(),
            phase: Cell::new(0),
            attempts: 0,
        })
    }

    async fn initialize<'ctx>(&mut self, context: Self::Context<'ctx>) -> Result<(), Self::Error> {
        self.trace.initialized.fetch_add(1, Ordering::SeqCst);
        self.trace.ids.lock().unwrap().push(context.id());
        self.phase.set(1);
        self.trace.initializing.notify_one();
        if self.trace.fail_initialize.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("partial acquisition failed"));
        }
        if self.trace.wait_initialize.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(())
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        mode: &mut Mode,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Answer, Self::Error> {
        self.trace.ran.fetch_add(1, Ordering::SeqCst);
        self.trace.ids.lock().unwrap().push(context.id());
        assert_eq!(context.attempt(), self.attempts);
        self.attempts += 1;
        self.phase.set(2);
        self.trace.running.notify_one();
        match mode {
            Mode::Retry if self.attempts == 1 => {
                Err(std::io::Error::other("transient run failure"))
            }
            Mode::WaitRun => std::future::pending().await,
            Mode::PanicRun => panic!("authored run panicked"),
            _ => Ok(Answer(42)),
        }
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.trace.stopped.fetch_add(1, Ordering::SeqCst);
        if self.trace.block_stop.load(Ordering::SeqCst) {
            self.trace.release_stop.notified().await;
        }
        if self.trace.fail_stop.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("physical stop failed"));
        }
        self.phase.set(3);
        Ok(())
    }

    async fn finalize(&mut self) -> Result<(), Self::Error> {
        assert_eq!(self.phase.get(), 3, "physical stop precedes finalization");
        self.trace.finalized.fetch_add(1, Ordering::SeqCst);
        self.trace.finalizing.notify_one();
        if self.trace.block_finalize.load(Ordering::SeqCst) {
            self.trace.release.notified().await;
        }
        if self.trace.fail_finalize.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("physical finalization failed"));
        }
        self.phase.set(4);
        Ok(())
    }

    fn classify(_: &Self::Error) -> Failure {
        Failure::Transient
    }
}

impl Drop for Read {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn cache_key(mode: &Mode) -> Option<CacheKey> {
    matches!(mode, Mode::Cache).then(|| CacheKey::new(vec![42]).unwrap())
}

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let trace = Arc::new(Trace::default());
    runtime
        .operations
        .register_operation::<Read>("app", Lanes::Serial, Arc::new(Resources(trace.clone())))
        .unwrap();
    (runtime, trace)
}

fn count(value: &AtomicUsize) -> usize {
    value.load(Ordering::SeqCst)
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("inert_construction_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let mut instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.initialized), 0);
                    assert_eq!((&mut instance).await.unwrap().0, 42);
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    assert!(
                        trace
                            .ids
                            .lock()
                            .unwrap()
                            .iter()
                            .all(|id| *id == instance.id())
                    );
                    assert_eq!(instance.status().phase(), OperationPhase::Settled);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("initialization_never_retries", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_initialize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    assert!(matches!(
                        instance.join().await,
                        Err(Error::OperationFailed { .. })
                    ));
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 0);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_interrupts_run", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.wait_initialize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    trace.initializing.notified().await;
                    assert_eq!(instance.status().phase(), OperationPhase::Initializing);
                    assert!(instance.status().started_at().is_some());
                    instance.cancel();
                    assert!(matches!(
                        instance.join().await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(count(&trace.ran), 0);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("initialization_uses_total", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.wait_initialize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    trace.initializing.notified().await;
                    tokio::time::advance(Duration::from_secs(2)).await;
                    assert!(matches!(
                        instance.join().await,
                        Err(Error::OperationRefused(Refusal::TimedOut))
                    ));
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 0);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("retries_preserve_instance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Retry)
                        .unwrap();
                    let id = instance.id();
                    assert_eq!(instance.join().await.unwrap().0, 42);
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 2);
                    assert_eq!(count(&trace.stopped), 1);
                    assert!(
                        trace
                            .ids
                            .lock()
                            .unwrap()
                            .iter()
                            .all(|observed| *observed == id)
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("finalize_retain_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    let mut instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    assert!((&mut instance).await.is_err());
                    assert_eq!(instance.status().phase(), OperationPhase::Stopping);
                    assert!(instance.status().cleanup_pending());
                    assert_eq!(count(&trace.finalized), 0);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    trace.block_finalize.store(true, Ordering::SeqCst);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(1), instance.settle())
                            .await
                            .is_err()
                    );
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.release.notify_one();
                    instance.settle().await.unwrap();
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    assert_eq!(instance.status().phase(), OperationPhase::Settled);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_never_replay", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_finalize.store(true, Ordering::SeqCst);
                    let mut instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    assert!((&mut instance).await.is_err());
                    assert_eq!(instance.status().phase(), OperationPhase::Finalizing);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.fail_finalize.store(false, Ordering::SeqCst);
                    instance.settle().await.unwrap();
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.finalized), 2);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_never_initializes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let first = runtime
                        .operations
                        .submit::<ReadContract>(Mode::WaitRun)
                        .unwrap();
                    trace.running.notified().await;
                    let queued = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    queued.cancel();
                    assert!(matches!(
                        queued.join().await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(count(&trace.constructed), 2);
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    first.cancel();
                    assert!(first.join().await.is_err());
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.dropped), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("delivery_skips_acquisition", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    assert_eq!(
                        runtime
                            .operations
                            .call::<ReadContract>(Mode::Cache)
                            .await
                            .unwrap()
                            .0,
                        42
                    );
                    assert_eq!(
                        runtime
                            .operations
                            .call::<ReadContract>(Mode::Cache)
                            .await
                            .unwrap()
                            .0,
                        42
                    );
                    assert_eq!(count(&trace.constructed), 2);
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 1);
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.finalized), 2);
                    assert_eq!(count(&trace.dropped), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("refusal_never_start", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_new.store(true, Ordering::SeqCst);
                    assert!(
                        runtime
                            .operations
                            .submit::<ReadContract>(Mode::Complete)
                            .is_err()
                    );
                    runtime.stop().await.unwrap();
                    assert_eq!(count(&trace.initialized), 0);
                    assert_eq!(count(&trace.stopped), 0);
                    assert_eq!(count(&trace.dropped), 0);
                });
        });
    }
    {
        scenario_runtime::check("panic_stops_finalizes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::PanicRun)
                        .unwrap();
                    assert!(matches!(
                        instance.join().await,
                        Err(Error::OperationPanicked { .. })
                    ));
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 1);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("recovery_preserves_stop", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    let mut instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    assert!((&mut instance).await.is_err());
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    trace.block_stop.store(true, Ordering::SeqCst);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(1), instance.settle())
                            .await
                            .is_err()
                    );
                    assert_eq!(instance.status().phase(), OperationPhase::Stopping);
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.finalized), 0);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.release_stop.notify_one();
                    instance.settle().await.unwrap();
                    assert_eq!(count(&trace.stopped), 2);
                    assert_eq!(count(&trace.finalized), 1);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_keeps_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_initialize.store(true, Ordering::SeqCst);
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    let mut instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    let error = (&mut instance).await.err().unwrap();
                    let Error::OperationFinalization {
                        outcome: Some(outcome),
                        source,
                        ..
                    } = error
                    else {
                        panic!("both logical and physical failures remain available");
                    };
                    assert!(matches!(*outcome, Error::OperationFailed { .. }));
                    assert!(matches!(*source, Error::OperationFailed { .. }));
                    assert_eq!(
                        instance.status().outcome(),
                        Some(harmos::operations::OperationOutcome::Failed)
                    );
                    assert_eq!(instance.status().phase(), OperationPhase::Stopping);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    instance.settle().await.unwrap();
                    assert_eq!(count(&trace.initialized), 1);
                    assert_eq!(count(&trace.ran), 0);
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("poll_retains_manager", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.block_finalize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::WaitRun)
                        .unwrap();
                    let id = instance.id();
                    let response = harmos::grpc::response(instance);
                    drop(response);
                    trace.finalizing.notified().await;
                    let status = runtime
                        .operations
                        .status(id)
                        .expect("original instance remains owned");
                    assert!(status.cancellation_requested());
                    assert_eq!(status.phase(), OperationPhase::Finalizing);
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.release.notify_one();
                    runtime.stop().await.unwrap();
                    assert_eq!(count(&trace.dropped), 1);
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("success_waits_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::future::Future;
                    use std::task::{Context as TaskContext, Waker};
                    let (runtime, trace) = runtime().await;
                    trace.block_finalize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    let id = instance.id();
                    let mut response = Box::pin(harmos::grpc::response(instance));
                    trace.finalizing.notified().await;
                    assert!(
                        response
                            .as_mut()
                            .poll(&mut TaskContext::from_waker(Waker::noop()))
                            .is_pending()
                    );
                    assert_eq!(count(&trace.dropped), 0);
                    trace.release.notify_one();
                    assert_eq!(response.await.unwrap().0, 42);
                    assert!(runtime.operations.status(id).is_none());
                    assert_eq!(count(&trace.dropped), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("failure_keeps_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_finalize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::Complete)
                        .unwrap();
                    let id = instance.id();
                    assert!(harmos::grpc::response(instance).await.is_err());
                    let status = runtime
                        .operations
                        .status(id)
                        .expect("failed cleanup retains its owner");
                    assert!(status.cleanup_pending());
                    assert_eq!(
                        count(&trace.finalized),
                        1,
                        "RPC delivery does not retry cleanup"
                    );
                    assert_eq!(count(&trace.dropped), 0);
                    trace.fail_finalize.store(false, Ordering::SeqCst);
                    runtime.stop().await.unwrap();
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.finalized), 2);
                    assert_eq!(count(&trace.dropped), 1);
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("run_retains_instance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::future::Future;
                    use std::task::{Context as TaskContext, Waker};
                    let (runtime, trace) = runtime().await;
                    trace.block_finalize.store(true, Ordering::SeqCst);
                    let instance = runtime
                        .operations
                        .submit::<ReadContract>(Mode::WaitRun)
                        .unwrap();
                    let id = instance.id();
                    let mut response = Box::pin(harmos::grpc::response(instance));
                    trace.running.notified().await;
                    assert!(
                        response
                            .as_mut()
                            .poll(&mut TaskContext::from_waker(Waker::noop()))
                            .is_pending()
                    );
                    drop(response);
                    trace.finalizing.notified().await;
                    let status = runtime
                        .operations
                        .status(id)
                        .expect("same running instance is retained");
                    assert!(status.cancellation_requested());
                    assert_eq!(status.phase(), OperationPhase::Finalizing);
                    assert_eq!(count(&trace.ran), 1);
                    assert_eq!(count(&trace.stopped), 1);
                    assert_eq!(count(&trace.dropped), 0);
                    trace.release.notify_one();
                    runtime.stop().await.unwrap();
                    assert_eq!(count(&trace.constructed), 1);
                    assert_eq!(count(&trace.dropped), 1);
                });
        });
    }
}

mod listed {

    use std::sync::Arc;
    use std::time::Duration;

    use harmos::operations::{
        AuthoredDeclarations, Context, Contracts, Declarations, OperationPolicy, RetryPolicy,
    };

    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(1),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };

    pub struct Doubling;

    #[harmos::operation(id = "authored.doubling", version = 1, policy = POLICY)]
    impl Doubling {
        pub fn new(_: Arc<()>) -> Self {
            Self
        }

        async fn run(&mut self, _: Context<'_>, value: &u32) -> Result<u32, harmos::Error> {
            Ok(value * 2)
        }
    }

    #[harmos::catalog]
    pub enum Listing {
        Doubling(Doubling),
    }

    #[harmos::catalog]
    pub enum Declared {
        Doubling(DoublingDeclaration),
    }

    #[test]
    fn contract_scenario() {
        {
            super::scenario_runtime::check("catalogs_project_declarations", || {
                let projected = Declarations::of::<Listing>().unwrap();
                assert_eq!(projected, Declarations::of::<Declared>().unwrap());
                assert_eq!(projected, AuthoredDeclarations::of::<Listing>().unwrap());
                assert_eq!(projected[0].id.as_str(), "authored.doubling");
                let contracts = Contracts::of::<Listing>().unwrap();
                assert_eq!(contracts, Contracts::of::<Declared>().unwrap());
                assert_eq!(contracts[0].version(), 1);
            });
        }
    }
}
