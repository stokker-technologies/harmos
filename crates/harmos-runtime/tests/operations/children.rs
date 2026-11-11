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
use super::app::App;
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

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
enum Exit {
    #[default]
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

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = ())]
#[derive(Clone)]
struct ParentContractDeclaration {
    exit: Exit,
}

static PARENT_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ParentContractDeclaration>(
        "children.parent",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(10),
                    jitter: Jitter::None,
                },
            },
        },
    );

const PARENT_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ParentContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ParentContractDeclaration>::new(&PARENT_CONTRACT_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = ())]
#[derive(Clone)]
struct ChildContractDeclaration {}

static CHILD_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ChildContractDeclaration>(
        "children.child",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(5),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const CHILD_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ChildContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ChildContractDeclaration>::new(&CHILD_CONTRACT_DECLARATION_DESCRIPTION);

/* === Traits === */

/* === Implementations === */

async fn parent_contract_declaration_handler(
    request: ParentContractDeclaration, context: harmos_runtime::operations::FunctionContext<ParentContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<()>, Failed> {
    let trace = context.resource::<Trace>().expect("declared resource is registered");
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<(), Failed> = async move {
        trace.attempts.fetch_add(1, Ordering::SeqCst);
        trace.events.lock().unwrap().push("parent-attempt");
        let _child = context.submit::<ChildContractDeclaration>(ChildContractDeclaration {})?;
        trace.child_started.notified().await;
        trace.parent_ready.notify_one();
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

fn parent_contract_declaration_classification(error: &Failed) -> FailureKind {
    match error {
        Failed::Retry => FailureKind::Transient,
        _ => FailureKind::Permanent,
    }
}

fn parent_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PARENT_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        parent_contract_declaration_handler,
        parent_contract_declaration_classification,
    )
}

async fn child_contract_declaration_handler(
    _request: ChildContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ChildContractDeclaration>,
) -> Result<harmos_runtime::Response<()>, std::convert::Infallible> {
    let trace = ctx.resource::<Trace>().expect("declared resource is registered");
    let _guard = ChildGuard(trace.clone());
    trace.events.lock().unwrap().push("child-started");
    trace.child_started.notify_one();
    std::future::pending().await
}

fn child_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        CHILD_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>()] },
        child_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
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
    runtime
        .operations
        .register_operation(parent_contract_declaration_operation().expect("representable operation"), crate::support::SERIAL)
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            child_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
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
            Err(Error::OperationRefused(Refusal::TimedOut { .. })) => assert!(!cancel),
            Err(Error::OperationRefused(Refusal::Cancelled { .. })) => assert!(cancel),
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
