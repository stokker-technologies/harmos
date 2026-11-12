/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Contract Client

/* === Modules === */

mod admission;
mod codec;
mod method;

/* === Re-Exports === */

pub use method::{Method, NoItems};

/* === Imports === */

// Standard Library Imports
use std::{marker::PhantomData, num::NonZeroUsize, sync::Arc};

// Third Party Imports
use prost::Message;
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tonic::{Request, Status, Streaming, transport::Channel};

// Local Imports
use codec::{Frame, Output, Wire};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Consumer-Owned Decoded Memory, Wire And Outgoing Queue Bounds. Operation Policy Stays With The Server.
#[derive(Clone, Copy)]
pub struct ClientSettings {
    /// Shared Across Clones And Calls; Released When Decoded Values Reach The Caller.
    pub retained_bytes: NonZeroUsize,

    pub message_bytes: NonZeroUsize,
    pub input_items: NonZeroUsize,
    pub input_bytes: NonZeroUsize,
}

#[derive(Clone)]
pub struct Client {
    retained: Arc<Semaphore>,
    channel: Channel,
    settings: ClientSettings,
}

/// An Ordinary Watch. Dropping It Ends Observation Without A Final-Result Promise.
pub struct Watching<O> {
    incoming: Streaming<Output<O, NoItems>>,
}

/// The Independently Owned Input Direction Of One Managed Invocation.
/// Finish It Explicitly To Tell The Server That No More Items Will Arrive.
#[must_use = "finish the input direction after sending its items"]
pub struct Input<I> {
    outgoing: mpsc::Sender<Frame>,
    budget: Arc<Semaphore>,
    settings: ClientSettings,
    marker: PhantomData<fn(I)>,
}

/// One Managed Invocation. Dropping It Abandons The Response And Aborts The RPC.
#[must_use = "join or cancel the call to observe its final response"]
pub struct Running<R, I, O> {
    outgoing: mpsc::Sender<Frame>,
    budget: Arc<Semaphore>,
    settings: ClientSettings,
    incoming: Streaming<Output<R, O>>,
    input: bool,
    output: bool,
    input_finished: bool,
    response: Option<(R, OwnedSemaphorePermit)>,
    failure: Option<Status>,
    ended: bool,
    marker: PhantomData<fn(I)>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Client {
    pub fn new(channel: Channel, settings: ClientSettings) -> Result<Self, Status> {
        if settings.retained_bytes.get() > Semaphore::MAX_PERMITS
            || settings.retained_bytes.get() > u32::MAX as usize
            || settings.input_bytes.get() > Semaphore::MAX_PERMITS
            || settings.input_bytes.get() > u32::MAX as usize
            || settings.input_items.get() > Semaphore::MAX_PERMITS
            || settings.message_bytes.get() > u32::MAX as usize
        {
            return Err(Status::invalid_argument("client decoded, wire or queue bounds exceed transport capacity"));
        }
        Ok(Self {
            retained: Arc::new(Semaphore::new(settings.retained_bytes.get())),
            channel,
            settings,
        })
    }

    /// Read One Consumer-Bounded Discovery Page Through The Shared Host Protocol.
    pub async fn catalog(
        &self, request: Request<harmos_protocol::host::CatalogEntriesListRequest>,
    ) -> Result<tonic::Response<harmos_protocol::host::CatalogEntriesListResponse>, Status> {
        self.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.host.v1.CatalogEntries/List"), request, host_page()).await
    }

    /// Call The Ordinary Unary Method With Generated Messages And Caller Metadata.
    pub async fn call<Q, R>(&self, method: &Method<Q, R>, request: Request<Q>) -> Result<tonic::Response<R>, Status>
    where
        Q: Message + Default + Send + 'static,
        R: Message + Default + Send + 'static,
    {
        method.validate()?;
        if method.contract.grpc.as_ref().is_some_and(|grpc| grpc.managed) {
            return Err(Status::invalid_argument("call requires an ordinary unary method; use start for a cooperative final result"));
        }
        self.unary(method.path.clone(), request, descriptors(method)?.0).await
    }

    async fn unary<Q, R>(
        &self, path: tonic::codegen::http::uri::PathAndQuery, request: Request<Q>, response: MessageDescriptor,
    ) -> Result<tonic::Response<R>, Status>
    where
        Q: Message + Default + Send + 'static,
        R: Message + Default + Send + 'static,
    {
        let (metadata, extensions, value) = request.into_parts();
        let budget = Arc::new(Semaphore::new(self.settings.input_bytes.get()));
        let frame = frame(&budget, self.settings, None, &value).await?;
        let mut client = self.transport();
        client.ready().await.map_err(|error| Status::unavailable(error.to_string()))?;
        let response =
            client.unary(Request::from_parts(metadata, extensions, frame), path, self.codec::<R, NoItems>(false, response, None)).await?;
        let (metadata, value, extensions) = response.into_parts();
        let Output::Response(value, _permit) = value else {
            return Err(Status::data_loss("unexpected unary item"));
        };
        Ok(tonic::Response::from_parts(metadata, value, extensions))
    }

    /// Start A Managed Unary, Upload, Download Or Bidirectional Invocation.
    pub async fn start<Q, R, I, O>(&self, method: &Method<Q, R, I, O>, request: Request<Q>) -> Result<Running<R, I, O>, Status>
    where
        Q: Message + Default + Send + 'static,
        R: Message + Default + Send + 'static,
        I: Message + Default + Send + 'static,
        O: Message + Default + Send + 'static,
    {
        method.validate()?;
        if !method.contract.grpc.as_ref().is_some_and(|grpc| grpc.managed) {
            return Err(Status::invalid_argument("start requires a declared cooperative final result"));
        }
        let (response, output) = descriptors(method)?;
        let (metadata, extensions, value) = request.into_parts();
        let budget = Arc::new(Semaphore::new(self.settings.input_bytes.get()));
        let opening = frame(&budget, self.settings, Some(1), &value).await?;
        let (outgoing, incoming) = mpsc::channel(self.settings.input_items.get());
        outgoing.send(opening).await.map_err(|_| Status::cancelled("client input closed"))?;
        let stream =
            futures_util::stream::unfold(incoming, |mut receiver| async move { receiver.recv().await.map(|frame| (frame, receiver)) });
        let mut client = self.transport();
        client.ready().await.map_err(|error| Status::unavailable(error.to_string()))?;
        let response = client
            .streaming(Request::from_parts(metadata, extensions, stream), method.path.clone(), self.codec::<R, O>(true, response, output))
            .await?;
        Ok(Running {
            outgoing,
            budget,
            settings: self.settings,
            incoming: response.into_inner(),
            input: method.input,
            output: method.output,
            input_finished: false,
            response: None,
            failure: None,
            ended: false,
            marker: PhantomData,
        })
    }

    /// Observe An Ordinary Server Stream Without A Cooperative Final-Result Promise.
    pub async fn watch<Q, R, O>(&self, method: &Method<Q, R, NoItems, O>, request: Request<Q>) -> Result<Watching<O>, Status>
    where
        Q: Message + Default + Send + 'static,
        R: Message + Default + Send + 'static,
        O: Message + Default + Send + 'static,
    {
        method.validate()?;
        if !method.output || method.contract.grpc.as_ref().is_some_and(|grpc| grpc.managed) {
            return Err(Status::invalid_argument("watch requires an ordinary server-streaming method"));
        }
        let (_, output) = descriptors(method)?;
        let (metadata, extensions, value) = request.into_parts();
        let budget = Arc::new(Semaphore::new(self.settings.input_bytes.get()));
        let opening = frame(&budget, self.settings, None, &value).await?;
        let mut client = self.transport();
        client.ready().await.map_err(|error| Status::unavailable(error.to_string()))?;
        let response = client
            .server_streaming(
                Request::from_parts(metadata, extensions, opening),
                method.path.clone(),
                self.codec::<O, NoItems>(false, output.expect("validated output"), None),
            )
            .await?;
        Ok(Watching { incoming: response.into_inner() })
    }

    fn transport(&self) -> tonic::client::Grpc<Channel> {
        tonic::client::Grpc::new(self.channel.clone())
            .max_encoding_message_size(self.settings.message_bytes.get())
            .max_decoding_message_size(self.settings.message_bytes.get())
    }

    fn codec<R, O>(&self, envelope: bool, response: MessageDescriptor, output: Option<MessageDescriptor>) -> Wire<R, O> {
        Wire {
            retained: self.retained.clone(),
            response,
            output,
            envelope,
            limit: self.settings.message_bytes.get(),
            marker: PhantomData,
        }
    }
}

impl<O: Message + Default> Watching<O> {
    pub async fn next(&mut self) -> Result<Option<O>, Status> {
        match self.incoming.message().await? {
            Some(Output::Response(value, _permit)) => Ok(Some(value)),
            Some(Output::Item(..)) => Err(Status::data_loss("Watch returned a control envelope")),
            None => Ok(None),
        }
    }
}

impl<I: Message> Input<I> {
    pub async fn send(&mut self, item: I) -> Result<(), Status> {
        let frame = frame(&self.budget, self.settings, Some(2), &item).await?;
        self.outgoing.send(frame).await.map_err(|_| Status::cancelled("operation input closed"))
    }

    pub async fn finish(self) -> Result<(), Status> {
        let frame = frame(&self.budget, self.settings, Some(4), &()).await?;
        self.outgoing.send(frame).await.map_err(|_| Status::cancelled("operation input closed"))
    }
}

impl<R: Message + Default, I: Message + Default, O: Message + Default> Running<R, I, O> {
    /// Move Input To A Separate Task While This Handle Receives Output And Settlement.
    pub fn take_input(&mut self) -> Result<Input<I>, Status> {
        if !self.input || self.input_finished || self.ended {
            return Err(Status::failed_precondition("operation input is closed"));
        }
        self.input_finished = true;
        Ok(Input {
            outgoing: self.outgoing.clone(),
            budget: self.budget.clone(),
            settings: self.settings,
            marker: PhantomData,
        })
    }

    pub async fn send(&mut self, item: I) -> Result<(), Status> {
        if !self.input || self.input_finished || self.ended {
            return Err(Status::failed_precondition("operation input is closed"));
        }
        let frame = frame(&self.budget, self.settings, Some(2), &item).await?;
        self.outgoing.send(frame).await.map_err(|_| Status::cancelled("operation input closed"))
    }

    pub async fn finish_input(&mut self) -> Result<(), Status> {
        if !self.input || self.input_finished || self.ended {
            return Err(Status::failed_precondition("operation input is closed"));
        }
        self.control(4).await?;
        self.input_finished = true;
        Ok(())
    }

    pub async fn request_cancel(&self) -> Result<(), Status> {
        self.control(3).await
    }

    pub async fn cancel(self) -> Result<R, Status> {
        // Completion May Have Closed Input Before Cancellation Arrives.
        let _ = self.request_cancel().await;
        self.join().await
    }

    pub async fn next(&mut self) -> Result<Option<O>, Status> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.receive().await;
        if let Err(error) = &result {
            self.failure = Some(error.clone());
            self.response = None;
        }
        result
    }

    pub async fn join(mut self) -> Result<R, Status> {
        while self.next().await?.is_some() {}
        self.response.take().map(|(value, _permit)| value).ok_or_else(|| Status::data_loss("operation closed without a final response"))
    }

    async fn receive(&mut self) -> Result<Option<O>, Status> {
        if self.ended {
            return Ok(None);
        }
        while let Some(frame) = self.incoming.message().await? {
            if self.response.is_some() {
                return Err(Status::data_loss("operation sent a frame after its final response"));
            }
            match frame {
                Output::Item(item, _permit) if self.output => return Ok(Some(item)),
                Output::Item(..) => return Err(Status::data_loss("operation has no output stream")),
                Output::Response(response, permit) => self.response = Some((response, permit)),
            }
        }
        self.ended = true;
        if self.response.is_none() {
            return Err(Status::data_loss("operation closed without a final response"));
        }
        Ok(None)
    }

    async fn control(&self, tag: u32) -> Result<(), Status> {
        let frame = frame(&self.budget, self.settings, Some(tag), &()).await?;
        self.outgoing.send(frame).await.map_err(|_| Status::cancelled("operation input closed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn host_page() -> MessageDescriptor {
    harmos_protocol::host::CatalogEntriesListResponse::default().descriptor()
}

fn descriptors<Q, R, I, O>(method: &Method<Q, R, I, O>) -> Result<(MessageDescriptor, Option<MessageDescriptor>), Status> {
    let protobuf = method.contract.protobuf.as_ref().expect("validated contract");
    let pool = DescriptorPool::decode(protobuf.descriptors.as_slice()).map_err(|error| Status::invalid_argument(error.to_string()))?;
    let response = pool.get_message_by_name(&protobuf.response).ok_or_else(|| Status::invalid_argument("missing response descriptor"))?;
    let output = protobuf
        .output
        .as_ref()
        .map(|name| pool.get_message_by_name(name).ok_or_else(|| Status::invalid_argument("missing output descriptor")))
        .transpose()?;
    Ok((response, output))
}

async fn frame<T: Message>(budget: &Arc<Semaphore>, settings: ClientSettings, tag: Option<u32>, value: &T) -> Result<Frame, Status> {
    let payload = value.encoded_len();
    let overhead = tag.map_or(0, |tag| prost::encoding::key_len(tag) + prost::encoding::encoded_len_varint(payload as u64));
    let length = payload.checked_add(overhead).ok_or_else(|| Status::resource_exhausted("client frame length overflow"))?;
    if length > settings.message_bytes.get() || length > settings.input_bytes.get() {
        return Err(Status::resource_exhausted("client frame exceeds wire or input byte budget"));
    }
    let permit = budget.clone().acquire_many_owned(length as u32).await.map_err(|_| Status::cancelled("client input closed"))?;
    let mut bytes = Vec::with_capacity(length);
    if let Some(tag) = tag {
        prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, &mut bytes);
        prost::encoding::encode_varint(payload as u64, &mut bytes);
    }
    value.encode(&mut bytes).map_err(|error| Status::internal(error.to_string()))?;
    Ok(Frame { bytes, _permit: permit })
}

/* === Tests === */
