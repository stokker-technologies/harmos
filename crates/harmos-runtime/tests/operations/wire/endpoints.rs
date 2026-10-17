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
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{
    Backoff, CacheKey, Context, Jitter, NoStream, Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Stream,
    StreamInput, StreamOutput, StreamSettings, stream,
};
use harmos_runtime::{Error, Runtime};

/* === Constants === */
const DECLARATION_NAME: &str = concat!(module_path!(), "::Declaration");
const DECLARATION_MARKER: &[u8] = &harmos_runtime::schema::declaration::encode::<
    { harmos_runtime::schema::declaration::encoded_len(DECLARATION_NAME, <Consume as harmos_runtime::schema::Described>::FIELDS) },
>(DECLARATION_NAME, <Consume as harmos_runtime::schema::Described>::FIELDS);

/* === Macros === */

/* === Type Aliases === */
type Endpoints = (StreamInput<u32>, StreamOutput<u32>);

/* === Structs === */
// One Forwarding Declaration Per Shape: The Request Says Whether To Consume Input.
struct Declaration<const N: u8> {
    consume: bool,
}
// The Portable Shape Every Forwarding Request Encodes As.
#[harmos_runtime::message]
struct Consume {
    consume: bool,
}
// The Stream Directions Each Forwarding Shape Declares.
struct Kind<const N: u8>;
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

// The Parent Request Carries Original Endpoints, So It Stays In Process.
struct ParentDeclaration {
    endpoints: Mutex<Option<Endpoints>>,
}
struct Parent<const CHARGED: bool>;

/* === Enums === */

/* === Traits === */
trait Shape {
    type Input: harmos_runtime::operations::Direction;
    type Output: harmos_runtime::operations::Direction;
}

/* === Implementations === */
crate::in_process!(ParentDeclaration);

impl harmos_runtime::MemorySize for ParentDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl OperationDeclaration for ParentDeclaration {
    type Response = u32;
    type Input = NoStream;
    type Output = NoStream;
    const ID: &'static str = "endpoints.parent";
    const VERSION: u32 = 1;
    const POLICY: OperationPolicy = Declaration::<0>::POLICY;
}
impl<const CHARGED: bool> Operation for Parent<CHARGED> {
    type Declaration = ParentDeclaration;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut ParentDeclaration, _: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            let endpoints = request.endpoints.get_mut().unwrap().take().expect("one transfer of original endpoints");
            let child = if CHARGED {
                let (requests, mut decoded) = stream::<Declaration<0>>(StreamSettings {
                    items: NonZeroUsize::new(1).unwrap(),
                    bytes: NonZeroUsize::new(1).unwrap(),
                })?;
                requests.send_with(1, || Ok(Declaration { consume: true })).await?;
                context.submit_charged::<Declaration<0>, _>(decoded.recv().await.unwrap(), endpoints)?
            } else {
                context.submit_endpoints(Declaration::<0> { consume: true }, endpoints)?
            };
            Ok(*child.await?)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Shape for Kind<0> {
    type Input = Stream<u32>;
    type Output = Stream<u32>;
}
impl Shape for Kind<1> {
    type Input = Stream<u32>;
    type Output = Stream<u32>;
}
impl Shape for Kind<2> {
    type Input = Stream<u32>;
    type Output = Stream<u32>;
}
impl Shape for Kind<3> {
    type Input = NoStream;
    type Output = NoStream;
}
impl Shape for Kind<4> {
    type Input = Stream<u64>;
    type Output = Stream<u32>;
}

impl<const N: u8> harmos_runtime::Message for Declaration<N> {
    fn encode_to_vec(&self) -> Vec<u8> {
        harmos_runtime::Message::encode_to_vec(&Consume { consume: self.consume })
    }
    fn encoded_len(&self) -> usize {
        harmos_runtime::Message::encoded_len(&Consume { consume: self.consume })
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <Consume as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Consume as harmos_runtime::Message>::decode(bytes).map(|value| Self { consume: value.consume })
    }
}

#[cfg(feature = "grpc")]
impl<const N: u8> harmos_runtime::Protobuf for Declaration<N> {
    fn encode_to_vec(&self) -> Vec<u8> {
        harmos_runtime::Protobuf::encode_to_vec(&Consume { consume: self.consume })
    }
    fn encoded_len(&self) -> usize {
        harmos_runtime::Protobuf::encoded_len(&Consume { consume: self.consume })
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <Consume as harmos_runtime::Protobuf>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Consume as harmos_runtime::Protobuf>::decode(bytes).map(|value| Self { consume: value.consume })
    }
}

impl<const N: u8> harmos_runtime::MemorySize for Declaration<N> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl<const N: u8> harmos_runtime::schema::Described for Declaration<N> {
    const NAME: &'static str = DECLARATION_NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <Consume as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = DECLARATION_MARKER;
}

impl<const N: u8> OperationDeclaration for Declaration<N>
where
    Kind<N>: Shape,
{
    type Response = u32;
    type Input = <Kind<N> as Shape>::Input;
    type Output = <Kind<N> as Shape>::Output;
    const VERSION: u32 = 1;
    const ID: &'static str = match N {
        1 => "endpoints.retry",
        2 => "endpoints.cache",
        _ => "endpoints.forward",
    };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(3),
        attempt_timeout: Duration::from_secs(2),
        retries: if N == 1 {
            RetryPolicy::OnTransient { max_retries: 1, backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None } }
        } else {
            RetryPolicy::Never
        },
        restart: RestartPolicy::Never,
    };
    fn key(&self) -> Option<CacheKey> {
        (N == 2).then(|| CacheKey::new(vec![1]).unwrap())
    }
}
impl<const N: u8> Operation for Forward<N>
where
    Kind<N>: Shape,
{
    type Declaration = Declaration<N>;
    type Resources = Trace;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = Endpoints;
    async fn initialize<'ctx>(trace: Arc<Trace>, _context: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        let constructed: harmos_runtime::Result<Self> = async move {
            trace.new.fetch_add(1, Ordering::SeqCst);
            Ok(Self { trace })
        }
        .await;
        let mut operation = constructed?;
        let started: harmos_runtime::Result<()> = async {
            let operation = &mut operation;
            operation.trace.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut Declaration<N>, endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            if !request.consume {
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
    async fn stop(&mut self) -> harmos_runtime::Result<()> {
        self.trace.stops.fetch_add(1, Ordering::SeqCst);
        if self.trace.fail_stop.load(Ordering::SeqCst) {
            return Err(Error::InvalidInput { field: "fixture stop", reason: "retained" });
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> harmos_runtime::Result<()> {
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
    stream(StreamSettings { items: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(4).unwrap() }).unwrap()
}
async fn bounded<T>(future: impl Future<Output = T>) -> T {
    (future).await
}
async fn runtime() -> Runtime<App> {
    Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap()
}

#[cfg(feature = "grpc")]
async fn envelope(
    value: harmos_runtime::grpc::InputEnvelope<Declaration<0>, u32>,
) -> harmos_runtime::operations::StreamItem<harmos_runtime::grpc::InputEnvelope<Declaration<0>, u32>> {
    let (output, mut input) =
        stream(StreamSettings { items: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(64).unwrap() }).unwrap();
    output.send_with(64, || Ok(value)).await.unwrap();
    input.recv().await.unwrap()
}

/* === Tests === */

#[tokio::test]
async fn endpoints_share_operation() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, mut output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
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
}

#[tokio::test]
async fn cancellation_settles_owner() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, mut output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
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
}

#[tokio::test]
async fn cleanup_retains_endpoints() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    trace.fail_stop.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: false }, (receiving, sending)).unwrap();
    assert!(matches!(bounded(instance).await, Err(Error::OperationFinalization { .. })));
    assert!(!input.is_closed());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_stop.store(false, Ordering::SeqCst);
    bounded(runtime.stop()).await.unwrap();
    assert!(input.is_closed());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.stops.load(Ordering::SeqCst), 2);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stream_retry_admission() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<1>>("app", Lanes::Open, trace.clone()).unwrap();
    runtime.operations.register_operation::<Forward<2>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    assert!(matches!(
        runtime.operations.submit_endpoints(Declaration::<2> { consume: false }, (receiving, sending)),
        Err(Error::InvalidInput { field: "operation stream cache", .. })
    ));
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    assert!(input.is_closed());
    bounded(runtime.stop()).await.unwrap();
}

#[tokio::test]
async fn cancellation_settles_child() {
    parent_cancellation::<false>().await;
    parent_cancellation::<true>().await;
}

#[tokio::test]
async fn registration_refuses_mismatched() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    assert!(matches!(
        runtime.operations.register_operation::<Forward<3>>("app", Lanes::Open, trace.clone()),
        Err(Error::InvalidInput { field: "operation endpoint types", .. })
    ));
    assert!(matches!(
        runtime.operations.register_operation::<Forward<4>>("app", Lanes::Open, trace.clone()),
        Err(Error::InvalidInput { field: "operation endpoint types", .. })
    ));
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn half_close_joins() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (producer, mut decoded) = channel();
    producer.send_with(4, || Ok(7)).await.unwrap();
    let item = decoded.recv().await.unwrap();
    let (input, receiving) = channel();
    let (sending, mut output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    let incoming = futures_util::stream::iter([Ok(item)]);
    let result = bounded(harmos_runtime::grpc::input(instance, input, incoming)).await.unwrap();
    assert_eq!(*result, 7);
    let held = output.recv().await.unwrap();
    assert_eq!(*held, 8);
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn settlement_never_wait() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: false }, (receiving, sending)).unwrap();
    let result = bounded(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending())).await.unwrap();
    assert_eq!(*result, 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn failure_preserves_cleanup() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    trace.fail_stop.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to preserve.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        while initialized.initialized.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        Err(tonic::Status::data_loss("broken input"))
    });
    let error = bounded(harmos_runtime::grpc::input(instance, input, incoming)).await.unwrap_err();
    let harmos_runtime::grpc::InputError::Forward { source, settlement } = error else { panic!("input failure missing") };
    assert!(matches!(*source, Error::TaskFailed { domain: "gRPC input", .. }));
    assert!(matches!(settlement.as_deref(), Some(Error::OperationFinalization { .. })));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_stop.store(false, Ordering::SeqCst);
    bounded(runtime.stop()).await.unwrap();
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn input_drop_cancels() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    drop(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending()));
    bounded(runtime.stop()).await.unwrap();
    // Abandoned before it started, the instance never constructed.
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "grpc")]
#[tokio::test(start_paused = true)]
async fn deadline_settles_peer() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    let error = bounded(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending())).await.unwrap_err();
    assert!(
        matches!(error, harmos_runtime::grpc::InputError::Operation(source) if matches!(*source, Error::OperationRefused(harmos_runtime::Refusal::TimedOut)))
    );
    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_exchange_polls() {
    use futures_util::StreamExt;
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (producer, mut decoded) =
        stream::<u32>(StreamSettings { items: NonZeroUsize::new(3).unwrap(), bytes: NonZeroUsize::new(12).unwrap() }).unwrap();
    let mut items = Vec::new();
    for value in 0..3 {
        producer.send_with(4, || Ok(value)).await.unwrap();
        items.push(Ok(decoded.recv().await.unwrap()));
    }
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, futures_util::stream::iter(items), output);
    for expected in 1..=3 {
        let event = bounded(exchange.next()).await.unwrap().unwrap();
        assert!(matches!(event, harmos_runtime::grpc::Event::Item(item) if *item == expected));
    }
    let event = bounded(exchange.next()).await.unwrap().unwrap();
    assert!(matches!(event, harmos_runtime::grpc::Event::Completion(response) if *response == 3));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.stops.load(Ordering::SeqCst), 1);
    assert_eq!(trace.finalized.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn failure_retains_cleanup() {
    use futures_util::StreamExt;
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    trace.fail_stop.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to retain.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        while initialized.initialized.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        Err(tonic::Status::data_loss("broken input"))
    });
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, incoming, output);
    let error = match bounded(exchange.next()).await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("expected forwarding failure"),
    };
    assert!(matches!(error, harmos_runtime::grpc::InputError::Forward { settlement: Some(error), .. }
        if matches!(*error, Error::OperationFinalization { .. })));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_stop.store(false, Ordering::SeqCst);
    bounded(runtime.stop()).await.unwrap();
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn exchange_drop_cancels() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime.operations.submit_endpoints(Declaration::<0> { consume: true }, (receiving, sending)).unwrap();
    drop(harmos_runtime::grpc::exchange(instance, input, futures_util::stream::pending(), output));
    bounded(runtime.stop()).await.unwrap();
    // Abandoned before it started, the instance never constructed.
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn request_survives_cleanup() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    trace.fail_stop.store(true, Ordering::SeqCst);
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (requests, mut decoded) =
        stream::<Declaration<0>>(StreamSettings { items: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(1).unwrap() }).unwrap();
    requests.send_with(1, || Ok(Declaration { consume: false })).await.unwrap();
    let request = decoded.recv().await.unwrap();
    let (_input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_charged::<Declaration<0>, _>(request, (receiving, sending)).unwrap();
    assert!(matches!(bounded(instance).await, Err(Error::OperationFinalization { .. })));
    let mut next = Box::pin(requests.reserve(1));
    assert!(std::future::poll_fn(|cx| std::task::Poll::Ready(next.as_mut().poll(cx).is_pending())).await);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
    trace.fail_stop.store(false, Ordering::SeqCst);
    bounded(runtime.stop()).await.unwrap();
    drop(bounded(next).await.unwrap());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn request_releases_memory() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let (requests, mut decoded) =
        stream::<Declaration<0>>(StreamSettings { items: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(1).unwrap() }).unwrap();
    requests.send_with(1, || Ok(Declaration { consume: false })).await.unwrap();
    let request = decoded.recv().await.unwrap();
    assert!(matches!(runtime.operations.submit_charged::<Declaration<0>, _>(request, ()), Err(Error::InvalidInput { .. })));
    drop(bounded(requests.reserve(1)).await.unwrap());
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn opening_projects_charged() {
    use futures_util::StreamExt;
    use harmos_runtime::grpc::InputEnvelope;
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    let frames = futures_util::stream::iter([
        Ok(envelope(InputEnvelope::Request(Declaration { consume: true })).await),
        Ok(envelope(InputEnvelope::Item(7)).await),
    ]);
    let (request, incoming) = bounded(harmos_runtime::grpc::open(frames)).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime.operations.submit_charged::<Declaration<0>, _>(request, (receiving, sending)).unwrap();
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, incoming, output);
    assert!(matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos_runtime::grpc::Event::Item(item) if *item == 8));
    assert!(matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos_runtime::grpc::Event::Completion(value) if *value == 7));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.stop()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn opening_refuses_bad() {
    use futures_util::StreamExt;
    use harmos_runtime::grpc::InputEnvelope;
    let missing = harmos_runtime::grpc::open::<bool, u32, _>(futures_util::stream::empty()).await;
    assert!(matches!(missing, Err(error) if error.code() == tonic::Code::InvalidArgument));
    let early = futures_util::stream::iter([Ok(envelope(InputEnvelope::Item(1)).await)]);
    assert!(matches!(harmos_runtime::grpc::open(early).await, Err(error) if error.code() == tonic::Code::InvalidArgument));
    let frames = futures_util::stream::iter([
        Ok(envelope(InputEnvelope::Request(Declaration { consume: true })).await),
        Ok(envelope(InputEnvelope::Request(Declaration { consume: false })).await),
        Ok(envelope(InputEnvelope::Item(7)).await),
    ]);
    let (_request, items) = harmos_runtime::grpc::open(frames).await.unwrap();
    tokio::pin!(items);
    assert!(matches!(items.next().await.unwrap(), Err(error) if error.code() == tonic::Code::InvalidArgument));
    assert!(items.next().await.is_none());
}

async fn parent_cancellation<const CHARGED: bool>() {
    let runtime = runtime().await;
    let trace = Arc::new(Trace::default());
    runtime.operations.register_operation::<Forward<0>>("app", Lanes::Open, trace.clone()).unwrap();
    runtime.operations.register_operation::<Parent<CHARGED>>("app", Lanes::Open, Arc::new(())).unwrap();
    let (input, receiving) = channel();
    let (sending, mut output) = channel();
    let parent = runtime.operations.submit(ParentDeclaration { endpoints: Mutex::new(Some((receiving, sending))) }).unwrap();
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
