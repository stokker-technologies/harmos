/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Prototype Uses The Landed Invocation And Instance Contract.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tonic::{Request, Status, Streaming, transport::Channel};
// Local Imports
use super::{CONTROL, DATA, Packet, Settings, VERSION, codec};
use harmos_domain::operations::instance::{Id, Outcome, Phase};
use harmos_sdk::operations::{
    CallContext, ContextError, ContextValue, EndpointSet, Instance, OperationDeclaration, OperationHost, RequestClaims, Response,
    RetryPolicy,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct Host {
    control: Channel,
    data: Channel,
    token: String,
    owner: String,
    generation: u64,
    next: Arc<AtomicU64>,
    context: CallContext,
    limit: usize,
    lifecycle: [Vec<u8>; 2],
}

pub struct Running<O: OperationDeclaration> {
    host: Host,
    id: u64,
    task: Option<tokio::task::JoinHandle<Result<Response<O::Response>, Error>>>,
    status: Arc<Mutex<harmos_domain::operations::instance::Status>>,
    operation: PhantomData<fn() -> O>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),

    #[error(transparent)]
    Status(#[from] Status),

    #[error(transparent)]
    Message(#[from] harmos_domain::MessageError),

    #[error(transparent)]
    Failure(#[from] harmos_domain::Failure),

    #[error("prototype does not admit these invocation options")]
    Options,

    #[error("prototype response failed: {0}")]
    Response(String),
}

/* === Traits === */

/* === Implementations === */

impl Host {
    pub async fn connect(address: &str, token: String, owner: String, generation: u64, settings: Settings) -> Result<Self, Error> {
        let settings = settings.validate().map_err(|error| Error::Response(error.to_string()))?;
        let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
            .map_err(|error| Error::Response(error.to_string()))?
            .initial_stream_window_size(settings.window_bytes.get() as u32)
            .initial_connection_window_size(settings.connection_window_bytes.get() as u32)
            .http2_adaptive_window(false)
            .max_frame_size(64 * 1024)
            .tcp_nodelay(true)
            .buffer_size(settings.calls.get());
        let control = endpoint.clone().connect().await?;
        let data = endpoint.connect().await?;
        let mut host = Self {
            control,
            data,
            token,
            owner,
            generation,
            next: Arc::new(AtomicU64::new(1)),
            context: CallContext::default(),
            limit: settings.message_bytes.get(),
            lifecycle: Default::default(),
        };
        let welcome = host.phase(Packet::default()).await?;
        host.lifecycle = [welcome.fingerprint, welcome.context];
        Ok(host)
    }

    pub async fn phase(&self, mut packet: Packet) -> Result<Packet, Error> {
        packet.version = VERSION;
        packet.generation = self.generation;
        let request = self.request(packet)?;
        let mut grpc =
            tonic::client::Grpc::new(self.control.clone()).max_decoding_message_size(self.limit).max_encoding_message_size(self.limit);
        grpc.ready().await.map_err(|error| Error::Response(error.to_string()))?;
        let response = grpc.unary(request, tonic::codegen::http::uri::PathAndQuery::from_static(CONTROL), codec::Codec).await?;
        codec::profile();
        Ok(response.into_inner())
    }

    pub async fn arrow(&self, invocation: u64, payload: Vec<u8>, count: u64) -> Result<Streaming<Packet>, Error> {
        let packet = Packet {
            version: VERSION,
            generation: self.generation,
            invocation,
            payload: payload.into(),
            count,
            ..Packet::default()
        };
        let mut grpc =
            tonic::client::Grpc::new(self.data.clone()).max_decoding_message_size(self.limit).max_encoding_message_size(self.limit);
        grpc.ready().await.map_err(|error| Error::Response(error.to_string()))?;
        let response =
            grpc.server_streaming(self.request(packet)?, tonic::codegen::http::uri::PathAndQuery::from_static(DATA), codec::Codec).await?;
        Ok(response.into_inner())
    }

    pub async fn retire(&self) -> Result<(), Error> {
        self.phase(Packet { phase: 20, ..Packet::default() }).await?;
        Ok(())
    }

    async fn command(&self, id: u64, phase: u32) -> Result<Vec<u8>, Error> {
        let packet = Packet {
            invocation: id,
            phase,
            context: vec![1; 32],
            ..Packet::default()
        };
        answer(self.phase(packet).await?)
    }

    fn request(&self, packet: Packet) -> Result<Request<Packet>, Error> {
        let mut request = Request::new(packet);
        let authorization =
            self.token.parse().map_err(|error: tonic::metadata::errors::InvalidMetadataValue| Error::Response(error.to_string()))?;
        request.metadata_mut().insert("authorization", authorization);
        Ok(request)
    }
}

impl OperationHost for Host {
    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;
    async fn invoke<O: OperationDeclaration, E: EndpointSet>(
        &self, request: O::Request, retry: Option<RetryPolicy>, _: E,
    ) -> Result<Running<O>, Error> {
        if retry.is_some() || E::item_types() != (None, None) || !self.context.portable().is_empty() {
            return Err(Error::Options);
        }
        self.submit::<O>(request).await
    }
    fn context(&self) -> &CallContext {
        &self.context
    }
    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        let mut host = self.clone();
        host.context = self.context.with_value(value)?;
        Ok(host)
    }
    async fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<Running<O>, Error> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut payload = Vec::new();
        if O::ID == "harmos.initialize" {
            use harmos_domain::Message;
            use prost::Message as _;
            let request = harmos_operations::lifecycle::Initialize::decode(&request.encode())?;
            let assignments: Vec<harmos_sdk::operations::AssignedOperation> = if request.assignments.is_empty() {
                Vec::new()
            } else {
                postcard::from_bytes(&request.assignments).map_err(|error| Error::Response(error.to_string()))?
            };
            let initialization = codec::Initialization {
                config_json: request.config_json,
                generation_context: request.generation_context,
                assignments: assignments
                    .into_iter()
                    .map(|entry| codec::Assignment {
                        operation: entry.operation,
                        fingerprint: entry.fingerprint.to_vec(),
                        generation: entry.generation,
                    })
                    .collect(),
            };
            payload = initialization.encode_to_vec();
        } else {
            harmos_sdk::grpc::encode(&request, &mut payload)?;
        }
        let packet = Packet {
            invocation: id,
            phase: 1,
            operation: format!("{}.{}", self.owner, O::ID),
            fingerprint: match O::ID {
                "harmos.initialize" => self.lifecycle[0].clone(),
                "harmos.shutdown" => self.lifecycle[1].clone(),
                _ => O::definition().fingerprint.0.to_vec(),
            },
            context: vec![1; 32],
            payload: payload.into(),
            ..Packet::default()
        };
        answer(self.phase(packet).await?)?;
        if let Err(error) = self.command(id, 2).await {
            let _ = self.command(id, 4).await;
            return Err(error);
        }
        let status = Arc::new(Mutex::new(harmos_domain::operations::instance::Status {
            phase: Phase::Running,
            outcome: None,
            cancellation_requested: false,
            attempt: 1,
        }));
        let progress = status.clone();
        let host = self.clone();
        let task = tokio::spawn(async move {
            let result = host.command(id, 3).await;
            let finalized = host.command(id, 4).await;
            let result = result.and_then(|bytes| {
                finalized?;
                harmos_sdk::grpc::decode::<O::Response>(harmos_sdk::grpc::DecodeInput::new(&bytes)).map(Response::new).map_err(Error::from)
            });
            let mut status = progress.lock().expect("prototype status lock");
            status.phase = Phase::Settled;
            status.outcome = Some(if result.is_ok() { Outcome::Succeeded } else { Outcome::Failed });
            result
        });
        Ok(Running {
            host: self.clone(),
            id,
            task: Some(task),
            status,
            operation: PhantomData,
        })
    }
    async fn submit_owned<O: OperationDeclaration>(&self, _: O::Request, _: RequestClaims) -> Result<Running<O>, Error> {
        Err(Error::Options)
    }
    async fn publish_on<S: harmos_domain::StreamSpec>(&self, _: &S::Row) -> Result<(), Error>
    where
        S::Row: harmos_domain::Message,
    {
        Err(Error::Options)
    }
}

impl<O: OperationDeclaration> Instance<O> for Running<O> {
    type Error = Error;
    fn id(&self) -> Id {
        Id::new(self.id)
    }
    async fn status(&self) -> Result<harmos_domain::operations::instance::Status, Error> {
        Ok(*self.status.lock().map_err(|error| Error::Response(error.to_string()))?)
    }
    async fn request_cancel(&self) -> Result<(), Error> {
        self.host.command(self.id, 5).await?;
        self.status.lock().map_err(|error| Error::Response(error.to_string()))?.cancellation_requested = true;
        Ok(())
    }
    async fn join(mut self) -> Result<Response<O::Response>, Error> {
        self.task.take().expect("original invocation task").await.map_err(|error| Error::Response(error.to_string()))?
    }
    fn detach(mut self) -> Id {
        self.task.take();
        Id::new(self.id)
    }
}

impl<O: OperationDeclaration> Drop for Running<O> {
    fn drop(&mut self) {
        if self.task.is_some() {
            let host = self.host.clone();
            let id = self.id;
            tokio::spawn(async move {
                let _ = host.command(id, 5).await;
            });
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn answer(packet: Packet) -> Result<Vec<u8>, Error> {
    if packet.phase == 0 {
        return Ok(packet.payload.to_vec());
    }
    let failure = serde_json::from_slice::<harmos_domain::Failure>(&packet.payload).map_err(|error| Error::Response(error.to_string()))?;
    Err(Error::Failure(failure))
}

/* === Tests === */
