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
use harmos_runtime::operations::{
    Backoff, FailureKind, Jitter, Operation, OperationInstanceId, OperationPhase, OperationPolicy, ReplaySafety, RetryPolicy,
};
use harmos_runtime::{Error, Refusal, Runtime};

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
    stopping: Notify,
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

#[harmos_runtime::message]
struct Answer {
    value: u64,
}

/* === Enums === */

#[harmos_runtime::message]
enum Mode {
    Complete,
    WaitRun,
    PanicRun,
}

#[harmos_runtime::operation(request = ReadContract, infer_request,
        response = Answer,
        id = "authored_read",

        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
            },
        },
        replay = ReplaySafety::Idempotent,
    )]
struct ReadContractDeclaration;

#[harmos_runtime::message]

struct ReadContract {
    mode: Mode,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContract(ReadContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Read {
    type Request = ReadContract;
    type Response = <ReadContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        let resources = context.dependency::<Resources>().expect("fixture registers its dependency");
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let constructed: Result<Self, Self::Error> = async move {
            resources.0.constructed.fetch_add(1, Ordering::SeqCst);
            if resources.0.fail_new.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("inert resource refusal"));
            }
            Ok(Self { trace: resources.0.clone(), phase: Cell::new(0), attempts: 0 })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Self::Error> = async {
            let operation = &mut operation;
            operation.trace.initialized.fetch_add(1, Ordering::SeqCst);
            operation.trace.ids.lock().unwrap().push(context.id());
            operation.phase.set(1);
            operation.trace.initializing.notify_one();
            if operation.trace.fail_initialize.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("partial acquisition failed"));
            }
            if operation.trace.wait_initialize.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut ReadContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Answer>, Self::Error> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<Answer, Self::Error> = async move {
            self.trace.ran.fetch_add(1, Ordering::SeqCst);
            self.trace.ids.lock().unwrap().push(context.id());
            assert_eq!(context.attempt(), self.attempts);
            self.attempts += 1;
            self.phase.set(2);
            self.trace.running.notify_one();
            match request.mode {
                Mode::WaitRun => std::future::pending().await,
                Mode::PanicRun => panic!("authored run panicked"),
                _ => Ok(Answer { value: 42 }),
            }
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.trace.stopped.fetch_add(1, Ordering::SeqCst);
        if self.trace.block_stop.load(Ordering::SeqCst) {
            self.trace.stopping.notify_one();
            self.trace.release_stop.notified().await;
        }
        if self.trace.fail_stop.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("physical stop failed"));
        }
        self.phase.set(3);
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
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

    fn classify(_: &Self::Error) -> FailureKind {
        FailureKind::Transient
    }
}

impl Drop for Read {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let trace = Arc::new(Trace::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(Resources(trace.clone()))).unwrap();
                dependencies
            })
            .implementation::<ReadContractDeclaration, Read>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    (runtime, trace)
}

fn count(value: &AtomicUsize) -> usize {
    value.load(Ordering::SeqCst)
}

/* === Tests === */

#[tokio::test]
async fn lazy_construction_lifecycle() {
    let (runtime, trace) = runtime().await;
    let mut instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
    // Construction waits for the first attempt's initialization.
    assert_eq!(count(&trace.constructed), 0);
    assert_eq!(count(&trace.initialized), 0);
    assert_eq!((&mut instance).await.unwrap().value, 42);
    assert_eq!(count(&trace.initialized), 1);
    assert_eq!(count(&trace.stopped), 1);
    assert_eq!(count(&trace.finalized), 1);
    assert_eq!(count(&trace.dropped), 1);
    assert!(trace.ids.lock().unwrap().iter().all(|id| *id == instance.id()));
    assert_eq!(instance.status().phase(), OperationPhase::Settled);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn initialization_refuses_work() {
    {
        let (runtime, trace) = runtime().await;
        trace.fail_new.store(true, Ordering::SeqCst);
        // A refused construction fails the instance before any work.
        let instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        assert!(matches!(instance.join().await, Err(Error::OperationFailed { .. })));
        runtime.stop().await.unwrap();
        assert_eq!(count(&trace.initialized), 0);
        assert_eq!(count(&trace.stopped), 0);
        assert_eq!(count(&trace.dropped), 0);
    }
    {
        let (runtime, trace) = runtime().await;
        trace.fail_initialize.store(true, Ordering::SeqCst);
        // A failed initialization owns no instance, so a failing
        // stop cannot retain one.
        trace.fail_stop.store(true, Ordering::SeqCst);
        let mut instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        assert!(matches!((&mut instance).await, Err(Error::OperationFailed { .. })));
        instance.settle().await.unwrap();
        assert_eq!(instance.status().phase(), OperationPhase::Settled);
        assert_eq!(instance.status().outcome(), Some(harmos_runtime::operations::OperationOutcome::Failed));
        // The transient failure is not retried and the partial owner
        // is dropped without cleanup.
        assert_eq!(count(&trace.initialized), 1);
        assert_eq!(count(&trace.ran), 0);
        assert_eq!(count(&trace.stopped), 0);
        assert_eq!(count(&trace.finalized), 0);
        assert_eq!(count(&trace.dropped), 1);
        runtime.stop().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn initialization_settles_partial() {
    {
        let (runtime, trace) = runtime().await;
        trace.wait_initialize.store(true, Ordering::SeqCst);
        let instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        trace.initializing.notified().await;
        assert_eq!(instance.status().phase(), OperationPhase::Initializing);
        assert!(instance.status().started_at().is_some());
        instance.cancel();
        assert!(matches!(instance.join().await, Err(Error::OperationRefused(Refusal::Cancelled))));
        assert_eq!(count(&trace.ran), 0);
        // A failed initialization drops its partial owner without cleanup.
        assert_eq!(count(&trace.stopped), 0);
        assert_eq!(count(&trace.finalized), 0);
        assert_eq!(count(&trace.dropped), 1);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, trace) = runtime().await;
        trace.wait_initialize.store(true, Ordering::SeqCst);
        let instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        trace.initializing.notified().await;
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(matches!(instance.join().await, Err(Error::OperationRefused(Refusal::TimedOut))));
        assert_eq!(count(&trace.initialized), 1);
        assert_eq!(count(&trace.ran), 0);
        assert_eq!(count(&trace.stopped), 0);
        assert_eq!(count(&trace.dropped), 1);
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn finalization_retries_owner() {
    {
        let (runtime, trace) = runtime().await;
        trace.fail_stop.store(true, Ordering::SeqCst);
        let mut instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        assert!((&mut instance).await.is_err());
        // A failed stop retains the owner with its cleanup pending.
        assert_eq!(instance.status().phase(), OperationPhase::Stopping);
        assert!(instance.status().cleanup_pending());
        assert_eq!(count(&trace.stopped), 1);
        assert_eq!(count(&trace.finalized), 0);
        assert_eq!(count(&trace.dropped), 0);
        trace.fail_stop.store(false, Ordering::SeqCst);
        trace.block_stop.store(true, Ordering::SeqCst);
        trace.block_finalize.store(true, Ordering::SeqCst);
        let mut settle = std::pin::pin!(instance.settle());
        // Settlement retries stop and finalizes only after stop returns.
        tokio::select! {
            biased;
            _ = &mut settle => panic!("settled while the retried stop blocks"),
            () = trace.stopping.notified() => {}
        }
        assert_eq!(instance.status().phase(), OperationPhase::Stopping);
        assert_eq!(count(&trace.stopped), 2);
        assert_eq!(count(&trace.finalized), 0);
        trace.release_stop.notify_one();
        tokio::select! {
            biased;
            _ = &mut settle => panic!("settled while finalization blocks"),
            () = trace.finalizing.notified() => {}
        }
        assert_eq!(count(&trace.finalized), 1);
        assert_eq!(count(&trace.dropped), 0);
        trace.release.notify_one();
        settle.await.unwrap();
        assert_eq!(count(&trace.constructed), 1);
        assert_eq!(count(&trace.stopped), 2);
        assert_eq!(count(&trace.finalized), 1);
        assert_eq!(count(&trace.dropped), 1);
        assert_eq!(instance.status().phase(), OperationPhase::Settled);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, trace) = runtime().await;
        trace.fail_finalize.store(true, Ordering::SeqCst);
        let mut instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
        assert!((&mut instance).await.is_err());
        assert_eq!(instance.status().phase(), OperationPhase::Finalizing);
        assert_eq!(count(&trace.stopped), 1);
        assert_eq!(count(&trace.dropped), 0);
        trace.fail_finalize.store(false, Ordering::SeqCst);
        // The retry repeats cleanup without replaying stop.
        instance.settle().await.unwrap();
        assert_eq!(count(&trace.stopped), 1);
        assert_eq!(count(&trace.finalized), 2);
        assert_eq!(count(&trace.dropped), 1);
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn panic_stops_finalizes() {
    let (runtime, trace) = runtime().await;
    let instance = runtime.operation(ReadContract { mode: Mode::PanicRun }).start().await.unwrap();
    assert!(matches!(instance.join().await, Err(Error::OperationPanicked { .. })));
    assert_eq!(count(&trace.initialized), 1);
    assert_eq!(count(&trace.ran), 1);
    assert_eq!(count(&trace.stopped), 1);
    assert_eq!(count(&trace.finalized), 1);
    assert_eq!(count(&trace.dropped), 1);
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn response_awaits_cleanup() {
    use std::future::Future;
    use std::task::{Context as TaskContext, Waker};
    let (runtime, trace) = runtime().await;
    trace.block_finalize.store(true, Ordering::SeqCst);
    let instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
    let id = instance.id();
    let mut response = Box::pin(harmos_runtime::grpc::response(instance));
    trace.finalizing.notified().await;
    assert!(response.as_mut().poll(&mut TaskContext::from_waker(Waker::noop())).is_pending());
    assert_eq!(count(&trace.dropped), 0);
    trace.release.notify_one();
    assert_eq!(response.await.unwrap().value, 42);
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(count(&trace.dropped), 1);
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn response_keeps_owner() {
    let (runtime, trace) = runtime().await;
    trace.fail_finalize.store(true, Ordering::SeqCst);
    let instance = runtime.operation(ReadContract { mode: Mode::Complete }).start().await.unwrap();
    let id = instance.id();
    assert!(harmos_runtime::grpc::response(instance).await.is_err());
    let status = runtime.operations.status(id).expect("failed cleanup retains its owner");
    assert!(status.cleanup_pending());
    assert_eq!(count(&trace.finalized), 1, "RPC delivery does not retry cleanup");
    assert_eq!(count(&trace.dropped), 0);
    trace.fail_finalize.store(false, Ordering::SeqCst);
    runtime.stop().await.unwrap();
    assert_eq!(count(&trace.constructed), 1);
    assert_eq!(count(&trace.finalized), 2);
    assert_eq!(count(&trace.dropped), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn response_drop_retains() {
    use std::future::Future;
    use std::task::{Context as TaskContext, Waker};
    // Dropping the response before or after its first poll keeps the
    // running instance owned until its cleanup is released.
    for polled in [false, true] {
        let (runtime, trace) = runtime().await;
        trace.block_finalize.store(true, Ordering::SeqCst);
        let instance = runtime.operation(ReadContract { mode: Mode::WaitRun }).start().await.unwrap();
        let id = instance.id();
        // Only a constructed instance has cleanup to retain.
        trace.running.notified().await;
        let mut response = Box::pin(harmos_runtime::grpc::response(instance));
        if polled {
            assert!(response.as_mut().poll(&mut TaskContext::from_waker(Waker::noop())).is_pending());
        }
        drop(response);
        trace.finalizing.notified().await;
        let status = runtime.operations.status(id).expect("same running instance is retained");
        assert!(status.cancellation_requested());
        assert_eq!(status.phase(), OperationPhase::Finalizing);
        assert_eq!(count(&trace.constructed), 1);
        assert_eq!(count(&trace.ran), 1);
        assert_eq!(count(&trace.stopped), 1);
        assert_eq!(count(&trace.dropped), 0);
        trace.release.notify_one();
        runtime.stop().await.unwrap();
        assert_eq!(count(&trace.dropped), 1);
    }

    let (runtime, trace) = runtime().await;
    trace.block_finalize.store(true, Ordering::SeqCst);
    let instance = runtime.operation(ReadContract { mode: Mode::WaitRun }).start().await.unwrap();
    let id = instance.id();
    let mut response = Box::pin(harmos_runtime::grpc::response(instance));
    trace.running.notified().await;
    assert!(response.as_mut().poll(&mut TaskContext::from_waker(Waker::noop())).is_pending());
    drop(response);
    trace.finalizing.notified().await;
    let status = runtime.operations.status(id).expect("same running instance is retained");
    assert!(status.cancellation_requested());
    assert_eq!(status.phase(), OperationPhase::Finalizing);
    assert_eq!(count(&trace.ran), 1);
    assert_eq!(count(&trace.stopped), 1);
    assert_eq!(count(&trace.dropped), 0);
    trace.release.notify_one();
    runtime.stop().await.unwrap();
    assert_eq!(count(&trace.constructed), 1);
    assert_eq!(count(&trace.dropped), 1);
}

mod listed {

    use std::sync::Arc;
    use std::time::Duration;

    use harmos_runtime::operations::{Contracts, Declarations, OperationPolicy, RetryPolicy};

    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(1),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };

    #[harmos_runtime::operation(request = DoublingDeclaration, infer_request, id = "authored_doubling",  response = u32, policy = POLICY)]
    pub struct DoublingDeclarationDeclaration;

    #[harmos_runtime::message]

    pub struct DoublingDeclaration {
        value: u32,
    }

    #[harmos_runtime::operation(request = InferredOwnedDeclaration, infer_request,
        id = "authored_inferred_owned",

        response = u32,
        policy = POLICY
    )]
    struct InferredOwnedDeclarationDeclaration;

    #[harmos_runtime::message]

    struct InferredOwnedDeclaration {
        value: u32,
    }

    #[harmos_runtime::operation(request = CachedOwnerDeclaration, infer_request,
        id = "authored_cached_owner",

        response = u32,
        policy = POLICY,
        key = owner_key
    )]
    struct CachedOwnerDeclarationDeclaration;

    #[harmos_runtime::message]

    struct CachedOwnerDeclaration {
        value: u32,
    }

    pub struct Doubling;

    impl harmos_runtime::Operation for Doubling {
        type Request = DoublingDeclaration;
        type Response = u32;
        type Error = harmos_runtime::Error;
        type Endpoints = ();
        async fn initialize(_: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn run(
            &mut self, _: harmos_runtime::OperationContext, request: &mut DoublingDeclaration, _: &mut (),
        ) -> Result<harmos_runtime::Response<u32>, harmos_runtime::Error> {
            Ok((request.value * 2).into())
        }
    }

    struct InferredOwned(Arc<u32>);

    impl harmos_runtime::Operation for InferredOwned {
        type Request = InferredOwnedDeclaration;
        type Response = u32;
        type Error = harmos_runtime::Error;
        type Endpoints = ();
        async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
            Ok(Self(context.dependency::<u32>()?))
        }
        async fn run(
            &mut self, _: harmos_runtime::OperationContext, _: &mut InferredOwnedDeclaration, _: &mut (),
        ) -> Result<harmos_runtime::operations::Response<u32>, harmos_runtime::Error> {
            Ok(harmos_runtime::operations::Response::from_owner(self.0.clone()))
        }
    }

    struct DeclaredOwned(Arc<u32>);

    impl harmos_runtime::Operation for DeclaredOwned {
        type Request = DoublingDeclaration;
        type Response = u32;
        type Error = harmos_runtime::Error;
        type Endpoints = ();
        async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
            Ok(Self(context.dependency::<u32>()?))
        }
        async fn run(
            &mut self, _: harmos_runtime::OperationContext, _: &mut DoublingDeclaration, _: &mut (),
        ) -> Result<harmos_runtime::operations::Response<u32>, harmos_runtime::Error> {
            Ok(harmos_runtime::operations::Response::from_owner(self.0.clone()))
        }
    }

    struct BorrowOwner {
        value: u32,
        borrowed: std::sync::atomic::AtomicUsize,
        ran: std::sync::atomic::AtomicUsize,
        operations: std::sync::Weak<harmos_runtime::operations::OperationManager<crate::operations::fixture::Lanes>>,
    }

    impl std::borrow::Borrow<u32> for BorrowOwner {
        fn borrow(&self) -> &u32 {
            self.borrowed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.operations.upgrade().unwrap().definition("authored_cached_owner").unwrap();
            &self.value
        }
    }

    struct CachedOwner(Arc<BorrowOwner>);

    fn owner_key(request: &CachedOwnerDeclaration) -> Option<harmos_runtime::operations::CacheKey> {
        Some(harmos_runtime::operations::CacheKey::new(request.value.to_le_bytes().to_vec()).unwrap())
    }

    impl harmos_runtime::Operation for CachedOwner {
        type Request = CachedOwnerDeclaration;
        type Response = u32;
        type Error = harmos_runtime::Error;
        type Endpoints = ();
        async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
            Ok(Self(context.dependency::<BorrowOwner>()?))
        }
        async fn run(
            &mut self, _: harmos_runtime::OperationContext, _: &mut CachedOwnerDeclaration, _: &mut (),
        ) -> Result<harmos_runtime::operations::Response<u32>, harmos_runtime::Error> {
            self.0.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(harmos_runtime::operations::Response::from_owner(self.0.clone()))
        }
    }

    #[harmos_runtime::catalog]
    pub enum Declared {
        Doubling(DoublingDeclarationDeclaration),
    }

    #[tokio::test]
    async fn restore_defers_borrow() {
        let runtime = crate::operations::fixture::runtime().await;
        let observed = Arc::new(runtime.operations.clone());
        let owner = Arc::new(BorrowOwner {
            value: 7,
            borrowed: std::sync::atomic::AtomicUsize::new(0),
            ran: std::sync::atomic::AtomicUsize::new(0),
            operations: Arc::downgrade(&observed),
        });
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(owner.clone()).unwrap();
                    dependencies
                })
                .implementation::<CachedOwnerDeclarationDeclaration, CachedOwner>(),
                crate::operations::fixture::Lanes::Open,
            ))
            .await
            .unwrap();
        let first = runtime.operation(CachedOwnerDeclaration { value: 0 }).run().await.unwrap();
        let cached = runtime.operation(CachedOwnerDeclaration { value: 0 }).run().await.unwrap();
        assert!(first.ptr_eq(&cached));
        assert_eq!(owner.ran.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
        let erased = cached.clone().into_erased();
        assert!(erased.clone().restore::<String>().is_none());
        let restored = erased.restore::<u32>().unwrap();
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(*restored, 7);
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop((first, cached, restored));
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn macro_retains_owner() {
        let runtime = crate::operations::fixture::runtime().await;
        let original = Arc::new(7_u32);
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(original.clone()).unwrap();
                    dependencies
                })
                .implementation::<InferredOwnedDeclarationDeclaration, InferredOwned>(),
                crate::operations::fixture::Lanes::Open,
            ))
            .await
            .unwrap();
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(original.clone()).unwrap();
                    dependencies
                })
                .implementation::<DoublingDeclarationDeclaration, DeclaredOwned>(),
                crate::operations::fixture::Lanes::Open,
            ))
            .await
            .unwrap();
        let inferred = runtime.operation(InferredOwnedDeclaration { value: 0 }).run().await.unwrap();
        let declared = runtime.operation(DoublingDeclaration { value: 0 }).run().await.unwrap();
        assert_eq!(*inferred, 7_u32);
        assert_eq!(*declared, 7_u32);
        assert!(std::ptr::eq(inferred.as_ref(), original.as_ref()));
        assert!(inferred.ptr_eq(&declared));
        drop((inferred, declared));
        runtime.stop().await.unwrap();
        let recovered = harmos_runtime::operations::Response::from_owner(original).try_unwrap().unwrap();
        assert_eq!(recovered, 7);
    }

    #[test]
    fn catalogs_project_declarations() {
        let batch = harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
            .implementation::<DoublingDeclarationDeclaration, Doubling>();
        batch.operations[0].validate().unwrap();
        let projected = Declared::catalog();
        assert_eq!(batch.operations[0].definition, projected[0]);
        assert_eq!(projected, Declarations::of::<Declared>().unwrap());
        assert_eq!(projected[0].id.as_str(), "authored_doubling");
        let contracts = Contracts::of::<Declared>().unwrap();
        assert_eq!(contracts[0].fingerprint(), projected[0].fingerprint);
    }
}
