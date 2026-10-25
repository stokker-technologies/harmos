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
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::task::Poll;

// Third Party Imports
use tokio::sync::{Mutex, watch};

// Local Imports
use super::ServingError;
use crate::operations::{EndpointSet, StreamInput, StreamOutput, StreamSettings, streaming};
use crate::{MemorySize, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;

/* === Structs === */

#[derive(Clone, Copy)]
pub struct WireStreamLimits {
    pub input: StreamSettings,
    pub output: StreamSettings,
}

#[derive(Clone)]
pub struct WireStreams {
    input: Option<Arc<dyn Input>>,
    output: Option<Arc<dyn Encoder>>,
    state: Arc<watch::Sender<State>>,

    #[cfg(feature = "grpc")]
    input_codec: Option<super::protobuf::ItemCodec>,

    #[cfg(feature = "grpc")]
    output_codec: Option<super::protobuf::ItemCodec>,
}

struct Sending<T>(Mutex<Option<StreamOutput<T>>>, Arc<AtomicBool>);
struct Receiving<T>(Mutex<StreamInput<T>>, Arc<AtomicBool>);
#[derive(Clone, Copy)]
struct State {
    epoch: u64,
    phase: Phase,
}

pub(super) struct Attempt {
    streams: WireStreams,
    epoch: u64,
    ended: bool,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Running,
    Ended,
    Interrupted,
    Closed,
}

/* === Traits === */

pub trait WireEndpoints: EndpointSet + Sized {
    fn empty() -> Option<Self> {
        None
    }

    fn bind(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError>;
}

trait Input: Send + Sync {
    fn send<'a>(&'a self, payload: &'a [u8], #[cfg(feature = "grpc")] codec: Option<super::protobuf::ItemCodec>) -> Pending<'a, ()>;

    #[cfg(feature = "grpc")]
    fn item_type(&self) -> std::any::TypeId;

    fn finish(&self) -> Result<(), ServingError>;
    fn transferred(&self) -> bool;
}
trait Encoder: Send + Sync {
    #[cfg(feature = "grpc")]
    fn item_type(&self) -> std::any::TypeId;

    fn receive<'a>(&'a self, streams: &'a WireStreams, epoch: u64) -> Pending<'a, Option<Vec<u8>>>;
    fn transferred(&self) -> bool;
}

/* === Implementations === */

impl WireEndpoints for () {
    fn empty() -> Option<Self> {
        Some(())
    }
    fn bind(_: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        Err(ServingError::StreamUnsupported)
    }
}

impl WireStreamLimits {
    pub fn validate(&self) -> Result<(), ServingError> {
        streaming::validate(&self.input).map_err(ServingError::Stream)?;
        streaming::validate(&self.output).map_err(ServingError::Stream)
    }
}

impl WireStreams {
    fn new(input: Option<Arc<dyn Input>>, output: Option<Arc<dyn Encoder>>) -> Self {
        let (state, _) = watch::channel(State { epoch: 0, phase: Phase::Waiting });
        Self {
            input,
            output,
            state: Arc::new(state),
            #[cfg(feature = "grpc")]
            input_codec: None,
            #[cfg(feature = "grpc")]
            output_codec: None,
        }
    }

    #[cfg(feature = "grpc")]
    pub(super) fn protobuf(mut self, codec: super::protobuf::Codec) -> Result<Self, ServingError> {
        if self.input.as_ref().map(|input| input.item_type()) != codec.input.map(|codec| codec.item_type)
            || self.output.as_ref().map(|output| output.item_type()) != codec.output.map(|codec| codec.item_type)
        {
            return Err(ServingError::Catalog);
        }
        self.input_codec = codec.input;
        self.output_codec = codec.output;
        Ok(self)
    }

    pub fn transferred(&self) -> bool {
        self.input.as_ref().is_some_and(|input| input.transferred()) || self.output.as_ref().is_some_and(|output| output.transferred())
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.send", level = "trace", skip_all)]
    pub async fn send(&self, epoch: u64, payload: &[u8]) -> Result<(), ServingError> {
        if payload.len() > super::codec::LIMIT {
            return Err(ServingError::Stream(streaming::Error::Capacity));
        }
        let input = self.input.as_ref().ok_or(ServingError::StreamPhase)?;
        let mut changed = self.state.subscribe();
        self.check_input(epoch)?;
        let mut sending = input.send(
            payload,
            #[cfg(feature = "grpc")]
            self.input_codec,
        );
        loop {
            let mut change = std::pin::pin!(changed.changed());
            let result = std::future::poll_fn(|cx| {
                if change.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(None);
                }
                sending.as_mut().poll(cx).map(Some)
            })
            .await;
            if let Some(result) = result {
                return result;
            }
            self.check_input(epoch)?;
        }
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.finish_input", level = "trace", skip_all)]
    pub fn finish_input(&self, epoch: u64) -> Result<(), ServingError> {
        self.check_input(epoch)?;
        self.input.as_ref().ok_or(ServingError::StreamPhase)?.finish()
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.receive", level = "trace", skip_all)]
    pub async fn receive(&self, epoch: u64) -> Result<Option<Vec<u8>>, ServingError> {
        self.output.as_ref().ok_or(ServingError::StreamPhase)?.receive(self, epoch).await
    }

    fn check_input(&self, epoch: u64) -> Result<(), ServingError> {
        let state = *self.state.borrow();
        if (state.epoch == epoch && state.phase == Phase::Running) || (state.epoch == 0 && epoch == 1 && state.phase == Phase::Waiting) {
            return Ok(());
        }
        Err(ServingError::StreamPhase)
    }

    pub(super) fn start(&self, expected: u64) -> Result<Attempt, ServingError> {
        let mut epoch = None;
        self.state.send_if_modified(|state| {
            if matches!(state.phase, Phase::Running | Phase::Closed) || (state.epoch != 0 && self.transferred()) {
                return false;
            }
            let Some(next) = state.epoch.checked_add(1) else {
                return false;
            };
            if expected != next {
                return false;
            }
            state.epoch = next;
            state.phase = Phase::Running;
            epoch = Some(next);
            true
        });
        Ok(Attempt { streams: self.clone(), epoch: epoch.ok_or(ServingError::StreamPhase)?, ended: false })
    }

    pub(super) fn close(&self) {
        self.state.send_modify(|state| state.phase = Phase::Closed);
    }

    fn output_phase(&self, epoch: u64) -> Result<Phase, ServingError> {
        let state = *self.state.borrow();
        if state.phase == Phase::Closed {
            return Err(ServingError::Closing);
        }
        if state.epoch == epoch && epoch != 0 {
            return match state.phase {
                Phase::Running | Phase::Ended => Ok(state.phase),
                _ => Err(ServingError::StreamPhase),
            };
        }
        if state.epoch.checked_add(1) == Some(epoch)
            && matches!(state.phase, Phase::Waiting | Phase::Ended | Phase::Interrupted)
            && (state.epoch == 0 || !self.transferred())
        {
            return Ok(Phase::Waiting);
        }
        Err(ServingError::StreamPhase)
    }
}

impl Attempt {
    pub(super) fn end(mut self) {
        self.streams.state.send_if_modified(|state| {
            if state.epoch != self.epoch || state.phase != Phase::Running {
                return false;
            }
            state.phase = Phase::Ended;
            true
        });
        self.ended = true;
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        if self.ended {
            return;
        }
        self.streams.state.send_if_modified(|state| {
            if state.epoch != self.epoch || state.phase != Phase::Running {
                return false;
            }
            state.phase = Phase::Interrupted;
            true
        });
    }
}

impl<T: Message + MemorySize + Send + 'static> WireEndpoints for StreamInput<T> {
    fn bind(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (sender, receiver) = streaming::stream(limits.input).map_err(ServingError::Stream)?;
        let transferred = sender.transfer_flag();
        Ok((receiver, WireStreams::new(Some(Arc::new(Sending(Mutex::new(Some(sender)), transferred))), None)))
    }
}
impl<T: Message + MemorySize + Send + 'static, E: 'static> WireEndpoints for StreamOutput<T, E> {
    fn bind(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (sender, receiver) = streaming::stream(limits.output).map_err(ServingError::Stream)?;
        let transferred = sender.transfer_flag();
        Ok((sender.failing_with(), WireStreams::new(None, Some(Arc::new(Receiving(Mutex::new(receiver), transferred))))))
    }
}
impl<I, O, E> WireEndpoints for (StreamInput<I>, StreamOutput<O, E>)
where
    I: Message + MemorySize + Send + 'static,
    O: Message + MemorySize + Send + 'static,
    E: 'static,
{
    fn bind(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (input, incoming) = StreamInput::<I>::bind(limits)?;
        let (output, outgoing) = StreamOutput::<O, E>::bind(limits)?;
        Ok(((input, output), WireStreams::new(incoming.input, outgoing.output)))
    }
}

impl<T: Message + MemorySize + Send + 'static> Input for Sending<T> {
    #[cfg(feature = "grpc")]
    fn item_type(&self) -> std::any::TypeId {
        std::any::TypeId::of::<T>()
    }
    #[tracing::instrument(target = "harmos::streams", name = "streams.send", level = "trace", skip_all)]
    fn send<'a>(&'a self, payload: &'a [u8], #[cfg(feature = "grpc")] codec: Option<super::protobuf::ItemCodec>) -> Pending<'a, ()> {
        Box::pin(async move {
            let input = self.0.try_lock().map_err(|_| ServingError::StreamBusy)?;
            let input = input.as_ref().ok_or(ServingError::StreamPhase)?;
            #[cfg(feature = "grpc")]
            if let Some(codec) = codec {
                let bytes = (codec.memory)(payload).map_err(ServingError::Codec)?;
                return input
                    .send_with(bytes, || {
                        let mut item = None::<T>;
                        (codec.decode)(payload, &mut item).map_err(streaming::Error::Codec)?;
                        item.ok_or_else(|| streaming::Error::Codec(crate::MessageError::borrowed("protobuf stream item projection failed")))
                    })
                    .await
                    .map_err(ServingError::Stream);
            }
            input.decode(payload).await.map_err(ServingError::Stream)
        })
    }
    fn finish(&self) -> Result<(), ServingError> {
        let mut input = self.0.try_lock().map_err(|_| ServingError::StreamBusy)?;
        input.take();
        Ok(())
    }
    fn transferred(&self) -> bool {
        self.1.load(Ordering::Acquire)
    }
}
impl<T: Message + MemorySize + Send + 'static> Encoder for Receiving<T> {
    #[cfg(feature = "grpc")]
    fn item_type(&self) -> std::any::TypeId {
        std::any::TypeId::of::<T>()
    }
    #[tracing::instrument(target = "harmos::streams", name = "streams.receive", level = "trace", skip_all)]
    fn receive<'a>(&'a self, streams: &'a WireStreams, epoch: u64) -> Pending<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let mut output = self.0.try_lock().map_err(|_| ServingError::StreamBusy)?;
            let mut changed = streams.state.subscribe();
            loop {
                let phase = streams.output_phase(epoch)?;
                if phase == Phase::Ended {
                    return match output.try_recv() {
                        Ok(item) => encode(
                            &*item,
                            #[cfg(feature = "grpc")]
                            streams.output_codec,
                        )
                        .map(Some),
                        Err(_) => Ok(None),
                    };
                }
                if phase == Phase::Waiting {
                    changed.changed().await.map_err(|_| ServingError::Closing)?;
                    continue;
                }
                let mut receiving = std::pin::pin!(output.recv());
                let mut change = std::pin::pin!(changed.changed());
                let result = std::future::poll_fn(|cx| {
                    if change.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(None);
                    }
                    receiving.as_mut().poll(cx).map(Some)
                })
                .await;
                if let Some(item) = result {
                    return item
                        .map(|item| {
                            encode(
                                &*item,
                                #[cfg(feature = "grpc")]
                                streams.output_codec,
                            )
                        })
                        .transpose();
                }
            }
        })
    }
    fn transferred(&self) -> bool {
        self.1.load(Ordering::Acquire)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn encode<T: Message + 'static>(
    item: &T, #[cfg(feature = "grpc")] codec: Option<super::protobuf::ItemCodec>,
) -> Result<Vec<u8>, ServingError> {
    #[cfg(feature = "grpc")]
    if let Some(codec) = codec {
        let bytes = (codec.length)(item).map_err(ServingError::Codec)?;
        if bytes > super::codec::LIMIT {
            return Err(ServingError::Stream(streaming::Error::Capacity));
        }
        return (codec.encode)(item).map_err(ServingError::Codec);
    }
    if item.encoded_len() > super::codec::LIMIT {
        return Err(ServingError::Stream(streaming::Error::Capacity));
    }
    Ok(item.encode_to_vec())
}

/* === Tests === */
