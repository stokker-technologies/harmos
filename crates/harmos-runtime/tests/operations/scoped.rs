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
use super::{Lanes, app::App};
use harmos_runtime::operations::{Operation, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime, operations::Lane};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
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

struct Parent {
    trace: Arc<Trace>,
}

struct Bound {
    trace: Arc<Trace>,
}

struct Free {
    trace: Arc<Trace>,
}

/* === Enums === */

#[harmos_runtime::message]
enum Mode {
    Leaf,
    HandleCycle,
    PropagateCycle,
    FullLane,
}

#[harmos_runtime::operation(request = ParentContract, infer_request, response = u64, id = "scoped_parent",  policy = POLICY)]
struct ParentContractDeclaration;

#[harmos_runtime::message]

struct ParentContract {
    mode: Mode,
}

#[harmos_runtime::operation(request = BoundContract, infer_request, response = u64, id = "scoped_bound",  policy = POLICY)]
struct BoundContractDeclaration;

#[harmos_runtime::message]

struct BoundContract {
    hold: bool,
}

#[harmos_runtime::operation(request = FreeContract, infer_request, response = u64, id = "scoped_free",  policy = POLICY)]
struct FreeContractDeclaration;

#[harmos_runtime::message]

struct FreeContract {
    wait: bool,
}

#[harmos_runtime::catalog]
enum Operations {
    ParentContract(ParentContractDeclaration),
    BoundContract(BoundContractDeclaration),
    FreeContract(FreeContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Parent {
    type Request = ParentContract;
    type Response = <ParentContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Error> {
        let trace = context.dependency::<Trace>().expect("fixture registers its dependency");
        trace.parent_constructions.fetch_add(1, Ordering::SeqCst);
        Ok(Self { trace })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut ParentContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Error> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            match request.mode {
                Mode::Leaf => Ok(7),
                Mode::HandleCycle => {
                    assert!(matches!(context.submit::<ParentContractDeclaration>(ParentContract { mode: Mode::Leaf }),
                    Err(Error::OperationLaneCycle { parent, child }) if parent == self.trace.lane && child == self.trace.lane));
                    let free = context.submit::<FreeContractDeclaration>(FreeContract { wait: false })?.await?;
                    assert_eq!(*free, 1);
                    Ok(7)
                }
                Mode::PropagateCycle => {
                    let answer = context.submit::<ParentContractDeclaration>(ParentContract { mode: Mode::Leaf })?.await?;
                    Ok(*answer)
                }
                Mode::FullLane => {
                    let _queued = context.submit::<BoundContractDeclaration>(BoundContract { hold: false })?;
                    let _free = context.submit::<FreeContractDeclaration>(FreeContract { wait: true })?;
                    self.trace.free_started.notified().await;
                    Ok(u64::try_from(self.trace.queued_runs.load(Ordering::SeqCst)).expect("run counts fit in u64"))
                }
            }
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Bound {
    type Request = BoundContract;
    type Response = <BoundContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::convert::Infallible;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        let trace = context.dependency::<Trace>().expect("fixture registers its dependency");
        Ok(Self { trace })
    }

    async fn run(
        &mut self, _context: harmos_runtime::OperationContext, request: &mut BoundContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let host = _context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            if request.hold {
                self.trace.bound_started.notify_one();
                self.trace.release.notified().await;
            } else {
                self.trace.queued_runs.fetch_add(1, Ordering::SeqCst);
            }
            Ok(7)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Free {
    type Request = FreeContract;
    type Response = <FreeContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::convert::Infallible;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        let trace = context.dependency::<Trace>().expect("fixture registers its dependency");
        Ok(Self { trace })
    }

    async fn run(
        &mut self, _context: harmos_runtime::OperationContext, request: &mut FreeContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let host = _context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            self.trace.free_started.notify_one();
            if request.wait {
                std::future::pending::<()>().await;
            }
            Ok(1)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Drop for Free {
    fn drop(&mut self) {
        self.trace.free_dropped.store(true, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime(parent_lane: Lanes) -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace {
        lane: parent_lane.name(),
        parent_constructions: AtomicUsize::new(0),
        queued_runs: AtomicUsize::new(0),
        free_dropped: AtomicBool::new(false),
        bound_started: Notify::new(),
        release: Notify::new(),
        free_started: Notify::new(),
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
            parent_lane,
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
            .implementation::<BoundContractDeclaration, Bound>(),
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
            .implementation::<FreeContractDeclaration, Free>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn ancestor_cycles_rejected() {
    for lane in [Lanes::Serial, Lanes::Concurrent] {
        let (runtime, trace) = runtime(lane).await;
        let answer = runtime.operation(ParentContract { mode: Mode::HandleCycle }).start().await.unwrap().await.unwrap();
        assert_eq!(*answer, 7);
        assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
        assert!(trace.free_dropped.load(Ordering::SeqCst));
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn refusal_survives_parent() {
    let (runtime, trace) = runtime(Lanes::Serial).await;
    let error = runtime.operation(ParentContract { mode: Mode::PropagateCycle }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected parent failure") };
    assert!(
        matches!(source.downcast_ref::<Error>(), Some(Error::OperationLaneCycle { parent, child }) if *parent == "Serial" && *child == "Serial")
    );
    assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn child_settles_another() {
    let (runtime, trace) = runtime(Lanes::Concurrent).await;
    let blocker = runtime.operation(BoundContract { hold: true }).start().await.unwrap();
    trace.bound_started.notified().await;
    let answer = runtime.operation(ParentContract { mode: Mode::FullLane }).start().await.unwrap().await.unwrap();
    assert_eq!(*answer, 0);
    assert!(trace.free_dropped.load(Ordering::SeqCst));
    assert_eq!(trace.queued_runs.load(Ordering::SeqCst), 0);

    trace.release.notify_one();
    assert_eq!(*blocker.await.unwrap(), 7);
    runtime.stop().await.unwrap();
    assert_eq!(trace.queued_runs.load(Ordering::SeqCst), 0, "the cancelled child never runs after capacity returns");
}
