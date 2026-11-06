/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Opt-In Local Sidecar Experiment. Production Attachment Still Uses The Framed Carrier.

/* === Modules === */

mod client;
mod codec;

/* === Re-Exports === */

pub use client::{Error, Host, Running};
pub use codec::Packet;
pub use tonic;

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
// Third Party Imports
use futures_util::{Stream, stream};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tonic::{Request, Response, Status, body::Body, codegen::http};
// Local Imports
use super::{PluginNative, Supervisor};
use harmos_domain::Message;
use harmos_sdk::__private::ResourceSet;
use harmos_sdk::operations::{Calls, WireAnswer, WireCommand};
use harmos_sdk::plugin::{Declared, Plugin};

/* === Constants === */

pub const VERSION: u32 = 1;
pub const CONTROL: &str = "/harmos.prototype.Sidecar/Control";
pub const DATA: &str = "/harmos.prototype.Sidecar/Data";

/* === Macros === */

/* === Type Aliases === */

type HttpFuture = Pin<Box<dyn Future<Output = Result<http::Response<Body>, Infallible>> + Send>>;
type ArrowStream = Pin<Box<dyn Stream<Item = Result<Packet, Status>> + Send>>;

/* === Structs === */

#[derive(Clone, Copy)]
pub struct Settings {
    pub message_bytes: NonZeroUsize,
    pub queued_batches: NonZeroUsize,
    pub window_bytes: NonZeroUsize,
    pub calls: NonZeroUsize,
}

#[derive(Clone)]
struct Endpoint {
    state: Arc<State>,
}

struct State {
    opening: super::SharedOpening,
    table: super::Table,
    host: Supervisor,
    token: String,
    generation: u64,
    retired: AtomicBool,
    settings: Settings,
    flight: Mutex<Option<Flight>>,
    lifecycle: [Vec<u8>; 2],
}

struct Flight {
    id: u64,
    cancellation: watch::Sender<bool>,
    progress: watch::Sender<Progress>,
}

#[derive(Clone, Copy, Default)]
struct Progress {
    produced: u64,
    blocked: bool,
    finished: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Settings {
    fn validate(self) -> std::io::Result<Self> {
        if self.message_bytes.get() > 16 * 1024 * 1024 || self.window_bytes.get() > i32::MAX as usize {
            return Err(std::io::Error::other("prototype limits exceed supported transport capacity"));
        }
        Ok(self)
    }
}

impl State {
    fn admit(&self, request: &Request<Packet>) -> Result<(), Status> {
        let authorization = request.metadata().get("authorization").and_then(|value| value.to_str().ok());
        if authorization != Some(self.token.as_str()) {
            return Err(Status::unauthenticated("generation credentials refused"));
        }
        if self.retired.load(Ordering::Acquire) || request.get_ref().generation != self.generation {
            return Err(Status::failed_precondition("generation retired"));
        }
        if request.get_ref().version != VERSION {
            return Err(Status::failed_precondition("prototype version refused"));
        }
        Ok(())
    }

    async fn control(&self, request: Request<Packet>) -> Result<Response<Packet>, Status> {
        self.admit(&request)?;
        let packet = request.into_inner();
        let mut response = Packet::default();
        if packet.phase == 0 {
            response.fingerprint = self.lifecycle[0].clone();
            response.context = self.lifecycle[1].clone();
            return Ok(Response::new(response));
        }
        if packet.phase == 10 || packet.phase == 11 || packet.phase == 12 {
            let (cancellation, mut progress) = {
                let flight = self.flight.lock().map_err(internal)?;
                let flight = flight
                    .as_ref()
                    .filter(|flight| flight.id == packet.invocation)
                    .ok_or_else(|| Status::not_found("Arrow invocation missing"))?;
                (flight.cancellation.clone(), flight.progress.subscribe())
            };
            if packet.phase == 10 {
                cancellation.send_replace(true);
                progress.wait_for(|value| value.finished).await.map_err(internal)?;
            }
            if packet.phase == 11 {
                progress.wait_for(|value| value.blocked || value.finished).await.map_err(internal)?;
            }
            let progress = *progress.borrow();
            response.count = progress.produced;
            response.phase = u32::from(progress.finished);
            return Ok(Response::new(response));
        }
        if packet.phase == 20 {
            self.retired.store(true, Ordering::Release);
            if let Some(flight) = self.flight.lock().map_err(internal)?.as_ref() {
                flight.cancellation.send_replace(true);
            }
            self.opening.lock().await.cleanup().await.map_err(internal)?;
            return Ok(Response::new(response));
        }
        let invocation = packet.invocation;
        let context: [u8; 32] = packet.context.as_slice().try_into().map_err(|_| Status::invalid_argument("context length"))?;
        let command = match packet.phase {
            1 => {
                let fingerprint = packet.fingerprint.as_slice().try_into().map_err(|_| Status::invalid_argument("fingerprint length"))?;
                // Translate Lifecycle Protobuf Into Its Existing Internal Codec.
                match lifecycle_payload(&packet.operation, &packet.payload)? {
                    Some(payload) => WireCommand::Prepare {
                        invocation,
                        operation: packet.operation,
                        fingerprint,
                        generation: packet.generation,
                        payload,
                    },
                    None => WireCommand::PrepareGrpc {
                        invocation,
                        operation: packet.operation,
                        fingerprint,
                        generation: packet.generation,
                        payload: packet.payload,
                    },
                }
            }
            2 => WireCommand::Initialize { invocation, context },
            3 => WireCommand::Attempt { invocation, context, values: Vec::new() },
            4 => WireCommand::Finalize { invocation },
            5 => WireCommand::Cancel { invocation },
            _ => return Err(Status::unimplemented("prototype phase")),
        };
        let host = self.host.for_operation(context);
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(Calls(Arc::new(host.clone())))).map_err(internal)?;
        resources.insert(Arc::new(host.clone())).map_err(internal)?;
        let execution = Arc::new(harmos_sdk::ExecutionContext::new(resources, host.context().clone()));
        let phase = self.table.lock().map_err(internal)?.begin(command, execution);
        match phase.await {
            WireAnswer::Ready { payload } => response.payload = payload,
            WireAnswer::Failed { failure } => {
                response.phase = 1;
                response.payload = serde_json::to_vec(&failure).map_err(internal)?;
            }
            WireAnswer::Stream { .. } => return Err(Status::unimplemented("operation stream phases are outside this experiment")),
        }
        Ok(Response::new(response))
    }

    fn data(&self, request: Request<Packet>) -> Result<Response<ArrowStream>, Status> {
        self.admit(&request)?;
        let packet = request.into_inner();
        if packet.payload.is_empty() || packet.count == 0 {
            return Err(Status::invalid_argument("Arrow batch and positive count required"));
        }
        let mut flight = self.flight.lock().map_err(internal)?;
        if flight.as_ref().is_some_and(|flight| !flight.progress.borrow().finished) {
            return Err(Status::resource_exhausted("one Arrow invocation at a time"));
        }
        let (cancellation, mut cancelled) = watch::channel(false);
        let (progress, _) = watch::channel(Progress::default());
        *flight = Some(Flight {
            id: packet.invocation,
            cancellation,
            progress: progress.clone(),
        });
        let (output, input) = mpsc::channel(self.settings.queued_batches.get());
        tokio::spawn(async move {
            for sequence in 0..packet.count {
                // Reserve Before Copying; Cancellation Never Waits For Data Capacity.
                let slot = match output.try_reserve() {
                    Ok(slot) => slot,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        progress.send_modify(|value| value.blocked = true);
                        tokio::select! {
                            biased;
                            _ = cancelled.wait_for(|value| *value) => break,
                            slot = output.reserve() => match slot { Ok(slot) => slot, Err(_) => break },
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                };
                if *cancelled.borrow() {
                    break;
                }
                slot.send(Ok(Packet {
                    invocation: packet.invocation,
                    count: sequence,
                    payload: packet.payload.clone(),
                    ..Packet::default()
                }));
                progress.send_modify(|value| {
                    value.produced += 1;
                    value.blocked = false;
                });
            }
            progress.send_modify(|value| value.finished = true);
        });
        let output = stream::unfold(input, |mut input| async move { input.recv().await.map(|packet| (packet, input)) });
        Ok(Response::new(Box::pin(output)))
    }
}

impl tonic::server::UnaryService<Packet> for Endpoint {
    type Response = Packet;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Packet>, Status>> + Send>>;
    fn call(&mut self, request: Request<Packet>) -> Self::Future {
        let state = self.state.clone();
        Box::pin(async move { state.control(request).await })
    }
}

impl tonic::server::ServerStreamingService<Packet> for Endpoint {
    type Response = Packet;
    type ResponseStream = ArrowStream;
    type Future = Pin<Box<dyn Future<Output = Result<Response<ArrowStream>, Status>> + Send>>;
    fn call(&mut self, request: Request<Packet>) -> Self::Future {
        let result = self.state.data(request);
        Box::pin(async move { result })
    }
}

impl tonic::server::NamedService for Endpoint {
    const NAME: &'static str = "harmos.prototype.Sidecar";
}

impl<B> tonic::codegen::Service<http::Request<B>> for Endpoint
where
    B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError> + Send,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = HttpFuture;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let endpoint = self.clone();
        let limit = self.state.settings.message_bytes.get();
        Box::pin(async move {
            let mut grpc = tonic::server::Grpc::new(codec::Codec).max_decoding_message_size(limit).max_encoding_message_size(limit);
            let response = match request.uri().path() {
                CONTROL => grpc.unary(endpoint, request).await,
                DATA => grpc.server_streaming(endpoint, request).await,
                _ => Status::unimplemented("prototype method").into_http(),
            };
            Ok(response)
        })
    }
}

/* === Public Functions === */

pub fn credentials() -> std::io::Result<String> {
    let mut token = [0u8; 32];
    getrandom::fill(&mut token).map_err(std::io::Error::other)?;
    Ok(token.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub async fn serve<P>(listener: TcpListener, token: String, generation: u64, settings: Settings) -> std::io::Result<()>
where
    P: Plugin<Context = Supervisor> + Declared,
{
    let settings = settings.validate()?;
    if !listener.local_addr()?.ip().is_loopback() || token.len() != 64 || generation == 0 {
        return Err(std::io::Error::other("private loopback endpoint, random token and generation required"));
    }
    let native = PluginNative::<P>::new(settings.calls)?;
    use harmos_domain::model::operations::lifecycle::Descriptor;
    let lifecycle = [
        Descriptor::of_initialize::<P>().map_err(std::io::Error::other)?.contract().fingerprint().0.to_vec(),
        Descriptor::of_shutdown::<P>().map_err(std::io::Error::other)?.contract().fingerprint().0.to_vec(),
    ];
    let mut opening = super::plugin::opening(&native)?;
    let serving = (opening.prepare)(P::ID).map_err(std::io::Error::other)?;
    let table = serving.operations.clone();
    opening.serving = Some(serving);
    let (outgoing, mut callbacks) = mpsc::channel(1);
    let host = Supervisor::new(outgoing, || None);
    let state = Arc::new(State {
        opening: Arc::new(tokio::sync::Mutex::new(opening)),
        table,
        host: host.clone(),
        token,
        generation,
        retired: AtomicBool::new(false),
        settings,
        flight: Mutex::new(None),
        lifecycle,
    });
    // Refuse Unsupported Host Callbacks Explicitly Instead Of Leaving A Waiter.
    let callback_task = tokio::spawn(async move {
        while let Some(callback) = callbacks.recv().await {
            if let super::host::Outgoing::Frame(crate::Frame::Call { id, .. }) = callback {
                host.receive_refusal(id, crate::Refusal::of("prototype real path has no host callbacks"));
            }
        }
    });
    let incoming = stream::unfold(listener, |listener| async move { Some((listener.accept().await.map(|(socket, _)| socket), listener)) });
    let result = tonic::transport::Server::builder()
        .initial_stream_window_size(settings.window_bytes.get() as u32)
        .initial_connection_window_size(settings.window_bytes.get() as u32)
        .http2_adaptive_window(Some(false))
        .max_concurrent_streams(settings.calls.get() as u32)
        .add_service(Endpoint { state: state.clone() })
        .serve_with_incoming(incoming)
        .await;
    state.retired.store(true, Ordering::Release);
    state.host.disconnected();
    callback_task.abort();
    let _ = callback_task.await;
    state.opening.lock().await.cleanup().await.map_err(std::io::Error::other)?;
    result.map_err(std::io::Error::other)
}

/* === Private Functions === */

fn internal(error: impl std::fmt::Display) -> Status {
    Status::internal(error.to_string())
}

fn lifecycle_payload(operation: &str, payload: &[u8]) -> Result<Option<Vec<u8>>, Status> {
    use harmos_operations::lifecycle::{Initialize, Shutdown};
    use harmos_sdk::grpc::{DecodeInput, decode};
    if operation.ends_with(".harmos.initialize") {
        return decode::<Initialize>(DecodeInput::new(payload)).map(|request| Some(request.encode())).map_err(internal);
    }
    if operation.ends_with(".harmos.shutdown") {
        return decode::<Shutdown>(DecodeInput::new(payload)).map(|request| Some(request.encode())).map_err(internal);
    }
    Ok(None)
}

/* === Tests === */
