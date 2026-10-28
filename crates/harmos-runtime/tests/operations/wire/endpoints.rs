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
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{
    Backoff, CacheKey, Jitter, OperationPolicy, RetryPolicy, StreamInput, StreamOutput, StreamSettings, stream,
};
use harmos_runtime::{Error, Runtime};

/* === Constants === */

/* === Macros === */

macro_rules! operation {
    ($name:ident, $id:literal, $retry:literal, $cache:literal) => {
        #[harmos_runtime::operation(resources = [Trace], id = $id, response = u32, input = u32, output = u32,
                                            policy = OperationPolicy {
                                                timeout: Duration::from_secs(3), attempt_timeout: Duration::from_secs(2),
                                                retries: if $retry { RetryPolicy::OnTransient { max_retries: 1,
                                                    backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None } } }
                                                    else { RetryPolicy::Never },
                                            }, key = |_: &$name| $cache.then(|| CacheKey::new(vec![1]).unwrap()))]
        struct $name {
            consume: bool,
        }
        impl harmos_runtime::Operation for $name {
            type Error = Error;
            async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<u32>> {
                let trace = ctx.resource::<Trace>();
                trace.new.fetch_add(1, Ordering::SeqCst);
                let _guard = AttemptGuard(trace.clone());
                if !self.consume {
                    return Ok(0.into());
                }
                let (input, output) = ctx.streams();
                let mut total = 0;
                while let Some(item) = input.recv().await {
                    trace.sending.fetch_add(1, Ordering::SeqCst);
                    trace.changed.notify_one();
                    output.send_with(4, || Ok(*item + 1)).await?;
                    total += *item;
                }
                Ok(total.into())
            }
        }
    };
}

operation!(ForwardDeclaration, "endpoints.forward", false, false);
operation!(RetryDeclaration, "endpoints.retry", true, false);
operation!(CachedDeclaration, "endpoints.cache", false, true);

/* === Type Aliases === */

type StreamEndpoints = (StreamInput<u32>, StreamOutput<u32>);

/* === Structs === */

#[derive(Default)]
struct Trace {
    new: AtomicUsize,
    dropped: AtomicUsize,
    sending: AtomicUsize,
    changed: Notify,
}

struct AttemptGuard(Arc<Trace>);

#[cfg(feature = "grpc")]
#[derive(Default)]
struct Gate(Notify);

#[cfg(feature = "grpc")]
#[harmos_runtime::operation(resources = [Gate], id = "endpoints.slow", response = u32, input = u32, policy = ForwardDeclaration::POLICY)]
struct SlowDeclaration {}

#[harmos_runtime::operation(codec = manual, resources = [harmos_runtime::operations::HostContext],
    operations = [ForwardDeclaration], id = "endpoints.parent", response = u32, policy = ForwardDeclaration::POLICY)]
#[derive(Clone)]
struct ParentDeclaration {
    endpoints: Arc<Mutex<Option<StreamEndpoints>>>,
    charged: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

crate::operations::in_process!(ParentDeclaration);

impl harmos_runtime::MemorySize for ParentDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl harmos_runtime::Operation for ParentDeclaration {
    type Error = Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<u32>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let endpoints = self.endpoints.lock().unwrap().take().expect("one transfer of original endpoints");
        let child = if self.charged {
            let (requests, decoded) = stream::<ForwardDeclaration>(StreamSettings {
                items: NonZeroUsize::new(1).unwrap(),
                bytes: NonZeroUsize::new(1).unwrap(),
            })?;
            requests.send_with(1, || Ok(ForwardDeclaration { consume: true })).await?;
            context.submit_charged::<ForwardDeclaration, _>(decoded.recv().await.unwrap(), endpoints)?
        } else {
            context.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, endpoints)?
        };
        Ok((*child.await?).into())
    }
}

#[cfg(feature = "grpc")]
impl harmos_runtime::Operation for SlowDeclaration {
    type Error = Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<u32>> {
        let gate = ctx.resource::<Gate>();
        let input = ctx.streams();
        // Consume Nothing Until The Forwarder Has Handed Off A Full Window.
        gate.0.notified().await;
        let mut total = 0;
        while let Some(item) = input.recv().await {
            tokio::task::yield_now().await;
            total += *item;
        }
        Ok(total.into())
    }
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
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
    (future).await
}
async fn runtime_with_resource<T: Send + Sync + 'static>(resource: Arc<T>) -> Runtime<App> {
    Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<T>(resource);
    })
    .await
    .unwrap()
}

#[cfg(feature = "grpc")]
async fn envelope(
    value: harmos_runtime::grpc::InputEnvelope<ForwardDeclaration, u32>,
) -> harmos_runtime::operations::StreamItem<harmos_runtime::grpc::InputEnvelope<ForwardDeclaration, u32>> {
    let (output, input) = stream(StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(64).unwrap(),
    })
    .unwrap();
    output.send_with(64, || Ok(value)).await.unwrap();
    input.recv().await.unwrap()
}

#[cfg(feature = "grpc")]
type SlowEnvelope = harmos_runtime::grpc::InputEnvelope<SlowDeclaration, u32>;

#[cfg(feature = "grpc")]
type SlowCodec = harmos_runtime::grpc::Codec<harmos_runtime::grpc::Transcoded, SlowEnvelope, harmos_runtime::Response<u32>>;

#[cfg(feature = "grpc")]
fn frame(value: &SlowEnvelope) -> bytes::Bytes {
    use harmos_runtime::grpc::{Transcoded, Wire};
    let mut payload = Vec::new();
    <Transcoded as Wire<SlowEnvelope>>::encode(value, &mut payload).unwrap();
    let mut frame = vec![0];
    frame.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    frame.extend_from_slice(&payload);
    frame.into()
}

#[cfg(feature = "grpc")]
fn request_body(
    frames: impl futures_util::Stream<Item = bytes::Bytes> + Send + 'static,
) -> impl tonic::codegen::Body<Data = bytes::Bytes, Error = std::convert::Infallible> + Send + 'static {
    use futures_util::StreamExt;
    let data = |bytes| Ok(http_body::Frame::data(bytes));
    http_body_util::StreamBody::new(frames.map(data))
}

// Holds Decoded Items From A Fresh Call Until The Shared Decode Budget Refuses One.
#[cfg(feature = "grpc")]
async fn admitted_items(codec: &mut SlowCodec, frames: u32) -> usize {
    let item = |index| frame(&harmos_runtime::grpc::InputEnvelope::Item(index));
    let body = request_body(futures_util::stream::iter((0..frames).map(item)));
    let mut incoming = tonic::codec::Codec::decoder(codec).request(body);
    let mut held = Vec::new();
    let refusal = loop {
        match incoming.message().await {
            Ok(Some(item)) => held.push(item),
            Ok(None) => break None,
            Err(status) => break Some(status),
        }
    };
    assert!(refusal.as_ref().is_none_or(|status| status.code() == tonic::Code::ResourceExhausted), "{refusal:?}");
    held.len()
}

/* === Tests === */

#[cfg(feature = "grpc")]
#[tokio::test]
async fn decoded_charge_releases_on_hand_off() {
    use futures_util::StreamExt;
    use harmos_runtime::grpc::{DecodeSettings, EncodeSettings, InputEnvelope};
    const WINDOW: u32 = 32;
    const FRAMES: u32 = 128;
    let decode = DecodeSettings {
        message_bytes: NonZeroUsize::new(4096).unwrap(),
        retained_bytes: NonZeroUsize::new(8 * std::mem::size_of::<SlowEnvelope>()).unwrap(),
    };
    let encode = EncodeSettings {
        message_bytes: NonZeroUsize::new(4096).unwrap(),
        buffer_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let capacity = admitted_items(&mut SlowCodec::new(decode, encode).unwrap(), FRAMES).await;
    assert!((2..WINDOW as usize).contains(&capacity), "the window must outgrow the decode budget: {capacity}");

    let gate = Arc::new(Gate::default());
    let runtime = runtime_with_resource(gate.clone()).await;
    runtime.operations.register::<crate::operations::Single<SlowDeclaration>>(Lanes::Open).await.unwrap();
    let mut codec = SlowCodec::new(decode, encode).unwrap();
    let opened = gate.clone();
    let frames = futures_util::stream::iter(0..=FRAMES).map(move |index| {
        // Every Earlier Item Is Already Forwarded Into The Unconsumed Window.
        if index == WINDOW {
            opened.0.notify_one();
        }
        frame(&if index == 0 { InputEnvelope::Request(SlowDeclaration {}) } else { InputEnvelope::Item(index) })
    });
    let incoming = tonic::codec::Codec::decoder(&mut codec).request(request_body(frames));
    let (request, items) = bounded(harmos_runtime::grpc::open(incoming)).await.unwrap();
    let (sending, receiving) = stream(StreamSettings {
        items: NonZeroUsize::new(WINDOW as usize).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    })
    .unwrap();
    let instance = runtime.operations.submit_charged::<SlowDeclaration, _>(request, receiving).unwrap();
    let total = bounded(harmos_runtime::grpc::input(instance, sending, items)).await.unwrap();
    assert_eq!(*total, (1..=FRAMES).sum::<u32>());
    assert_eq!(admitted_items(&mut codec, FRAMES).await, capacity);
    bounded(runtime.shutdown()).await.unwrap();
}

#[tokio::test]
async fn endpoint_admission_contract() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    {
        runtime.operations.register::<crate::operations::Single<RetryDeclaration>>(Lanes::Open).await.unwrap();
        runtime.operations.register::<crate::operations::Single<CachedDeclaration>>(Lanes::Open).await.unwrap();
        let (input, receiving) = channel();
        let (sending, _output) = channel();
        assert!(matches!(
            runtime.operations.submit_endpoints::<CachedDeclaration, _>(CachedDeclaration { consume: false }, (receiving, sending)),
            Err(Error::InvalidInput { field: "operation stream cache", .. })
        ));
        assert_eq!(trace.new.load(Ordering::SeqCst), 0);
        assert!(input.is_closed());
    }

    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    bounded(input.send_with(4, || Ok(7))).await.unwrap();
    drop(input);
    let item = bounded(output.recv()).await.unwrap();
    assert_eq!(*item, 8);
    assert_eq!(*bounded(instance).await.unwrap(), 7);
    assert!(bounded(output.recv()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(*item, 8);
    bounded(runtime.shutdown()).await.unwrap();
}

#[tokio::test]
async fn cancellation_settles_owner() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
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
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(*held, 2);
    bounded(runtime.shutdown()).await.unwrap();
}

#[tokio::test]
async fn cancellation_settles_child() {
    parent_cancellation::<false>().await;
    parent_cancellation::<true>().await;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn half_close_joins() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (producer, decoded) = channel();
    producer.send_with(4, || Ok(7)).await.unwrap();
    let item = decoded.recv().await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    let incoming = futures_util::stream::iter([Ok(item)]);
    let result = bounded(harmos_runtime::grpc::input(instance, input, incoming)).await.unwrap();
    assert_eq!(*result, 7);
    let held = output.recv().await.unwrap();
    assert_eq!(*held, 8);
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn settlement_never_wait() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: false }, (receiving, sending)).unwrap();
    let result = bounded(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending())).await.unwrap();
    assert_eq!(*result, 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn input_failure_settles_attempt() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to preserve.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        while initialized.new.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        Err(tonic::Status::data_loss("broken input"))
    });
    let error = bounded(harmos_runtime::grpc::input(instance, input, incoming)).await.unwrap_err();
    let harmos_runtime::grpc::InputError::Forward { source, settlement } = error else { panic!("input failure missing") };
    assert!(matches!(*source, Error::TaskFailed { domain: "gRPC input", .. }));
    assert!(matches!(settlement.as_deref(), Some(Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn input_drop_cancels() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    drop(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending()));
    bounded(runtime.shutdown()).await.unwrap();
    // Abandoned before it started, the instance never constructed.
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "grpc")]
#[tokio::test(start_paused = true)]
async fn deadline_settles_peer() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    let error = bounded(harmos_runtime::grpc::input(instance, input, futures_util::stream::pending())).await.unwrap_err();
    assert!(
        matches!(error, harmos_runtime::grpc::InputError::Operation(source) if matches!(*source, Error::OperationRefused(harmos_runtime::Refusal::TimedOut)))
    );
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_exchange_polls() {
    use futures_util::StreamExt;
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (producer, decoded) = stream::<u32>(StreamSettings {
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
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, futures_util::stream::iter(items), output);
    for expected in 1..=3 {
        let event = bounded(exchange.next()).await.unwrap().unwrap();
        assert!(matches!(event, harmos_runtime::grpc::Event::Item(item) if *item == expected));
    }
    let event = bounded(exchange.next()).await.unwrap().unwrap();
    assert!(matches!(event, harmos_runtime::grpc::Event::Completion(response) if *response == 3));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn exchange_failure_settles_attempt() {
    use futures_util::StreamExt;
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to retain.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        while initialized.new.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        Err(tonic::Status::data_loss("broken input"))
    });
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, incoming, output);
    let error = match bounded(exchange.next()).await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("expected forwarding failure"),
    };
    assert!(matches!(error, harmos_runtime::grpc::InputError::Forward { settlement: Some(ref settlement), .. }
        if matches!(settlement.as_ref(), Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn exchange_drop_cancels() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    drop(harmos_runtime::grpc::exchange(instance, input, futures_util::stream::pending(), output));
    bounded(runtime.shutdown()).await.unwrap();
    // Abandoned before it started, the instance never constructed.
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn charged_request_ownership() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let (requests, decoded) = stream::<ForwardDeclaration>(StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(1).unwrap(),
    })
    .unwrap();
    requests.send_with(1, || Ok(ForwardDeclaration { consume: false })).await.unwrap();
    let request = decoded.recv().await.unwrap();
    assert!(matches!(runtime.operations.submit_charged::<ForwardDeclaration, _>(request, ()), Err(Error::InvalidInput { .. })));
    drop(bounded(requests.reserve(1)).await.unwrap());
    assert_eq!(trace.new.load(Ordering::SeqCst), 0);

    requests.send_with(1, || Ok(ForwardDeclaration { consume: false })).await.unwrap();
    let request = decoded.recv().await.unwrap();
    let (_input, receiving) = channel();
    let (sending, _output) = channel();
    let instance = runtime.operations.submit_charged::<ForwardDeclaration, _>(request, (receiving, sending)).unwrap();
    assert_eq!(*bounded(instance).await.unwrap(), 0);
    drop(bounded(requests.reserve(1)).await.unwrap());
    bounded(runtime.shutdown()).await.unwrap();
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn opening_admission_exchange() {
    use futures_util::StreamExt;
    use harmos_runtime::grpc::InputEnvelope;
    {
        let missing = harmos_runtime::grpc::open::<bool, u32, _>(futures_util::stream::empty()).await;
        assert!(matches!(missing, Err(error) if error.code() == tonic::Code::InvalidArgument));
        let early = futures_util::stream::iter([Ok(envelope(InputEnvelope::Item(1)).await)]);
        assert!(matches!(harmos_runtime::grpc::open(early).await, Err(error) if error.code() == tonic::Code::InvalidArgument));
        let frames = futures_util::stream::iter([
            Ok(envelope(InputEnvelope::Request(ForwardDeclaration { consume: true })).await),
            Ok(envelope(InputEnvelope::Request(ForwardDeclaration { consume: false })).await),
            Ok(envelope(InputEnvelope::Item(7)).await),
        ]);
        let (_request, items) = harmos_runtime::grpc::open(frames).await.unwrap();
        tokio::pin!(items);
        assert!(matches!(items.next().await.unwrap(), Err(error) if error.code() == tonic::Code::InvalidArgument));
        assert!(items.next().await.is_none());
    }

    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    let frames = futures_util::stream::iter([
        Ok(envelope(InputEnvelope::Request(ForwardDeclaration { consume: true })).await),
        Ok(envelope(InputEnvelope::Item(7)).await),
    ]);
    let (request, incoming) = bounded(harmos_runtime::grpc::open(frames)).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance = runtime.operations.submit_charged::<ForwardDeclaration, _>(request, (receiving, sending)).unwrap();
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, incoming, output);
    assert!(matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos_runtime::grpc::Event::Item(item) if *item == 8));
    assert!(matches!(bounded(exchange.next()).await.unwrap().unwrap(), harmos_runtime::grpc::Event::Completion(value) if *value == 7));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}

async fn parent_cancellation<const CHARGED: bool>() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime.operations.register::<crate::operations::Single<ForwardDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<ParentDeclaration>>(Lanes::Open).await.unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let parent = runtime
        .operation(ParentDeclaration {
            endpoints: Arc::new(Mutex::new(Some((receiving, sending)))),
            charged: CHARGED,
        })
        .start()
        .await
        .unwrap();
    bounded(input.send_with(4, || Ok(7))).await.unwrap();
    let held = bounded(output.recv()).await.unwrap();
    parent.cancel();
    assert!(bounded(parent).await.is_err());
    assert!(input.is_closed());
    assert!(bounded(output.recv()).await.is_none());
    assert_eq!(*held, 8);
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(runtime.shutdown()).await.unwrap();
}
