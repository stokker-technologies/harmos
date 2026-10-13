/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

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
use harmos::Runtime;
use harmos::grpc::Event;
use harmos::operations::{
    Context, Operation, OperationDeclaration, OperationPhase, OperationPolicy, RetryPolicy,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Produce;
struct Request {
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

struct Observe(Arc<Mutex<Vec<harmos::operations::middleware::Observation>>>);

impl harmos::operations::middleware::Middleware for Observe {
    fn enter<'a>(
        &'a self,
        _: &'a mut harmos::operations::middleware::Invocation,
    ) -> harmos::operations::middleware::Enter<'a> {
        Box::pin(async { Ok(()) })
    }
    fn observe(
        &self,
        _: &harmos::operations::middleware::Invocation,
        observation: harmos::operations::middleware::Observation,
    ) {
        self.0.lock().unwrap().push(observation);
    }
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Produce {
    type Request = Request;
    type Response = u32;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<u32>()))
    }
    const ID: &'static str = "grpc.produce";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
    };
}
impl harmos::operations::OutputDeclaration for Produce {
    type Item = u32;
}
impl Operation for Production {
    type Declaration = Produce;
    type Resources = Trace;
    type Error = std::io::Error;
    type Endpoints = harmos::operations::StreamOutput<u32>;
    type Context<'a> = Context<'a>;
    fn new(trace: Arc<Trace>) -> Result<Self, Self::Error> {
        trace.constructed.fetch_add(1, Ordering::SeqCst);
        Ok(Self { trace })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Request,
        sender: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<u32>, Self::Error> {
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
    async fn finalize(&mut self) -> Result<(), Self::Error> {
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
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
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
        .register_operation::<Production>("grpc", Lanes::Serial, trace.clone())
        .unwrap();
    (runtime, trace)
}
fn item(
    event: harmos::Result<
        Event<harmos::operations::StreamItem<u32>, harmos::operations::Response<u32>>,
    >,
) -> u32 {
    match event.unwrap() {
        Event::Item(value) => *value,
        Event::Completion(_) => panic!("expected an output item"),
    }
}

fn channel(
    items: usize,
) -> (
    harmos::operations::StreamOutput<u32>,
    harmos::operations::StreamInput<u32>,
) {
    harmos::operations::stream(harmos::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(items).unwrap(),
        bytes: std::num::NonZeroUsize::new(items * 4).unwrap(),
    })
    .unwrap()
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("items_precede_settled", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let (sender, receiver) = channel(3);
                    let permit = sender.reserve(4).await.unwrap();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 2,
                                fail: false,
                            },
                            sender.clone(),
                        )
                        .unwrap();
                    let id = instance.id();
                    let mut stream = harmos::grpc::stream(instance, receiver);
                    trace.finalizing.acquire().await.unwrap().forget();
                    assert_eq!(item(stream.next().await.unwrap()), 0);
                    assert_eq!(item(stream.next().await.unwrap()), 1);
                    assert!(
                        std::future::poll_fn(|cx| std::task::Poll::Ready(
                            futures_core::Stream::poll_next(std::pin::Pin::new(&mut stream), cx)
                                .is_pending()
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
                    assert!(
                        permit
                            .send_with(|| panic!("closed stream constructed a late item"))
                            .is_err()
                    );
                    assert!(stream.next().await.is_none());
                    assert!(stream.next().await.is_none());
                    assert!(runtime.operations.status(id).is_none());
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("output_releases_backpressure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(1);
    let instance = runtime
        .operations
        .submit_endpoints::<Produce, _>(
            Request {
                count: 3,
                fail: false,
            },
            sender,
        )
        .unwrap();
    let mut stream = harmos::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    for expected in 0..3 {
        assert_eq!(item(stream.next().await.unwrap()), expected);
    }
    trace.finalizing.acquire().await.unwrap().forget();
    trace.release.add_permits(1);
    assert!(
        matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 3)
    );
    assert!(stream.next().await.is_none());
    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("abandonment_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let (sender, receiver) = channel(1);
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 3,
                                fail: false,
                            },
                            sender,
                        )
                        .unwrap();
                    let id = instance.id();
                    drop(harmos::grpc::stream(instance, receiver));
                    trace.finalizing.acquire().await.unwrap().forget();
                    let status = runtime.operations.status(id).unwrap();
                    assert!(status.cancellation_requested());
                    assert_eq!(status.phase(), OperationPhase::Finalizing);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.release.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("abandonment_cancels_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let (sender, receiver) = channel(1);
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 3,
                                fail: false,
                            },
                            sender,
                        )
                        .unwrap();
                    let id = instance.id();
                    let stream = harmos::grpc::stream(instance, receiver);
                    trace.attempting.acquire_many(2).await.unwrap().forget();
                    drop(stream);
                    trace.finalizing.acquire().await.unwrap().forget();
                    assert!(
                        runtime
                            .operations
                            .status(id)
                            .unwrap()
                            .cancellation_requested()
                    );
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.release.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("items_remain_visible", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let (sender, receiver) = channel(2);
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 2,
                                fail: true,
                            },
                            sender,
                        )
                        .unwrap();
                    let mut stream = harmos::grpc::stream(instance, receiver);
                    trace.finalizing.acquire().await.unwrap().forget();
                    trace.release.add_permits(1);
                    assert_eq!(item(stream.next().await.unwrap()), 0);
                    assert_eq!(item(stream.next().await.unwrap()), 1);
                    assert!(stream.next().await.unwrap().is_err());
                    assert!(stream.next().await.is_none());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_not_success", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.fail_cleanup.store(true, Ordering::SeqCst);
                    let observations = Arc::new(Mutex::new(Vec::new()));
                    runtime
                        .operations
                        .register_middleware(Arc::new(Observe(observations.clone())))
                        .unwrap();
                    let (sender, receiver) = channel(1);
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 1,
                                fail: false,
                            },
                            sender,
                        )
                        .unwrap();
                    let id = instance.id();
                    let mut stream = harmos::grpc::stream(instance, receiver);
                    assert_eq!(item(stream.next().await.unwrap()), 0);
                    assert!(stream.next().await.unwrap().is_err());
                    assert!(stream.next().await.is_none());
                    assert!(runtime.operations.status(id).unwrap().cleanup_pending());
                    {
                        let observations = observations.lock().unwrap();
                        assert_eq!(observations.len(), 1);
                        assert!(!observations[0].settled);
                        assert_eq!(
                            observations[0].outcome,
                            harmos::operations::OperationOutcome::Failed
                        );
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
                });
        });
    }
    {
        scenario_runtime::check("eof_waits_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let (runtime, trace) = runtime().await;
    let (sender, receiver) = channel(1);
    let instance = runtime
        .operations
        .submit_endpoints::<Produce, _>(
            Request {
                count: 0,
                fail: false,
            },
            sender,
        )
        .unwrap();
    let mut stream = harmos::grpc::stream(instance, receiver);
    trace.finalizing.acquire().await.unwrap().forget();
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(
            futures_core::Stream::poll_next(std::pin::Pin::new(&mut stream), cx).is_pending()
        ))
        .await
    );
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.release.add_permits(1);
    assert!(
        matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 0)
    );
    assert!(stream.next().await.is_none());
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("deadline_settles_draining", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    trace.release.add_permits(1);
                    let (sender, receiver) = channel(1);
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 3,
                                fail: false,
                            },
                            sender,
                        )
                        .unwrap();
                    let id = instance.id();
                    let mut stream = harmos::grpc::stream(instance, receiver);
                    trace.attempting.acquire_many(2).await.unwrap().forget();
                    tokio::time::advance(Duration::from_secs(6)).await;
                    trace.finalizing.acquire().await.unwrap().forget();
                    assert_eq!(item(stream.next().await.unwrap()), 0);
                    assert!(stream.next().await.unwrap().is_err());
                    assert!(stream.next().await.is_none());
                    assert!(runtime.operations.status(id).is_none());
                    assert_eq!(trace.constructed.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("item_keeps_byte", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let (runtime, trace) = runtime().await;
    let (sender, receiver) = harmos::operations::stream(harmos::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(2).unwrap(),
        bytes: std::num::NonZeroUsize::new(4).unwrap(),
    })
    .unwrap();
    let instance = runtime
        .operations
        .submit_endpoints::<Produce, _>(
            Request {
                count: 2,
                fail: false,
            },
            sender,
        )
        .unwrap();
    let mut stream = harmos::grpc::stream(instance, receiver);
    trace.attempting.acquire_many(2).await.unwrap().forget();
    let Event::Item(held) = stream.next().await.unwrap().unwrap() else {
        panic!("expected item")
    };
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
    assert!(
        matches!(stream.next().await.unwrap().unwrap(), Event::Completion(value) if *value == 2)
    );
    assert!(stream.next().await.is_none());
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("abandonment_cancels_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, trace) = runtime().await;
                    let (sender, receiver) =
                        harmos::operations::stream(harmos::operations::StreamSettings {
                            items: std::num::NonZeroUsize::new(2).unwrap(),
                            bytes: std::num::NonZeroUsize::new(4).unwrap(),
                        })
                        .unwrap();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Produce, _>(
                            Request {
                                count: 2,
                                fail: false,
                            },
                            sender,
                        )
                        .unwrap();
                    let id = instance.id();
                    let mut stream = harmos::grpc::stream(instance, receiver);
                    trace.attempting.acquire_many(2).await.unwrap().forget();
                    let Event::Item(held) = stream.next().await.unwrap().unwrap() else {
                        panic!("expected item")
                    };
                    drop(stream);
                    trace.finalizing.acquire().await.unwrap().forget();
                    assert_eq!(trace.items_created.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.release.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert!(runtime.operations.status(id).is_none());
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(*held, 0);
                });
        });
    }
}
