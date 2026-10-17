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
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::{Error, Runtime, operations::Lane};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
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
enum Mode {
    Leaf,
    HandleCycle,
    PropagateCycle,
    FullLane,
}

#[harmos::operation(request = Mode, response = usize, id = "scoped.parent", policy = POLICY)]
struct ParentContract;

#[harmos::operation(request = bool, response = usize, id = "scoped.bound", policy = POLICY)]
struct BoundContract;

#[harmos::operation(request = bool, response = usize, id = "scoped.free", policy = POLICY)]
struct FreeContract;

#[harmos::catalog]
enum Operations {
    ParentContract(ParentContract),
    BoundContract(BoundContract),
    FreeContract(FreeContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Parent {
    type Declaration = ParentContract;
    type Resources = Trace;
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(trace: Arc<Trace>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        trace.parent_constructions.fetch_add(1, Ordering::SeqCst);
        Ok(Self { trace })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        mode: &mut Mode,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
        match mode {
            Mode::Leaf => Ok(7),
            Mode::HandleCycle => {
                assert!(matches!(context.submit_as::<ParentContract>(Mode::Leaf),
                    Err(Error::OperationLaneCycle { parent, child }) if parent == self.trace.lane && child == self.trace.lane));
                let free = context.submit_as::<FreeContract>(false)?.await?;
                assert_eq!(*free, 1);
                Ok(7)
            }
            Mode::PropagateCycle => {
                let answer = context.submit_as::<ParentContract>(Mode::Leaf)?.await?;
                Ok(*answer)
            }
            Mode::FullLane => {
                let _queued = context.submit_as::<BoundContract>(false)?;
                let _free = context.submit_as::<FreeContract>(true)?;
                self.trace.free_started.notified().await;
                Ok(self.trace.queued_runs.load(Ordering::SeqCst))
            }
        }
            }.await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Bound {
    type Declaration = BoundContract;
    type Resources = Trace;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        trace: Arc<Trace>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self { trace })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        hold: &mut bool,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
            if *hold {
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
    type Declaration = FreeContract;
    type Resources = Trace;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        trace: Arc<Trace>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self { trace })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        wait: &mut bool,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
            self.trace.free_started.notify_one();
            if *wait {
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
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Parent>("app", parent_lane, trace.clone())
        .unwrap();
    runtime
        .operations
        .register_operation::<Bound>("app", Lanes::Serial, trace.clone())
        .unwrap();
    runtime
        .operations
        .register_operation::<Free>("app", Lanes::Open, trace.clone())
        .unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn ancestor_cycles_rejected() {
    for lane in [Lanes::Serial, Lanes::Concurrent] {
        let (runtime, trace) = runtime(lane).await;
        let answer = runtime
            .operations
            .submit_as::<ParentContract>(Mode::HandleCycle)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(*answer, 7);
        assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
        assert!(trace.free_dropped.load(Ordering::SeqCst));
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn refusal_survives_parent() {
    let (runtime, trace) = runtime(Lanes::Serial).await;
    let error = runtime
        .operations
        .submit_as::<ParentContract>(Mode::PropagateCycle)
        .unwrap()
        .await
        .unwrap_err();
    let Error::OperationFailed { source, .. } = error else {
        panic!("expected parent failure")
    };
    assert!(
        matches!(source.downcast_ref::<Error>(), Some(Error::OperationLaneCycle { parent, child }) if *parent == "Serial" && *child == "Serial")
    );
    assert_eq!(trace.parent_constructions.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn child_settles_another() {
    let (runtime, trace) = runtime(Lanes::Concurrent).await;
    let blocker = runtime.operations.submit_as::<BoundContract>(true).unwrap();
    trace.bound_started.notified().await;
    let answer = runtime
        .operations
        .submit_as::<ParentContract>(Mode::FullLane)
        .unwrap()
        .await
        .unwrap();
    assert_eq!(*answer, 0);
    assert!(trace.free_dropped.load(Ordering::SeqCst));
    assert_eq!(trace.queued_runs.load(Ordering::SeqCst), 0);

    trace.release.notify_one();
    assert_eq!(*blocker.await.unwrap(), 7);
    runtime.stop().await.unwrap();
    assert_eq!(
        trace.queued_runs.load(Ordering::SeqCst),
        0,
        "the cancelled child never runs after capacity returns"
    );
}
