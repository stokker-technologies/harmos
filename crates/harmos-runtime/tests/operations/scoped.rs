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
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::app::App;
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Trace {
    lane: &'static str,
    parent_constructions: AtomicUsize,
    queued_runs: AtomicUsize,
    free_dropped: AtomicBool,
    bound_started: Notify,
    release: Notify,
    free_started: Notify,
}

struct FreeGuard {
    trace: Arc<Trace>,
}

/* === Enums === */

#[derive(harmos_runtime::Message, Clone, Debug)]
enum Mode {
    Leaf,
    HandleCycle,
    PropagateCycle,
    FullLane,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Leaf
    }
}

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct ParentContractDeclaration {
    mode: Mode,
}

static PARENT_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ParentContractDeclaration>("scoped.parent", POLICY);

const PARENT_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ParentContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ParentContractDeclaration>::new(&PARENT_CONTRACT_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct BoundContractDeclaration {
    hold: bool,
}

static BOUND_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<BoundContractDeclaration>("scoped.bound", POLICY);

const BOUND_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<BoundContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<BoundContractDeclaration>::new(&BOUND_CONTRACT_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct FreeContractDeclaration {
    wait: bool,
}

static FREE_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<FreeContractDeclaration>("scoped.free", POLICY);

const FREE_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<FreeContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<FreeContractDeclaration>::new(&FREE_CONTRACT_DECLARATION_DESCRIPTION);

/* === Traits === */

/* === Implementations === */

async fn parent_contract_declaration_handler(
    request: ParentContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ParentContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<u64>, Error> {
    let trace = ctx.resource::<Trace>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    trace.parent_constructions.fetch_add(1, Ordering::SeqCst);
    let result: ::core::result::Result<u64, Error> = async move {
        match request.mode {
            Mode::Leaf => Ok(7),
            Mode::HandleCycle => {
                assert!(matches!(context.submit::<ParentContractDeclaration>(ParentContractDeclaration { mode: Mode::Leaf }),
                    Err(Error::OperationLaneCycle { parent, child }) if parent == trace.lane && child == trace.lane));
                let free = context.submit::<FreeContractDeclaration>(FreeContractDeclaration { wait: false })?.await?;
                assert_eq!(*free, 1);
                Ok(7)
            }
            Mode::PropagateCycle => {
                let answer = context.submit::<ParentContractDeclaration>(ParentContractDeclaration { mode: Mode::Leaf })?.await?;
                Ok(*answer)
            }
            Mode::FullLane => {
                let _queued = context.submit::<BoundContractDeclaration>(BoundContractDeclaration { hold: false })?;
                let _free = context.submit::<FreeContractDeclaration>(FreeContractDeclaration { wait: true })?;
                trace.free_started.notified().await;
                Ok(u64::try_from(trace.queued_runs.load(Ordering::SeqCst)).expect("run counts fit in u64"))
            }
        }
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn parent_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PARENT_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        parent_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn bound_contract_declaration_handler(
    request: BoundContractDeclaration, _context: harmos_runtime::operations::FunctionContext<BoundContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<u64>, std::convert::Infallible> {
    let trace = _context.resource::<Trace>().expect("declared resource is registered");
    let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let _context = host.borrow();
    let result: ::core::result::Result<u64, std::convert::Infallible> = async move {
        if request.hold {
            trace.bound_started.notify_one();
            trace.release.notified().await;
        } else {
            trace.queued_runs.fetch_add(1, Ordering::SeqCst);
        }
        Ok(7)
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn bound_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        BOUND_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        bound_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn free_contract_declaration_handler(
    request: FreeContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<FreeContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<u64>, std::convert::Infallible> {
    let trace = ctx.resource::<Trace>().expect("declared resource is registered");
    let _guard = FreeGuard { trace: trace.clone() };
    let result: ::core::result::Result<u64, std::convert::Infallible> = async move {
        trace.free_started.notify_one();
        if request.wait {
            std::future::pending::<()>().await;
        }
        Ok(1)
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn free_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        FREE_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>()] },
        free_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Drop for FreeGuard {
    fn drop(&mut self) {
        self.trace.free_dropped.store(true, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime(parent_lane: harmos_runtime::operations::Lane) -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace {
        lane: parent_lane.name(),
        parent_constructions: AtomicUsize::new(0),
        queued_runs: AtomicUsize::new(0),
        free_dropped: AtomicBool::new(false),
        bound_started: Notify::new(),
        release: Notify::new(),
        free_started: Notify::new(),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(parent_contract_declaration_operation().expect("representable operation"), parent_lane)
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(bound_contract_declaration_operation().expect("representable operation"), crate::support::SERIAL)
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            free_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn ancestor_cycles_rejected() {
    for lane in [crate::support::SERIAL, crate::support::CONCURRENT] {
        let (runtime, trace) = runtime(lane).await;
        let answer = runtime.operation(ParentContractDeclaration { mode: Mode::HandleCycle }).start().await.unwrap().await.unwrap();
        assert_eq!(*answer, 7);
        assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
        assert!(trace.free_dropped.load(Ordering::SeqCst));
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn refusal_survives_parent() {
    let (runtime, trace) = runtime(crate::support::SERIAL).await;
    let error = runtime.operation(ParentContractDeclaration { mode: Mode::PropagateCycle }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected parent failure") };
    assert!(
        matches!(source.downcast_ref::<Error>(), Some(Error::OperationLaneCycle { parent, child }) if *parent == "Serial" && *child == "Serial")
    );
    assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn child_settles_another() {
    let (runtime, trace) = runtime(crate::support::CONCURRENT).await;
    let blocker = runtime.operation(BoundContractDeclaration { hold: true }).start().await.unwrap();
    trace.bound_started.notified().await;
    let answer = runtime.operation(ParentContractDeclaration { mode: Mode::FullLane }).start().await.unwrap().await.unwrap();
    assert_eq!(*answer, 0);
    assert!(trace.free_dropped.load(Ordering::SeqCst));
    assert_eq!(trace.queued_runs.load(Ordering::SeqCst), 0);

    trace.release.notify_one();
    assert_eq!(*blocker.await.unwrap(), 7);
    runtime.shutdown().await.unwrap();
    assert_eq!(trace.queued_runs.load(Ordering::SeqCst), 0, "the cancelled child never runs after capacity returns");
}
