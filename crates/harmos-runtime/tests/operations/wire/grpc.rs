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
use futures_util::StreamExt;
use tokio::sync::Semaphore;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::Runtime;
use harmos_runtime::grpc::Event;
use harmos_runtime::operations::RetryPolicy;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Request = ProduceDeclaration;

/* === Structs === */

#[harmos_runtime::operation(resources = [Trace], id = "grpc.produce",
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
    finishing: Semaphore,
    stopped: Semaphore,
    release: Semaphore,
    fail_finish: AtomicBool,
    constructed: AtomicUsize,
    items_created: AtomicUsize,
    finished: AtomicUsize,
    dropped: AtomicUsize,
}
struct AttemptGuard {
    trace: Arc<Trace>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ProduceDeclaration {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
        let trace = ctx.resource::<Trace>();
        trace.constructed.fetch_add(1, Ordering::SeqCst);
        let _guard = AttemptGuard { trace: trace.clone() };
        let sender = ctx.streams();
        for value in 0..self.count {
            trace.attempting.add_permits(1);
            sender
                .send_with(4, || {
                    trace.items_created.fetch_add(1, Ordering::SeqCst);
                    Ok(value)
                })
                .await
                .map_err(|_| std::io::Error::other("output abandoned"))?;
        }
        trace.finished.fetch_add(1, Ordering::SeqCst);
        trace.finishing.add_permits(1);
        if trace.fail_finish.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("finish failed"));
        }
        trace.release.acquire().await.unwrap().forget();
        if self.fail {
            return Err(std::io::Error::other("failed after items"));
        }
        Ok(self.count.into())
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
        self.trace.stopped.add_permits(1);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace {
        attempting: Semaphore::new(0),
        finishing: Semaphore::new(0),
        stopped: Semaphore::new(0),
        release: Semaphore::new(0),
        fail_finish: AtomicBool::new(false),
        constructed: AtomicUsize::new(0),
        items_created: AtomicUsize::new(0),
        finished: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone()).operations::<crate::operations::Single<ProduceDeclaration>>(Lanes::Serial);
    })
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
    trace.finishing.acquire().await.unwrap().forget();
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
    runtime.shutdown().await.unwrap();
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
    trace.finishing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert!(matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 3));
    assert!(stream.next().await.is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn abandonment_drops_attempt() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 3, fail: false }, sender).unwrap();
    let id = instance.id();
    let stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.attempting.acquire().await.unwrap().forget();
    drop(stream);
    runtime.shutdown().await.unwrap();
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn items_remain_visible() {
    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(2);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 2, fail: true }, sender).unwrap();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    trace.finishing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert_eq!(item(stream.next().await.unwrap()), 1);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failure_not_success() {
    let (runtime, trace) = runtime().await;
    trace.fail_finish.store(true, Ordering::SeqCst);
    let (sender, receiver) = channel(1);
    let instance = runtime.operations.submit_endpoints::<ProduceDeclaration, _>(Request { count: 1, fail: false }, sender).unwrap();
    let id = instance.id();
    let mut stream = harmos_runtime::grpc::stream(instance, receiver);
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    assert!(runtime.operations.status(id).is_none());
    drop(stream);
    assert_eq!(trace.finished.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    trace.fail_finish.store(false, Ordering::SeqCst);
    trace.release.add_permits(1);
    runtime.shutdown().await.unwrap();
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finished.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
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
    trace.stopped.acquire().await.unwrap().forget();
    assert_eq!(item(stream.next().await.unwrap()), 0);
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
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
    trace.finishing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert!(matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 2));
    assert!(stream.next().await.is_none());
    runtime.shutdown().await.unwrap();
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
    trace.stopped.acquire().await.unwrap().forget();
    assert_eq!(trace.items_created.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    trace.release.add_permits(1);
    runtime.shutdown().await.unwrap();
    assert!(runtime.operations.status(id).is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(*held, 0);
}
