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
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use futures_util::StreamExt;
use tokio::sync::Semaphore;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::Runtime;
use harmos_runtime::grpc::Event;
use harmos_runtime::operations::{Handler, OperationPhase, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Request = ProduceDeclaration;

/* === Structs === */

#[harmos_runtime::operation(id = "grpc_produce",
    response = u32,
    output = u32,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
    }
)]
struct ProduceDeclaration {
    count: u32,
    fail: bool,
}

struct Trace {
    attempting: Semaphore,
    finalizing: Semaphore,
    release: Semaphore,
    fail_cleanup: AtomicBool,
    constructed: AtomicUsize,
    items_created: AtomicUsize,
    finalized: AtomicUsize,
    dropped: AtomicUsize,
}
struct Production {
    trace: Arc<Trace>,
}

struct Observe(Arc<Mutex<Vec<harmos_runtime::operations::middleware::Observation>>>);

impl harmos_runtime::operations::middleware::Middleware for Observe {
    fn enter<'a>(
        &'a self, _: &'a mut harmos_runtime::operations::middleware::Invocation,
    ) -> harmos_runtime::operations::middleware::Enter<'a> {
        Box::pin(async { Ok(()) })
    }
    fn observe(
        &self, _: &harmos_runtime::operations::middleware::Invocation, observation: harmos_runtime::operations::middleware::Observation,
    ) {
        self.0.lock().unwrap().push(observation);
    }
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Handler for Production {
    type Request = ProduceDeclaration;
    type Response = <ProduceDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type StreamEndpoints = harmos_runtime::operations::StreamOutput<u32>;
    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Self::Error> {
        let trace = context.resource::<Trace>().expect("fixture registers its dependency");
        trace.constructed.fetch_add(1, Ordering::SeqCst);
        Ok(Self { trace })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, request: &mut Request, sender: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<u32>, Self::Error> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            for value in 0..request.count {
                self.trace.attempting.add_permits(1);
                sender
                    .send_with(4, || {
                        self.trace.items_created.fetch_add(1, Ordering::SeqCst);
                        Ok(value)
                    })
                    .await
                    .map_err(|_| std::io::Error::other("output abandoned"))?;
            }
            if request.fail {
                return Err(std::io::Error::other("failed after items"));
            }
            Ok(request.count)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.trace.finalized.fetch_add(1, Ordering::SeqCst);
        self.trace.finalizing.add_permits(1);
        if self.trace.fail_cleanup.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("physical cleanup failed"));
        }
        self.trace.release.acquire().await.unwrap().forget();
        Ok(())
    }
}
impl Drop for Production {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let trace = Arc::new(Trace {
        attempting: Semaphore::new(0),
        finalizing: Semaphore::new(0),
        release: Semaphore::new(0),
        fail_cleanup: AtomicBool::new(false),
        constructed: AtomicUsize::new(0),
        items_created: AtomicUsize::new(0),
        finalized: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(trace.clone()).unwrap();
                dependencies
            })
            .implementation::<ProduceDeclaration, Production>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    (runtime, trace)
}
fn item(
    event: harmos_runtime::Result<Event<harmos_runtime::operations::StreamItem<u32>, harmos_runtime::operations::Response<u32>>>,
) -> u32 {
    match event.unwrap() {
        Event::Item(value) => *value,
        Event::Completion(_) => panic!("expected an output item"),
    }
}

fn channel(items: usize) -> (harmos_runtime::operations::StreamOutput<u32>, harmos_runtime::operations::StreamInput<u32>) {
    harmos_runtime::operations::stream(harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(items).unwrap(),
        bytes: std::num::NonZeroUsize::new(items * 4).unwrap(),
    })
    .unwrap()
}

/* === Tests === */

#[tokio::test]
async fn items_precede_settled() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(3);
    let permit = sender.reserve(4).await.unwrap();
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 2, fail: false }, sender.clone()).unwrap();
    let id = instance.id();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.finalizing.acquire().await.unwrap().forget();
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert_eq!(item(stream.next().await.unwrap()), 1);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(
            futures_core::Stream::poll_next(std::pin::Pin::new(&mut stream), cx).is_pending()
        ))
        .await
    );
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.release.add_permits(1);
    match stream.next().await.unwrap().unwrap() {
        Event::Completion(response) => assert_eq!(*response, 2),
        Event::Item(_) => panic!("expected settled response"),
    }
    assert!(sender.is_closed());
    assert!(permit.send_with(|| panic!("closed stream constructed a late item")).is_err());
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn output_releases_backpressure() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 3, fail: false }, sender).unwrap();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    for expected in 0..3 {
        assert_eq!(item(stream.next().await.unwrap()), expected);
    }
    trace.finalizing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert!(matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 3));
    assert!(stream.next().await.is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn abandonment_retains_cleanup() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 3, fail: false }, sender).unwrap();
    let id = instance.id();
    // Only a constructed instance has cleanup to retain.
    let stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire().await.unwrap().forget();
    drop(stream);
    trace.finalizing.acquire().await.unwrap().forget();
    let status = runtime.operations.status(id).unwrap();
    assert!(status.cancellation_requested());
    assert_eq!(status.phase(), OperationPhase::Finalizing);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.release.add_permits(1);
    runtime.stop().await.unwrap();
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn items_remain_visible() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(2);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 2, fail: true }, sender).unwrap();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.finalizing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert_eq!(item(stream.next().await.unwrap()), 1);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn failure_not_success() {
    let (runtime, trace) = runtime().await;
    trace.fail_cleanup.store(true, Ordering::SeqCst);
    let observations = Arc::new(Mutex::new(Vec::new()));
    runtime.operations.register_middleware(Arc::new(Observe(observations.clone()))).unwrap();
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 1, fail: false }, sender).unwrap();
    let id = instance.id();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    assert!(runtime.operations.status(id).unwrap().cleanup_pending());
    {
        let observations = observations.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert!(!observations[0].settled);
        assert_eq!(observations[0].outcome, harmos_runtime::operations::OperationOutcome::Failed);
    }
    drop(stream);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_cleanup.store(false, Ordering::SeqCst);
    trace.release.add_permits(1);
    runtime.stop().await.unwrap();
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 2);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(observations.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn deadline_settles_draining() {
    let (runtime, trace) = runtime().await;
    trace.release.add_permits(1);
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 3, fail: false }, sender).unwrap();
    let id = instance.id();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    trace.finalizing.acquire().await.unwrap().forget();
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn held_item_backpressures() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = harmos_runtime::operations::stream(harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(2).unwrap(),
        bytes: std::num::NonZeroUsize::new(4).unwrap(),
    })
    .unwrap();
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 2, fail: false }, sender).unwrap();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    let Event::Item(held) = stream.next().await.unwrap().unwrap() else { panic!("expected item") };
    assert_eq!(*held, 0);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(
            futures_core::Stream::poll_next(std::pin::Pin::new(&mut stream), cx).is_pending()
        ))
        .await
    );
    assert_eq!(trace.items_created.load(Ordering::SeqCst), 1);
    drop(held);
    assert_eq!(item(stream.next().await.unwrap()), 1);
    trace.finalizing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert!(matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 2));
    assert!(stream.next().await.is_none());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn abandonment_cancels_operation() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = harmos_runtime::operations::stream(harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(2).unwrap(),
        bytes: std::num::NonZeroUsize::new(4).unwrap(),
    })
    .unwrap();
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 2, fail: false }, sender).unwrap();
    let id = instance.id();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    let Event::Item(held) = stream.next().await.unwrap().unwrap() else { panic!("expected item") };
    drop(stream);
    trace.finalizing.acquire().await.unwrap().forget();
    assert_eq!(trace.items_created.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.release.add_permits(1);
    runtime.stop().await.unwrap();
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(*held, 0);
}
