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
use futures_util::FutureExt;
use tokio::sync::{Notify, Semaphore};

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{Backoff, FailureKind, Handler, Jitter, OperationPolicy, RetryPolicy};
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
    finalizing: Notify,
    release: Semaphore,
    block_cleanup: bool,
}

struct Parent {
    trace: Arc<Trace>,
}

struct Child(Arc<Trace>);

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

#[harmos_runtime::operation(response = (),
        id = "children_parent",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
        },
    )]
struct ParentContractDeclaration {
    exit: Exit,
}

#[harmos_runtime::operation(response = (),
        id = "children_child",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(5),
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

impl Handler for Parent {
    type Request = ParentContractDeclaration;
    type Response = <ParentContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Failed;
    type StreamEndpoints = ();

    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Failed> {
        let trace = context.resource::<Trace>().expect("fixture registers its dependency");
        Ok(Self { trace })
    }

    async fn run(
        &mut self, context: harmos_runtime::ExecutionContext, request: &mut ParentContractDeclaration,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.trace.attempts.fetch_add(1, Ordering::SeqCst);
            self.trace.events.lock().unwrap().push("parent-attempt");
            let _child = context.submit::<ChildContractDeclaration>(ChildContractDeclaration {})?;
            self.trace.child_started.notified().await;
            self.trace.parent_ready.notify_one();
            match request.exit {
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

impl Handler for Child {
    type Request = ChildContractDeclaration;
    type Response = <ChildContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::convert::Infallible;
    type StreamEndpoints = ();

    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Self::Error> {
        let trace = context.resource::<Trace>().expect("fixture registers its dependency");
        Ok(Self(trace))
    }

    async fn run(
        &mut self, _context: harmos_runtime::ExecutionContext, _request: &mut ChildContractDeclaration,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
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
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(trace.clone()).unwrap();
                dependencies
            })
            .implementation::<ParentContractDeclaration, Parent>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(trace.clone()).unwrap();
                dependencies
            })
            .implementation::<ChildContractDeclaration, Child>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn parent_joins_child() {
    let exits = [(Exit::Return, false), (Exit::Fail, false), (Exit::Wait, false), (Exit::Wait, true)];
    for (exit, cancel) in exits {
        let (runtime, trace) = runtime(false).await;
        let parent = runtime.operation(ParentContractDeclaration { exit }).start().await.unwrap();
        if cancel {
            trace.parent_ready.notified().await;
            parent.cancel();
        }
        let outcome = parent.await;

        // The child is finalized and dropped before the parent settles.
        assert_eq!(*trace.events.lock().unwrap(), ["parent-attempt", "child-started", "child-finalized", "child-dropped"]);
        match outcome {
            Ok(_) => {}
            Err(Error::OperationFailed { source, .. }) => assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Attempt))),
            Err(Error::OperationRefused(Refusal::TimedOut)) => assert!(!cancel),
            Err(Error::OperationRefused(Refusal::Cancelled)) => assert!(cancel),
            Err(error) => panic!("unexpected parent outcome: {error}"),
        }
        runtime.stop().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn retry_waits_child() {
    let (runtime, trace) = runtime(true).await;
    let parent = runtime.operation(ParentContractDeclaration { exit: Exit::Retry }).start().await.unwrap();
    trace.finalizing.notified().await;
    tokio::time::advance(Duration::from_millis(20)).await;
    assert_eq!(trace.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(*trace.events.lock().unwrap(), ["parent-attempt", "child-started"]);

    trace.release.add_permits(1);
    trace.finalizing.notified().await;
    assert_eq!(trace.attempts.load(Ordering::SeqCst), 2);
    trace.release.add_permits(1);
    let error = parent.await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected authored failure") };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Retry)));
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
}

#[tokio::test]
async fn result_awaits_finalization() {
    for exit in [Exit::Return, Exit::Fail] {
        let succeeds = matches!(exit, Exit::Return);
        let (runtime, trace) = runtime(true).await;
        let mut parent = runtime.operation(ParentContractDeclaration { exit }).start().await.unwrap();
        trace.finalizing.notified().await;
        assert!((&mut parent).now_or_never().is_none());
        assert_eq!(*trace.events.lock().unwrap(), ["parent-attempt", "child-started"]);

        trace.release.add_permits(1);
        assert_eq!(parent.await.is_ok(), succeeds);
        assert_eq!(*trace.events.lock().unwrap(), ["parent-attempt", "child-started", "child-finalized", "child-dropped"]);
        runtime.stop().await.unwrap();
    }
}
