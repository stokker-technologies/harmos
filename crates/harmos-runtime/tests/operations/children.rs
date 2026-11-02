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
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Trace {
    attempts: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
    child_started: Notify,
    parent_ready: Notify,
}

struct ChildGuard(Arc<Trace>);

/* === Enums === */

#[harmos_runtime::message]
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

#[harmos_runtime::operation(resources = [Trace, harmos_runtime::operations::HostContext], response = (),
        id = "children.parent",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
        },
    )]
struct ParentContractDeclaration {
    exit: Exit,
}

#[harmos_runtime::operation(resources = [Trace], response = (),
        id = "children.child",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(5),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    )]
struct ChildContractDeclaration {}

#[harmos_runtime::catalog]
enum Operations {
    ParentContractDeclaration(ParentContractDeclaration),
    ChildContractDeclaration(ChildContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ParentContractDeclaration {
    type Error = Failed;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Failed> {
        let trace = context.resource::<Trace>();
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            trace.attempts.fetch_add(1, Ordering::SeqCst);
            trace.events.lock().unwrap().push("parent-attempt");
            let _child = context.submit::<ChildContractDeclaration>(ChildContractDeclaration {})?;
            trace.child_started.notified().await;
            trace.parent_ready.notify_one();
            match self.exit {
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

impl harmos_runtime::Operation for ChildContractDeclaration {
    type Error = std::convert::Infallible;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let trace = ctx.resource::<Trace>();
        let _guard = ChildGuard(trace.clone());
        trace.events.lock().unwrap().push("child-started");
        trace.child_started.notify_one();
        std::future::pending().await
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push("child-dropped");
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace {
        attempts: AtomicUsize::new(0),
        events: Mutex::new(Vec::new()),
        child_started: Notify::new(),
        parent_ready: Notify::new(),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<super::Single<ParentContractDeclaration>>(Lanes::Serial).await.unwrap();
    runtime.operations.register::<super::Single<ChildContractDeclaration>>(Lanes::Open).await.unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn parent_joins_child() {
    let exits = [(Exit::Return, false), (Exit::Fail, false), (Exit::Wait, false), (Exit::Wait, true)];
    for (exit, cancel) in exits {
        let (runtime, trace) = runtime().await;
        let parent = runtime.operation(ParentContractDeclaration { exit }).start().await.unwrap();
        if cancel {
            trace.parent_ready.notified().await;
            parent.request_cancel();
        }
        let outcome = parent.await;

        // The Child Future Is Dropped Before The Parent Settles.
        assert_eq!(*trace.events.lock().unwrap(), ["parent-attempt", "child-started", "child-dropped"]);
        match outcome {
            Ok(_) => {}
            Err(Error::OperationFailed { source, .. }) => assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Attempt))),
            Err(Error::OperationRefused(Refusal::TimedOut)) => assert!(!cancel),
            Err(Error::OperationRefused(Refusal::Cancelled)) => assert!(cancel),
            Err(error) => panic!("unexpected parent outcome: {error}"),
        }
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn retry_waits_child() {
    let (runtime, trace) = runtime().await;
    let parent = runtime.operation(ParentContractDeclaration { exit: Exit::Retry }).start().await.unwrap();
    let error = parent.await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected authored failure") };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Retry)));
    assert_eq!(
        *trace.events.lock().unwrap(),
        ["parent-attempt", "child-started", "child-dropped", "parent-attempt", "child-started", "child-dropped",]
    );
    runtime.shutdown().await.unwrap();
}
