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
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
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

struct FreeGuard {
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

#[harmos_runtime::operation(resources = [Trace, harmos_runtime::operations::HostContext], response = u64, id = "scoped.parent",  policy = POLICY)]
struct ParentContractDeclaration {
    mode: Mode,
}

#[harmos_runtime::operation(resources = [Trace, harmos_runtime::operations::HostContext], response = u64, id = "scoped.bound",  policy = POLICY)]
struct BoundContractDeclaration {
    hold: bool,
}

#[harmos_runtime::operation(resources = [Trace], response = u64, id = "scoped.free",  policy = POLICY)]
struct FreeContractDeclaration {
    wait: bool,
}

#[harmos_runtime::catalog]
enum Operations {
    ParentContractDeclaration(ParentContractDeclaration),
    BoundContractDeclaration(BoundContractDeclaration),
    FreeContractDeclaration(FreeContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ParentContractDeclaration {
    type Error = Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Error> {
        let trace = ctx.resource::<Trace>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        trace.parent_constructions.fetch_add(1, Ordering::SeqCst);
        let result: ::core::result::Result<u64, Self::Error> = async move {
            match self.mode {
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
}

impl harmos_runtime::Operation for BoundContractDeclaration {
    type Error = std::convert::Infallible;

    async fn run(self, _context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let trace = _context.resource::<Trace>();
        let host = _context.resource::<harmos_runtime::operations::HostContext>();
        let _context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            if self.hold {
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
}

impl harmos_runtime::Operation for FreeContractDeclaration {
    type Error = std::convert::Infallible;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let trace = ctx.resource::<Trace>();
        let _guard = FreeGuard { trace: trace.clone() };
        let result: ::core::result::Result<u64, Self::Error> = async move {
            trace.free_started.notify_one();
            if self.wait {
                std::future::pending::<()>().await;
            }
            Ok(1)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Drop for FreeGuard {
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
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<super::Single<ParentContractDeclaration>>(parent_lane).await.unwrap();
    runtime.operations.register::<super::Single<BoundContractDeclaration>>(Lanes::Serial).await.unwrap();
    runtime.operations.register::<super::Single<FreeContractDeclaration>>(Lanes::Open).await.unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn ancestor_cycles_rejected() {
    for lane in [Lanes::Serial, Lanes::Concurrent] {
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
    let (runtime, trace) = runtime(Lanes::Serial).await;
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
    let (runtime, trace) = runtime(Lanes::Concurrent).await;
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
