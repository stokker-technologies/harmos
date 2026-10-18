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
// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::Notify;
// Local Imports
use super::{Lanes, runtime};
use harmos_runtime::operations::{CacheKey, Context, Operation, OperationDeclaration, OperationOutcome, OperationPhase};
use harmos_runtime::{Error, Refusal};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Request = Finish;

/* === Structs === */
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "aa.finish",
    version = 1,
    response = u64,
    policy = super::Compute::POLICY,
    key = |request: &Finish| request.keyed.then(|| CacheKey::new([1]).unwrap()),
)]
struct Finish {
    keyed: bool,
    fail_attempt: bool,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(id = "zz.parent", version = 1, response = u64, policy = Finish::POLICY)]
struct Parent {
    keyed: bool,
    fail_attempt: bool,
}

struct State {
    ran: bool,
    counters: Arc<Counters>,
}

#[derive(Default)]
struct Counters {
    runs: AtomicUsize,
    finished: AtomicUsize,
    dropped: AtomicUsize,
    fail: AtomicBool,
    panic_cleanup: AtomicBool,
    block: AtomicBool,
    entering: Notify,
    release: Notify,
}

struct Owner(Arc<Counters>);

struct ParentOwner(Arc<Counters>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl From<Finish> for Parent {
    fn from(request: Finish) -> Self {
        Self { keyed: request.keyed, fail_attempt: request.fail_attempt }
    }
}

struct Finishing {
    resources: Arc<Owner>,
    state: State,
}

impl Operation for Finishing {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Finish;
    type Resources = Owner;
    type Error = std::io::Error;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        let state = State { ran: false, counters: Arc::clone(&resources.0) };
        Ok(Self { resources, state })
    }

    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut Request, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            self.resources.0.runs.fetch_add(1, Ordering::SeqCst);
            self.state.ran = true;
            if request.fail_attempt {
                return Err(std::io::Error::other("attempt cause"));
            }
            Ok(42)
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.0.finished.fetch_add(1, Ordering::SeqCst);
        self.resources.0.entering.notify_one();
        if self.state.ran && self.resources.0.block.load(Ordering::SeqCst) {
            self.resources.0.release.notified().await;
        }
        if self.resources.0.panic_cleanup.load(Ordering::SeqCst) {
            panic!("cleanup poll panic");
        }
        if self.resources.0.fail.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("cleanup cause"));
        }
        Ok(())
    }
}

struct ParentCall {
    _resources: Arc<ParentOwner>,
    _state: State,
}

impl Operation for ParentCall {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Parent;
    type Resources = ParentOwner;
    type Error = Error;

    async fn initialize<'ctx>(resources: Arc<ParentOwner>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        let state = State { ran: false, counters: Arc::clone(&resources.0) };
        Ok(Self { _resources: resources, _state: state })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut Parent, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Error> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let request = Request { keyed: request.keyed, fail_attempt: request.fail_attempt };
            let response = context.call(request).await?;
            Ok(*response)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.counters.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn request() -> Request {
    Request { keyed: false, fail_attempt: false }
}

/* === Tests === */

#[tokio::test]
async fn cleanup_failure_retains() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    let owner = Owner(Arc::clone(&counters));
    let owner = Arc::new(owner);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
    let failed_request = Request { fail_attempt: true, ..request() };
    let first = runtime.operations.submit(failed_request).unwrap();

    let error = first.await.unwrap_err();

    let Error::OperationFinalization { outcome: Some(outcome), source, .. } = error else {
        panic!("must retain both attempt and cleanup causes");
    };
    let outcome = std::iter::successors(Some(&*outcome as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert!(outcome.contains("attempt cause"), "{outcome}");
    let source = std::iter::successors(Some(&*source as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert!(source.contains("cleanup cause"), "{source}");
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);

    counters.fail.store(false, Ordering::SeqCst);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_refuses_failing() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    let owner = Arc::new(Owner(Arc::clone(&counters)));
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
    let invocation = runtime.operations.submit(request()).unwrap();
    assert!(invocation.await.is_err());

    // A retried cleanup that fails again refuses the unregister and keeps the state.
    assert!(runtime.operations.unregister(Finish::ID).await.is_err());
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);

    counters.fail.store(false, Ordering::SeqCst);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_awaits_cleanup() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.block.store(true, Ordering::SeqCst);
    let owner = Owner(Arc::clone(&counters));
    let owner = Arc::new(owner);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
    let invocation = runtime.operations.submit(request()).unwrap();
    counters.entering.notified().await;
    assert!(runtime.operations.unregister(Finish::ID).now_or_never().is_none(), "physical cleanup is blocked");
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.release.notify_one();
    runtime.operations.unregister(Finish::ID).await.unwrap();
    assert_eq!(*invocation.await.unwrap(), 42);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_retries_child() {
    let runtime = runtime().await;
    let child = Arc::new(Counters::default());
    child.fail.store(true, Ordering::SeqCst);
    let child_owner = Owner(Arc::clone(&child));
    let child_owner = Arc::new(child_owner);
    runtime.operations.register_operation::<Finishing>("child", Lanes::Serial, child_owner).unwrap();
    let parent = Arc::new(Counters::default());
    let parent_owner = ParentOwner(Arc::clone(&parent));
    let parent_owner = Arc::new(parent_owner);
    runtime.operations.register_operation::<ParentCall>("parent", Lanes::Open, parent_owner).unwrap();
    let invocation = runtime.operations.submit(Parent::from(request())).unwrap();
    child.entering.notified().await;
    assert!(runtime.operations.unregister(Parent::ID).await.is_err());
    assert_eq!(child.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(parent.dropped.load(Ordering::SeqCst), 0);

    child.fail.store(false, Ordering::SeqCst);
    runtime.operations.unregister(Parent::ID).await.unwrap();
    assert!(invocation.await.is_err());
    assert_eq!(child.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(parent.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_retries_cleanup() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    let owner = Owner(Arc::clone(&counters));
    let owner = Arc::new(owner);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
    let invocation = runtime.operations.submit(request()).unwrap();
    assert!(invocation.await.is_err());
    counters.fail.store(false, Ordering::SeqCst);
    counters.block.store(true, Ordering::SeqCst);
    {
        let waiting = runtime.operations.unregister(Finish::ID);
        tokio::pin!(waiting);
        tokio::select! {
            biased;
            result = &mut waiting => panic!("retry is blocked: {result:?}"),
            _ = async {
                while counters.finished.load(Ordering::SeqCst) < 2 {
                    counters.entering.notified().await;
                }
            } => {},
        }
        assert_eq!(counters.finished.load(Ordering::SeqCst), 2);
    }
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.release.notify_one();
    runtime.operations.unregister(Finish::ID).await.unwrap();
    assert_eq!(counters.finished.load(Ordering::SeqCst), 2);
    assert_eq!(counters.runs.load(Ordering::SeqCst), 1);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cleanup_panic_retained() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.panic_cleanup.store(true, Ordering::SeqCst);
    let owner = Owner(Arc::clone(&counters));
    let owner = Arc::new(owner);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Open, owner).unwrap();
    let invocation = runtime.operations.submit(request()).unwrap();
    let error = invocation.await.unwrap_err();
    assert!(matches!(error, Error::OperationFinalization { source, .. }
        if matches!(source.as_ref(), Error::OperationPanicked { .. })));
    assert!(runtime.operations.shutdown().await.is_err());
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.panic_cleanup.store(false, Ordering::SeqCst);
    runtime.operations.shutdown().await.unwrap();
    assert_eq!(counters.runs.load(Ordering::SeqCst), 1);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn finalization_past_deadline() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.block.store(true, Ordering::SeqCst);
    let owner = Owner(Arc::clone(&counters));
    let owner = Arc::new(owner);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Open, owner).unwrap();
    let first_request = Request { keyed: true, ..request() };
    let first = runtime.operations.submit(first_request).unwrap();
    counters.entering.notified().await;
    tokio::time::advance(Finish::POLICY.timeout).await;
    counters.release.notify_one();
    assert!(matches!(first.await, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);

    counters.block.store(false, Ordering::SeqCst);
    let second_request = Request { keyed: true, ..request() };
    let second = runtime.operations.submit(second_request).unwrap();
    assert_eq!(*second.await.unwrap(), 42);
    assert_eq!(counters.runs.load(Ordering::SeqCst), 2);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn boot_retains_cleanup() {
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    let owner = Arc::new(Owner(Arc::clone(&counters)));
    let mut error = harmos_runtime::Runtime::<super::support::Bank>::builder(super::support::Ledger::default(), ())
        .register(super::support::catalog())
        .register(Lanes::catalog())
        .initialize(std::time::Duration::from_secs(1), move |handle| {
            let owner = Arc::clone(&owner);
            async move {
                handle.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
                let result = handle.operations.call(request()).await;
                assert!(matches!(result, Err(Error::OperationFinalization { .. })));
                Err::<(), _>(std::io::Error::other("startup failed after invocation"))
            }
        })
        .runtime()
        .await
        .err()
        .unwrap();
    assert!(error.cleanup_error().is_some());
    assert!(error.to_string().contains("startup failed after invocation"));
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.fail.store(false, Ordering::SeqCst);
    counters.block.store(true, Ordering::SeqCst);
    let previous = counters.finished.load(Ordering::SeqCst);
    {
        let retry = error.retry_cleanup();
        tokio::pin!(retry);
        tokio::select! {
            _ = &mut retry => panic!("retained finalization is blocked"),
            _ = async {
                while counters.finished.load(Ordering::SeqCst) <= previous {
                    counters.entering.notified().await;
                }
            } => {}
        }
    }
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.block.store(false, Ordering::SeqCst);
    counters.release.notify_one();
    error.retry_cleanup().await.unwrap();
    assert!(error.cleanup_error().is_none());
    assert_eq!(counters.runs.load(Ordering::SeqCst), 1);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn receipt_retains_cleanup() {
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    let owner = Arc::new(Owner(Arc::clone(&counters)));
    let (ready, mut reached) = tokio::sync::watch::channel(false);
    let cleanup;
    {
        let opening = harmos_runtime::Runtime::<super::support::Bank>::builder(super::support::Ledger::default(), ())
            .register(super::support::catalog())
            .register(Lanes::catalog())
            .initialize(std::time::Duration::from_secs(1), move |handle| {
                let owner = Arc::clone(&owner);
                let ready = ready.clone();
                async move {
                    handle.operations.register_operation::<Finishing>("app", Lanes::Serial, owner).unwrap();
                    assert!(matches!(handle.operations.call(request()).await, Err(Error::OperationFinalization { .. })));
                    ready.send_replace(true);
                    std::future::pending::<Result<(), std::io::Error>>().await
                }
            })
            .runtime();
        cleanup = opening.cleanup();
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("startup must remain pending"),
            ready = reached.wait_for(|ready| *ready) => { ready.unwrap(); }
        }
    }
    let refusal = cleanup.wait().await.unwrap_err();
    assert!(matches!(refusal, Error::TaskFailed { domain: "bootstrap cleanup", .. }));
    assert_eq!(cleanup.status(), harmos_runtime::runtime::BootstrapStatus::Failed);
    assert!(cleanup.failure().is_some());
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);

    // Cancelling The Retry Waiter Does Not Cancel Physical Cleanup
    counters.fail.store(false, Ordering::SeqCst);
    counters.block.store(true, Ordering::SeqCst);
    let previous = counters.finished.load(Ordering::SeqCst);
    {
        let retry = cleanup.retry();
        tokio::pin!(retry);
        tokio::select! {
            outcome = &mut retry => panic!("cleanup remains blocked: {outcome:?}"),
            _ = async {
                while counters.finished.load(Ordering::SeqCst) <= previous {
                    counters.entering.notified().await;
                }
            } => {}
        }
    }
    assert_eq!(cleanup.status(), harmos_runtime::runtime::BootstrapStatus::Cleaning);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.block.store(false, Ordering::SeqCst);
    counters.release.notify_one();
    cleanup.wait().await.unwrap();
    assert_eq!(cleanup.status(), harmos_runtime::runtime::BootstrapStatus::Settled);
    assert!(cleanup.failure().is_none());
    assert_eq!(counters.runs.load(Ordering::SeqCst), 1);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unpolled_startup_cleanup() {
    let opening = harmos_runtime::Runtime::<super::support::Bank>::builder(super::support::Ledger::default(), ())
        .register(super::support::catalog())
        .runtime();
    let cleanup = opening.cleanup();
    assert_eq!(cleanup.status(), harmos_runtime::runtime::BootstrapStatus::Starting);
    drop(opening);
    cleanup.wait().await.unwrap();
    assert_eq!(cleanup.status(), harmos_runtime::runtime::BootstrapStatus::Settled);
}

#[tokio::test]
async fn outcome_snapshot_finalizing() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, Arc::new(Owner(Arc::clone(&counters)))).unwrap();
    let mut instance = runtime.operations.submit(Request { fail_attempt: true, ..request() }).unwrap();
    let id = instance.id();
    let generation = instance.generation();
    assert!(matches!((&mut instance).await, Err(Error::OperationFinalization { .. })));
    let failed = instance.status();
    assert_eq!(failed.phase(), OperationPhase::Finalizing);
    assert_eq!(failed.outcome(), Some(OperationOutcome::Failed));
    assert!(failed.cleanup_pending());
    assert!(failed.settled_at().is_none());
    assert!(matches!(failed.cleanup_failure().unwrap().as_ref(),
        Error::OperationFinalization { outcome: Some(outcome), .. }
        if std::iter::successors(Some(&**outcome as &(dyn std::error::Error + 'static)), |error| error.source()).map(ToString::to_string).collect::<Vec<_>>().join(": ").contains("attempt cause")));
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);

    counters.fail.store(false, Ordering::SeqCst);
    instance.settle().await.unwrap();
    let settled = instance.status();
    assert_eq!(instance.id(), id);
    assert_eq!(instance.generation(), generation);
    assert_eq!(settled.phase(), OperationPhase::Settled);
    assert_eq!(settled.outcome(), Some(OperationOutcome::Failed));
    assert!(!settled.cleanup_pending());
    assert!(settled.cleanup_failure().is_none());
    assert!(settled.settled_at().is_some());
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(failed.phase(), OperationPhase::Finalizing, "snapshots remain independent");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn reregister_renews_generation() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, Arc::new(Owner(Arc::clone(&counters)))).unwrap();
    let first = runtime.operations.submit(request()).unwrap();
    let second = runtime.operations.submit(request()).unwrap();
    assert_ne!(first.id(), second.id());
    assert_eq!(first.generation(), second.generation());
    let id = second.id();
    let generation = second.generation();
    assert_eq!(*first.join().await.unwrap(), 42);
    assert_eq!(*second.join().await.unwrap(), 42);

    runtime.operations.unregister(Finish::ID).await.unwrap();
    runtime.operations.register_operation::<Finishing>("app", Lanes::Serial, Arc::new(Owner(Arc::clone(&counters)))).unwrap();
    let replacement = runtime.operations.submit(request()).unwrap();

    assert_ne!(replacement.generation(), generation);
    assert_ne!(replacement.id(), id);
    assert_eq!(*replacement.join().await.unwrap(), 42);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn settle_awaits_cleanup() {
    let runtime = runtime().await;
    let counters = Arc::new(Counters::default());
    counters.fail.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Finishing>("app", Lanes::Open, Arc::new(Owner(Arc::clone(&counters)))).unwrap();
    let mut instance = runtime.operations.submit(request()).unwrap();
    assert!(matches!((&mut instance).await, Err(Error::OperationFinalization { .. })));
    assert_eq!(instance.status().outcome(), Some(OperationOutcome::Succeeded));
    counters.fail.store(false, Ordering::SeqCst);
    counters.block.store(true, Ordering::SeqCst);
    assert!(instance.settle().now_or_never().is_none());
    while counters.finished.load(Ordering::SeqCst) < 2 {
        counters.entering.notified().await;
    }
    assert_eq!(counters.finished.load(Ordering::SeqCst), 2);
    instance.cancel();
    let pending = instance.status();
    assert_eq!(pending.phase(), OperationPhase::Finalizing);
    assert_eq!(pending.outcome(), Some(OperationOutcome::Succeeded));
    assert!(pending.cancellation_requested());
    assert!(pending.cleanup_pending());
    assert!(pending.settled_at().is_none());
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 0);
    counters.release.notify_one();
    instance.settle().await.unwrap();
    assert_eq!(counters.finished.load(Ordering::SeqCst), 2);
    assert_eq!(counters.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(instance.status().phase(), OperationPhase::Settled);
    assert_eq!(instance.status().outcome(), Some(OperationOutcome::Succeeded));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn stop_retries_cleanup() {
    let runtime = runtime().await;
    let child = Arc::new(Counters::default());
    child.fail.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Finishing>("child", Lanes::Serial, Arc::new(Owner(child.clone()))).unwrap();
    let parent = Arc::new(Counters::default());
    runtime.operations.register_operation::<ParentCall>("parent", Lanes::Open, Arc::new(ParentOwner(parent.clone()))).unwrap();
    let invocation = runtime.operations.submit(Parent::from(request())).unwrap();
    child.entering.notified().await;
    assert_eq!(child.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(parent.dropped.load(Ordering::SeqCst), 0);
    child.fail.store(false, Ordering::SeqCst);
    runtime.stop().await.unwrap();
    assert!(invocation.join().await.is_err());
    assert_eq!(child.finished.load(Ordering::SeqCst), 2);
    assert_eq!(child.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(parent.dropped.load(Ordering::SeqCst), 1);
}
