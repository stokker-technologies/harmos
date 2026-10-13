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
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{
    Backoff, CacheKey, Context, Jitter, Operation, OperationDeclaration, OperationPolicy,
    RetryPolicy, StreamInput, StreamOutput, StreamSettings, stream,
};
use harmos::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Endpoints = (StreamInput<u32>, StreamOutput<u32>);

/* === Structs === */
struct Declaration<const N: u8>;
#[derive(Default)]
struct Trace {
    new: AtomicUsize,
    initialized: AtomicUsize,
    stops: AtomicUsize,
    finalized: AtomicUsize,
    dropped: AtomicUsize,
    sending: AtomicUsize,
    fail_stop: AtomicBool,
    changed: Notify,
}
struct Forward<const N: u8> {
    trace: Arc<Trace>,
}

struct ParentDeclaration;
struct Parent<const CHARGED: bool>;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ParentDeclaration {
    type Request = Option<Endpoints>;
    type Response = u32;
    const ID: &'static str = "endpoints.parent";
    const POLICY: OperationPolicy = Declaration::<0>::POLICY;
}
impl<const CHARGED: bool> Operation for Parent<CHARGED> {
    type Declaration = ParentDeclaration;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos::Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        endpoints: &mut Option<Endpoints>,
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            let endpoints = endpoints
                .take()
                .expect("one transfer of original endpoints");
            let child = if CHARGED {
                let (requests, mut decoded) = stream::<bool>(StreamSettings {
                    items: NonZeroUsize::new(1).unwrap(),
                    bytes: NonZeroUsize::new(1).unwrap(),
                })?;
                requests.send_with(1, || Ok(true)).await?;
                context
                    .submit_charged::<Declaration<0>, _>(decoded.recv().await.unwrap(), endpoints)?
            } else {
                context.submit_endpoints::<Declaration<0>, _>(true, endpoints)?
            };
            Ok(*child.await?)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl<const N: u8> OperationDeclaration for Declaration<N> {
    type Request = bool;
    type Response = u32;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        if N == 3 {
            return (None, None);
        }
        if N == 4 {
            return (
                Some(std::any::TypeId::of::<u64>()),
                Some(std::any::TypeId::of::<u32>()),
            );
        }
        (
            Some(std::any::TypeId::of::<u32>()),
            Some(std::any::TypeId::of::<u32>()),
        )
    }
    const ID: &'static str = match N {
        1 => "endpoints.retry",
        2 => "endpoints.cache",
        _ => "endpoints.forward",
    };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(3),
        attempt_timeout: Duration::from_secs(2),
        retries: if N == 1 {
            RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            }
        } else {
            RetryPolicy::Never
        },
    };
    fn key(_: &bool) -> Option<CacheKey> {
        (N == 2).then(|| CacheKey::new(vec![1]).unwrap())
    }
}
impl<const N: u8> harmos::operations::InputDeclaration for Declaration<N> {
    type Item = u32;
}
impl<const N: u8> harmos::operations::OutputDeclaration for Declaration<N> {
    type Item = u32;
}
impl<const N: u8> Operation for Forward<N> {
    type Declaration = Declaration<N>;
    type Resources = Trace;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = Endpoints;
    async fn initialize<'ctx>(
        trace: Arc<Trace>,
        _context: Self::Context<'ctx>,
    ) -> harmos::Result<Self> {
        let constructed: harmos::Result<Self> = async move {
            trace.new.fetch_add(1, Ordering::SeqCst);
            Ok(Self { trace })
        }
        .await;
        let mut operation = constructed?;
        let started: harmos::Result<()> = async {
            let operation = &mut operation;
            operation.trace.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        consume: &mut bool,
        endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            if !*consume {
                return Ok(0);
            }
            let (input, output) = endpoints;
            let mut total = 0;
            while let Some(item) = input.recv().await {
                self.trace.sending.fetch_add(1, Ordering::SeqCst);
                self.trace.changed.notify_one();
                output.send_with(4, || Ok(*item + 1)).await?;
                total += *item;
            }
            Ok(total)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> harmos::Result<()> {
        self.trace.stops.fetch_add(1, Ordering::SeqCst);
        if self.trace.fail_stop.load(Ordering::SeqCst) {
            return Err(Error::InvalidInput {
                field: "fixture stop",
                reason: "retained",
            });
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> harmos::Result<()> {
        self.trace.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl<const N: u8> Drop for Forward<N> {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn channel() -> (StreamOutput<u32>, StreamInput<u32>) {
    stream(StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(4).unwrap(),
    })
    .unwrap()
}
async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .unwrap()
}
async fn runtime() -> Runtime<App> {
    Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap()
}

#[cfg(feature = "grpc")]
async fn envelope(
    value: harmos::grpc::InputEnvelope<bool, u32>,
) -> harmos::operations::StreamItem<harmos::grpc::InputEnvelope<bool, u32>> {
    let (output, mut input) = stream(StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(64).unwrap(),
    })
    .unwrap();
    output.send_with(64, || Ok(value)).await.unwrap();
    input.recv().await.unwrap()
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("endpoints_share_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, mut output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    bounded(input.send_with(4, || Ok(7))).await.unwrap();
                    drop(input);
                    let item = bounded(output.recv()).await.unwrap();
                    assert_eq!(*item, 8);
                    assert_eq!(*bounded(instance).await.unwrap(), 7);
                    assert!(bounded(output.recv()).await.is_none());
                    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(*item, 8);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_settles_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, mut output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    bounded(input.send_with(4, || Ok(1))).await.unwrap();
                    let held = bounded(output.recv()).await.unwrap();
                    bounded(input.send_with(4, || Ok(2))).await.unwrap();
                    bounded(async {
                        while trace.sending.load(Ordering::SeqCst) < 2 {
                            trace.changed.notified().await;
                        }
                    })
                    .await;
                    instance.cancel();
                    assert!(bounded(instance).await.is_err());
                    assert!(input.is_closed());
                    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(*held, 2);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cleanup_retains_endpoints", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, _output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(false, (receiving, sending))
                        .unwrap();
                    assert!(matches!(
                        bounded(instance).await,
                        Err(Error::OperationFinalization { .. })
                    ));
                    assert!(!input.is_closed());
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    bounded(runtime.stop()).await.unwrap();
                    assert!(input.is_closed());
                    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.stops.load(Ordering::SeqCst), 2);
                    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("stream_retry_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<1>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Forward<2>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, _output) = channel();
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_endpoints::<Declaration<2>, _>(false, (receiving, sending)),
                        Err(Error::InvalidInput {
                            field: "operation stream cache",
                            ..
                        })
                    ));
                    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
                    assert!(input.is_closed());
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_settles_child", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    parent_cancellation::<false>().await;
                    parent_cancellation::<true>().await;
                });
        });
    }
    {
        scenario_runtime::check("registration_refuses_mismatched", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    assert!(matches!(
                        runtime.operations.register_operation::<Forward<3>>(
                            "app",
                            Lanes::Open,
                            trace.clone()
                        ),
                        Err(Error::InvalidInput {
                            field: "operation endpoint types",
                            ..
                        })
                    ));
                    assert!(matches!(
                        runtime.operations.register_operation::<Forward<4>>(
                            "app",
                            Lanes::Open,
                            trace.clone()
                        ),
                        Err(Error::InvalidInput {
                            field: "operation endpoint types",
                            ..
                        })
                    ));
                    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("half_close_joins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (producer, mut decoded) = channel();
                    producer.send_with(4, || Ok(7)).await.unwrap();
                    let item = decoded.recv().await.unwrap();
                    let (input, receiving) = channel();
                    let (sending, mut output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    let incoming = futures_util::stream::iter([Ok(item)]);
                    let result = bounded(harmos::grpc::input(instance, input, incoming))
                        .await
                        .unwrap();
                    assert_eq!(*result, 7);
                    let held = output.recv().await.unwrap();
                    assert_eq!(*held, 8);
                    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("settlement_never_wait", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, _output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(false, (receiving, sending))
                        .unwrap();
                    let result = bounded(harmos::grpc::input(
                        instance,
                        input,
                        futures_util::stream::pending(),
                    ))
                    .await
                    .unwrap();
                    assert_eq!(*result, 0);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("failure_preserves_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, _output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    // Only a constructed instance has cleanup to preserve.
                    let initialized = trace.clone();
                    let incoming = futures_util::stream::once(async move {
                        while initialized.initialized.load(Ordering::SeqCst) == 0 {
                            tokio::task::yield_now().await;
                        }
                        Err(tonic::Status::data_loss("broken input"))
                    });
                    let error = bounded(harmos::grpc::input(instance, input, incoming))
                        .await
                        .unwrap_err();
                    let harmos::grpc::InputError::Forward { source, settlement } = error else {
                        panic!("input failure missing")
                    };
                    assert!(matches!(
                        *source,
                        Error::TaskFailed {
                            domain: "gRPC input",
                            ..
                        }
                    ));
                    assert!(matches!(
                        settlement.as_deref(),
                        Some(Error::OperationFinalization { .. })
                    ));
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    bounded(runtime.stop()).await.unwrap();
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("input_drop_cancels", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, _output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    drop(harmos::grpc::input(
                        instance,
                        input,
                        futures_util::stream::pending(),
                    ));
                    bounded(runtime.stop()).await.unwrap();
                    // Abandoned before it started, the instance never constructed.
                    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("deadline_settles_peer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime
        .operations
        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
        .unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime
        .operations
        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
        .unwrap();
    let error = bounded(harmos::grpc::input(
        instance,
        input,
        futures_util::stream::pending(),
    ))
    .await
    .unwrap_err();
    assert!(
        matches!(error, harmos::grpc::InputError::Operation(source) if matches!(*source, Error::OperationRefused(harmos::Refusal::TimedOut)))
    );
    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();

});
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("grpc_exchange_polls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use futures_util::StreamExt;
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (producer, mut decoded) = stream::<u32>(StreamSettings {
                        items: NonZeroUsize::new(3).unwrap(),
                        bytes: NonZeroUsize::new(12).unwrap(),
                    })
                    .unwrap();
                    let mut items = Vec::new();
                    for value in 0..3 {
                        producer.send_with(4, || Ok(value)).await.unwrap();
                        items.push(Ok(decoded.recv().await.unwrap()));
                    }
                    let (input, receiving) = channel();
                    let (sending, output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    let mut exchange = harmos::grpc::exchange(
                        instance,
                        input,
                        futures_util::stream::iter(items),
                        output,
                    );
                    for expected in 1..=3 {
                        let event = bounded(exchange.next()).await.unwrap().unwrap();
                        assert!(
                            matches!(event, harmos::grpc::Event::Item(item) if *item == expected)
                        );
                    }
                    let event = bounded(exchange.next()).await.unwrap().unwrap();
                    assert!(
                        matches!(event, harmos::grpc::Event::Completion(response) if *response == 3)
                    );
                    assert!(bounded(exchange.next()).await.is_none());
                    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("failure_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use futures_util::StreamExt;
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    trace.fail_stop.store(true, Ordering::SeqCst);
    runtime
        .operations
        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
        .unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime
        .operations
        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
        .unwrap();
    // Only a constructed instance has cleanup to retain.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        while initialized.initialized.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        Err(tonic::Status::data_loss("broken input"))
    });
    let mut exchange = harmos::grpc::exchange(instance, input, incoming, output);
    let error = match bounded(exchange.next()).await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("expected forwarding failure"),
    };
    assert!(
        matches!(error, harmos::grpc::InputError::Forward { settlement: Some(error), .. }
        if matches!(*error, Error::OperationFinalization { .. }))
    );
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_stop.store(false, Ordering::SeqCst);
    bounded(runtime.stop()).await.unwrap();
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);

});
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("exchange_drop_cancels", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (input, receiving) = channel();
                    let (sending, output) = channel();
                    let instance = runtime
                        .operations
                        .submit_endpoints::<Declaration<0>, _>(true, (receiving, sending))
                        .unwrap();
                    drop(harmos::grpc::exchange(
                        instance,
                        input,
                        futures_util::stream::pending(),
                        output,
                    ));
                    bounded(runtime.stop()).await.unwrap();
                    // Abandoned before it started, the instance never constructed.
                    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                });
        });
    }
    {
        scenario_runtime::check("request_survives_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    trace.fail_stop.store(true, Ordering::SeqCst);
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (requests, mut decoded) = stream::<bool>(StreamSettings {
                        items: NonZeroUsize::new(1).unwrap(),
                        bytes: NonZeroUsize::new(1).unwrap(),
                    })
                    .unwrap();
                    requests.send_with(1, || Ok(false)).await.unwrap();
                    let request = decoded.recv().await.unwrap();
                    let (_input, receiving) = channel();
                    let (sending, _output) = channel();
                    let instance = runtime
                        .operations
                        .submit_charged::<Declaration<0>, _>(request, (receiving, sending))
                        .unwrap();
                    assert!(matches!(
                        bounded(instance).await,
                        Err(Error::OperationFinalization { .. })
                    ));
                    let mut next = Box::pin(requests.reserve(1));
                    assert!(
                        std::future::poll_fn(|cx| std::task::Poll::Ready(
                            next.as_mut().poll(cx).is_pending()
                        ))
                        .await
                    );
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
                    trace.fail_stop.store(false, Ordering::SeqCst);
                    bounded(runtime.stop()).await.unwrap();
                    drop(bounded(next).await.unwrap());
                    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("request_releases_memory", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let trace = Arc::new(Trace::default());
                    runtime
                        .operations
                        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
                        .unwrap();
                    let (requests, mut decoded) = stream::<bool>(StreamSettings {
                        items: NonZeroUsize::new(1).unwrap(),
                        bytes: NonZeroUsize::new(1).unwrap(),
                    })
                    .unwrap();
                    requests.send_with(1, || Ok(false)).await.unwrap();
                    let request = decoded.recv().await.unwrap();
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_charged::<Declaration<0>, _>(request, ()),
                        Err(Error::InvalidInput { .. })
                    ));
                    drop(bounded(requests.reserve(1)).await.unwrap());
                    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
                    bounded(runtime.stop()).await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("opening_projects_charged", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use futures_util::StreamExt;
    use harmos::grpc::InputEnvelope;
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime
        .operations
        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
        .unwrap();
    let frames = futures_util::stream::iter([
        Ok(envelope(InputEnvelope::Request(true)).await),
        Ok(envelope(InputEnvelope::Item(7)).await),
    ]);
    let (request, incoming) = bounded(harmos::grpc::open(frames)).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime
        .operations
        .submit_charged::<Declaration<0>, _>(request, (receiving, sending))
        .unwrap();
    let mut exchange = harmos::grpc::exchange(instance, input, incoming, output);
    assert!(
        matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos::grpc::Event::Item(item) if *item == 8)
    );
    assert!(
        matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos::grpc::Event::Completion(value) if *value == 7)
    );
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();

});
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("opening_refuses_bad", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use futures_util::StreamExt;
    use harmos::grpc::InputEnvelope;
    let missing = harmos::grpc::open::<bool, u32, _>(futures_util::stream::empty()).await;
    assert!(matches!(missing, Err(error) if error.code() == tonic::Code::InvalidArgument));
    let early = futures_util::stream::iter([Ok(envelope(InputEnvelope::Item(1)).await)]);
    assert!(
        matches!(harmos::grpc::open(early).await, Err(error) if error.code() == tonic::Code::InvalidArgument)
    );
    let frames = futures_util::stream::iter([
        Ok(envelope(InputEnvelope::Request(true)).await),
        Ok(envelope(InputEnvelope::Request(false)).await),
        Ok(envelope(InputEnvelope::Item(7)).await),
    ]);
    let (_request, items) = harmos::grpc::open(frames).await.unwrap();
    tokio::pin!(items);
    assert!(
        matches!(items.next().await.unwrap(), Err(error) if error.code() == tonic::Code::InvalidArgument)
    );
    assert!(items.next().await.is_none());

});
        });
    }
}

async fn parent_cancellation<const CHARGED: bool>() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime
        .operations
        .register_operation::<Forward<0>>("app", Lanes::Open, trace.clone())
        .unwrap();
    runtime
        .operations
        .register_operation::<Parent<CHARGED>>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    let (input, receiving) = channel();
    let (sending, mut output) = channel();
    let parent = runtime
        .operations
        .submit_as::<ParentDeclaration>(Some((receiving, sending)))
        .unwrap();
    bounded(input.send_with(4, || Ok(7))).await.unwrap();
    let held = bounded(output.recv()).await.unwrap();
    parent.cancel();
    assert!(bounded(parent).await.is_err());
    assert!(input.is_closed());
    assert!(bounded(output.recv()).await.is_none());
    assert_eq!(*held, 8);
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}
