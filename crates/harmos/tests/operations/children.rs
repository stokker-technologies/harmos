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
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::{Notify, Semaphore};

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{
    Backoff, Context, FailureKind, Jitter, Operation, OperationPolicy, RestartPolicy, RetryPolicy,
};
use harmos::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Trace {
    attempts: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
    child_started: Notify,
    parent_ready: Notify,
    finalizing: Notify,
    release: Semaphore,
    block_cleanup: bool,
}

struct Parent {
    trace: Arc<Trace>,
}

struct Child(Arc<Trace>);

/* === Enums === */
enum Exit {
    Return,
    Fail,
    Wait,
    Retry,
}

#[derive(Debug, thiserror::Error)]
enum Failed {
    #[error("the attempt failed")]
    Attempt,
    #[error("retry the attempt")]
    Retry,
    #[error(transparent)]
    Child(#[from] Error),
}

#[harmos::operation(
        request = Exit,
        response = (),
        id = "children.parent",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
            restart: RestartPolicy::Never,
        },
    )]
struct ParentContract;

#[harmos::operation(
        request = (),
        response = (),
        id = "children.child",
        policy = OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(5),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct ChildContract;

#[harmos::catalog]
enum Operations {
    ParentContract(ParentContract),
    ChildContract(ChildContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Parent {
    type Declaration = ParentContract;
    type Resources = Trace;
    type Error = Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(trace: Arc<Trace>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        Ok(Self { trace })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        exit: &mut Exit,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Failed> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.trace.attempts.fetch_add(1, Ordering::SeqCst);
            self.trace.events.lock().unwrap().push("parent-attempt");
            let _child = context.submit_as::<ChildContract>(())?;
            self.trace.child_started.notified().await;
            self.trace.parent_ready.notify_one();
            match exit {
                Exit::Return => Ok(()),
                Exit::Fail => Err(Failed::Attempt),
                Exit::Wait => std::future::pending().await,
                Exit::Retry => Err(Failed::Retry),
            }
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Failed) -> FailureKind {
        match error {
            Failed::Retry => FailureKind::Transient,
            _ => FailureKind::Permanent,
        }
    }
}

impl Operation for Child {
    type Declaration = ChildContract;
    type Resources = Trace;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        trace: Arc<Trace>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(trace))
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.0.events.lock().unwrap().push("child-started");
            self.0.child_started.notify_one();
            std::future::pending().await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.finalizing.notify_one();
        if self.0.block_cleanup {
            self.0.release.acquire().await.unwrap().forget();
        }
        self.0.events.lock().unwrap().push("child-finalized");
        Ok(())
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push("child-dropped");
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime(block_cleanup: bool) -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace {
        attempts: AtomicUsize::new(0),
        events: Mutex::new(Vec::new()),
        child_started: Notify::new(),
        parent_ready: Notify::new(),
        finalizing: Notify::new(),
        release: Semaphore::new(0),
        block_cleanup,
    });
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Parent>("app", Lanes::Serial, trace.clone())
        .unwrap();
    runtime
        .operations
        .register_operation::<Child>("app", Lanes::Open, trace.clone())
        .unwrap();
    (runtime, trace)
}

async fn torn_down(exit: Exit, cancel: bool) -> Result<harmos::operations::Response<()>, Error> {
    let (runtime, trace) = runtime(false).await;
    let parent = runtime
        .operations
        .submit_as::<ParentContract>(exit)
        .unwrap();
    if cancel {
        trace.parent_ready.notified().await;
        parent.cancel();
    }
    let outcome = parent.await;
    assert_eq!(
        *trace.events.lock().unwrap(),
        [
            "parent-attempt",
            "child-started",
            "child-finalized",
            "child-dropped"
        ]
    );
    runtime.stop().await.unwrap();
    outcome
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("returning_parent_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    torn_down(Exit::Return, false).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failed_parent_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let error = torn_down(Exit::Fail, false).await.unwrap_err();
                    let Error::OperationFailed { source, .. } = error else {
                        panic!("expected authored failure")
                    };
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Attempt)
                    ));
                });
        });
    }
    {
        scenario_runtime::check("expired_parent_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert!(matches!(
                        torn_down(Exit::Wait, false).await,
                        Err(Error::OperationRefused(Refusal::TimedOut))
                    ));
                });
        });
    }
    {
        scenario_runtime::check("parent_settles_child", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert!(matches!(
                        torn_down(Exit::Wait, true).await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                });
        });
    }
    {
        scenario_runtime::check("retry_waits_child", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime(true).await;
                    let parent = runtime
                        .operations
                        .submit_as::<ParentContract>(Exit::Retry)
                        .unwrap();
                    trace.finalizing.notified().await;
                    tokio::time::advance(Duration::from_millis(20)).await;
                    assert_eq!(trace.attempts.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        *trace.events.lock().unwrap(),
                        ["parent-attempt", "child-started"]
                    );

                    trace.release.add_permits(1);
                    trace.finalizing.notified().await;
                    assert_eq!(trace.attempts.load(Ordering::SeqCst), 2);
                    trace.release.add_permits(1);
                    let error = parent.await.unwrap_err();
                    let Error::OperationFailed { source, .. } = error else {
                        panic!("expected authored failure")
                    };
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Retry)
                    ));
                    assert_eq!(
                        *trace.events.lock().unwrap(),
                        [
                            "parent-attempt",
                            "child-started",
                            "child-finalized",
                            "child-dropped",
                            "parent-attempt",
                            "child-started",
                            "child-finalized",
                            "child-dropped",
                        ]
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("result_remains_during", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for exit in [Exit::Return, Exit::Fail] {
                        let succeeds = matches!(exit, Exit::Return);
                        let (runtime, trace) = runtime(true).await;
                        let mut parent = runtime
                            .operations
                            .submit_as::<ParentContract>(exit)
                            .unwrap();
                        trace.finalizing.notified().await;
                        assert!((&mut parent).now_or_never().is_none());
                        assert_eq!(
                            *trace.events.lock().unwrap(),
                            ["parent-attempt", "child-started"]
                        );

                        trace.release.add_permits(1);
                        assert_eq!(parent.await.is_ok(), succeeds);
                        assert_eq!(
                            *trace.events.lock().unwrap(),
                            [
                                "parent-attempt",
                                "child-started",
                                "child-finalized",
                                "child-dropped"
                            ]
                        );
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
}
