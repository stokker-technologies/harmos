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
use std::{any::TypeId, error::Error, future::Future, marker::PhantomData, mem::size_of, ops::Deref, sync::Arc, time::Duration};

// Third Party Imports
use harmos_domain::{__private::decoding_add, config::SchemaSettings};
use harmos_protocol::schema;
use prost::Message;
use prost_reflect::{Kind, MessageDescriptor, ReflectMessage, ServiceDescriptor};
use prost_types::FileDescriptorSet;
use tokio::{sync::watch::Receiver, time::Instant};

// Local Imports
#[cfg(not(target_arch = "wasm32"))]
use super::native::admit_wait;
use super::{NativeInstance, OperationFactory, PreparedNative, ServiceFuture};
use crate::execution::grpc::{DecodeInput, DecodeScalar, DecodeValue};
use crate::execution::operations::calls::OperationDependency;
use crate::execution::operations::response::ErasedResponse;
use crate::execution::operations::{
    Calls, Contract, ContractError, EndpointSet, ExecutionContext, FailureKind, Grpc, InputEndpoint, OperationDefinition, OperationPolicy,
    OutputEndpoint, ReplaySafety, RequestClaims, Resource, ResourceSet, Response, ServingError, StreamError, StreamInput, StreamItem,
    StreamOutput, Target, Visibility, WireEndpoints, WireStreamLimits, WireStreams,
};
use crate::execution::{self, Fingerprint, Identifier, MemorySize, MessageError};
use crate::operations::{self, Protobuf, ProtobufExt};
use crate::plugin::Grant;
use crate::reserved;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ProtobufDependency {
    definition: fn() -> Result<OperationDependency, ServingError>,
}

pub struct ProtobufInput<T>(StreamInput<Value<T>>);
pub struct ProtobufOutput<T>(StreamOutput<Encoded>, PhantomData<fn() -> T>);

struct Encoded(Vec<u8>);
pub struct ProtobufItem<T>(StreamItem<Value<T>>);

#[derive(Clone)]
struct Value<T> {
    value: T,
    memory: usize,
}

struct Instance<O: ProtobufOperation> {
    request: O::Request,
    endpoints: O::Endpoints,
    resources: ResourceSet,
    streams: Option<WireStreams>,
    epoch: u64,
    marker: PhantomData<O>,
}

/* === Enums === */

/* === Traits === */

/// A Host-Native Declaration Using Generated Protobuf Messages Directly.
pub trait ProtobufOperation: Send + Sync + 'static {
    type Request: ReflectMessage + Default + Clone + Send + Sync + 'static;
    type Response: ReflectMessage + Default + Send + Sync + 'static;
    type Endpoints: ProtobufEndpoints;

    const ID: &'static str;
    const GRPC: Option<Grpc> = None;
    const POLICY: OperationPolicy;
    const VISIBILITY: Visibility = Visibility::Protected;
    const DESCRIPTION: &'static str = "";
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[];
    const OPERATIONS: &'static [ProtobufDependency] = &[];

    /// Product Service Metadata, Including Its Declared Audience.
    fn service() -> Option<ServiceDescriptor> {
        let (service, _) = Self::ID.split_once('/')?;
        Self::Request::default().descriptor().parent_pool().get_service_by_name(service)
    }

    fn grant(_request: &Self::Request) -> Result<Option<Grant>, MessageError> {
        Ok(None)
    }

    /// A Clock Wait This Request Performs, Registered When the Call Is Admitted.
    fn admitted_wait(_request: &Self::Request) -> Option<Duration> {
        None
    }

    fn classify(_error: &(dyn Error + Send + Sync + 'static)) -> FailureKind {
        FailureKind::Permanent
    }

    fn run(
        context: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints,
    ) -> impl Future<Output = Result<Self::Response, ServingError>> + Send;
}

pub trait ProtobufHost: Sync {
    type Error: Send;
    fn call_protobuf<O: ProtobufOperation<Endpoints = ()>>(
        &self, request: O::Request,
    ) -> impl Future<Output = Result<O::Response, Self::Error>> + Send;
}

pub trait ProtobufExchangeEndpoints: ProtobufEndpoints {
    type Input: ReflectMessage + Default + Send + Sync + 'static;
    type Output: ReflectMessage + Default + Send + Sync + 'static;
}

pub trait ProtobufEndpoints: EndpointSet {
    fn descriptors() -> (Option<MessageDescriptor>, Option<MessageDescriptor>);
}

/* === Implementations === */

impl Target {
    pub fn protobuf<O: ProtobufOperation>() -> Result<Self, ServingError> {
        OperationFactory::protobuf::<O>()?;
        Ok(Self {
            in_process: false,
            id: O::ID,
            protobuf_native: true,
            definition: OperationFactory::protobuf::<O>()?.definition,
            request: TypeId::of::<O::Request>,
            response: TypeId::of::<O::Response>,
            memory: |request| {
                request
                    .downcast_ref::<O::Request>()
                    .map_or(0, |request| memory::<O::Request>(&request.encode_to_vec()).unwrap_or(usize::MAX))
            },
            encode: |request| request.downcast_ref::<O::Request>().map(Message::encode_to_vec),
            decode: decode_response::<O>,
            encode_protobuf: |request| {
                request
                    .downcast_ref::<O::Request>()
                    .map(Message::encode_to_vec)
                    .ok_or_else(|| MessageError::borrowed("callee payload differs"))
            },
            decode_protobuf: decode_response::<O>,
        })
    }
}
impl ExecutionContext {
    pub async fn call_protobuf<O: ProtobufOperation<Endpoints = ()>>(
        &self, request: O::Request,
    ) -> Result<Response<O::Response>, ServingError> {
        let target = Target::protobuf::<O>()?;
        let calls = self.resource::<Calls>().map_err(|_| ServingError::Catalog)?;
        let child = calls.0.start(target, Box::new(request), None, Box::new(()), self.call.clone()).await?;
        child.join().await?.restore().ok_or(ServingError::EndpointTypes)
    }
}
impl ProtobufDependency {
    pub const fn of<O: ProtobufOperation>() -> Self {
        Self {
            definition: || {
                let definition = OperationFactory::protobuf::<O>()?.definition;
                Ok(OperationDependency {
                    id: definition.id.to_string(),
                    fingerprint: definition.fingerprint,
                })
            },
        }
    }
}

impl<I, O> ProtobufExchangeEndpoints for (ProtobufInput<I>, ProtobufOutput<O>)
where
    I: ReflectMessage + Default + Send + Sync + 'static,
    O: ReflectMessage + Default + Send + Sync + 'static,
{
    type Input = I;
    type Output = O;
}

impl ProtobufEndpoints for () {
    fn descriptors() -> (Option<MessageDescriptor>, Option<MessageDescriptor>) {
        (None, None)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> ProtobufEndpoints for ProtobufInput<T> {
    fn descriptors() -> (Option<MessageDescriptor>, Option<MessageDescriptor>) {
        (Some(T::default().descriptor()), None)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> ProtobufEndpoints for ProtobufOutput<T> {
    fn descriptors() -> (Option<MessageDescriptor>, Option<MessageDescriptor>) {
        (None, Some(T::default().descriptor()))
    }
}

impl<I: ProtobufEndpoints, O: ProtobufEndpoints> ProtobufEndpoints for (I, O)
where
    (I, O): EndpointSet,
{
    fn descriptors() -> (Option<MessageDescriptor>, Option<MessageDescriptor>) {
        (I::descriptors().0, O::descriptors().1)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> EndpointSet for ProtobufInput<T> {
    fn bind_wire(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (endpoint, streams) = StreamInput::<Value<T>>::bind(limits)?;
        Ok((Self(endpoint), streams))
    }
    fn item_types() -> (Option<TypeId>, Option<TypeId>) {
        (Some(TypeId::of::<T>()), None)
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(&self.0)
    }
    fn progress(&self) -> Vec<Receiver<Option<Instant>>> {
        EndpointSet::progress(&self.0)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> EndpointSet for ProtobufOutput<T> {
    fn bind_wire(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (endpoint, streams) = StreamOutput::<Encoded>::bind(limits)?;
        Ok((Self(endpoint, PhantomData), streams))
    }
    fn item_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<T>()))
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(&self.0)
    }
    fn progress(&self) -> Vec<Receiver<Option<Instant>>> {
        EndpointSet::progress(&self.0)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> ProtobufInput<T> {
    pub async fn recv(&self) -> Option<ProtobufItem<T>> {
        self.0.recv().await.map(ProtobufItem)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> InputEndpoint for ProtobufInput<T> {
    type Item = T;
    fn bind_wire(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        <Self as EndpointSet>::bind_wire(limits)
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(self)
    }
    fn progress(&self) -> Vec<Receiver<Option<Instant>>> {
        EndpointSet::progress(self)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> OutputEndpoint for ProtobufOutput<T> {
    type Item = T;
    fn bind_wire(limits: WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        <Self as EndpointSet>::bind_wire(limits)
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(self)
    }
    fn progress(&self) -> Vec<Receiver<Option<Instant>>> {
        EndpointSet::progress(self)
    }
}

impl<T: Clone + Send + Sync + 'static> ProtobufItem<T> {
    pub fn into_owned_request(self) -> Result<(T, RequestClaims), StreamError> {
        let (value, claims) = self.0.into_owned_request()?;
        Ok((value.value, claims))
    }
}

impl<T> Deref for ProtobufItem<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.value
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> ProtobufOutput<T> {
    pub async fn requested_after(&self, after: u64) -> Result<u64, StreamError> {
        self.0.requested_after(after).await
    }

    pub async fn send(&self, value: T) -> Result<(), StreamError> {
        let memory = value.encoded_len().checked_add(size_of::<Encoded>()).ok_or(StreamError::Capacity)?;
        self.send_with(memory, || Ok(value)).await
    }

    pub async fn send_with(&self, memory: usize, make: impl FnOnce() -> Result<T, StreamError>) -> Result<(), StreamError> {
        self.0.send_with(memory, || Ok(Encoded(make()?.encode_to_vec()))).await
    }
}

impl MemorySize for Encoded {
    fn memory_size(&self) -> usize {
        size_of::<Self>().saturating_add(self.0.capacity())
    }
}

impl execution::Message for Encoded {
    const CODEC: &'static str = "harmos.protobuf.v1";
    fn encode(&self) -> Vec<u8> {
        self.0.clone()
    }
    fn encoded_size(&self) -> usize {
        self.0.len()
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        Ok(Self(bytes.to_vec()))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        decoding_add(size_of::<Self>(), bytes.len())
    }
}

impl<T> MemorySize for Value<T> {
    fn memory_size(&self) -> usize {
        self.memory
    }
}

impl<T: ReflectMessage + Default + Send + Sync> execution::Message for Value<T> {
    const CODEC: &'static str = "harmos.protobuf.v1";
    fn encode(&self) -> Vec<u8> {
        self.value.encode_to_vec()
    }
    fn encoded_size(&self) -> usize {
        self.value.encoded_len()
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        memory::<T>(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        let memory = memory::<T>(bytes)?;
        let value = T::decode(bytes).map_err(|error| MessageError::new(error.to_string()))?;
        Ok(Self { value, memory })
    }
}

impl OperationFactory {
    pub fn protobuf<O: ProtobufOperation>() -> Result<Self, ServingError> {
        let declared = contract::<O>(&SchemaSettings::default()).map_err(ServingError::Contract)?;
        Ok(Self {
            definition: declared.operation().clone(),
            operations: Arc::new(|| O::OPERATIONS.iter().map(|dependency| (dependency.definition)()).collect()),
            declared: Arc::new(|| contract::<O>(&SchemaSettings::default()).expect("validated protobuf declaration").operation().clone()),
            named: Arc::new(|id| {
                contract::<O>(&SchemaSettings::default())
                    .and_then(|contract| contract.with_id(id))
                    .map(|contract| contract.operation().clone())
                    .map_err(ServingError::Contract)
            }),
            registered_id: None,
            grant: Arc::new(|_, grant| grant),
            request_grant: Arc::new(|request| request.downcast_ref::<O::Request>().map_or(Ok(None), O::grant)),
            builtin: reserved(O::ID),
            resources: O::RESOURCES,
            contract: Arc::new(contract::<O>),
            native: Arc::new(|_definition, resources, request, endpoints| {
                let request = *request.downcast::<O::Request>().map_err(|_| ServingError::EndpointTypes)?;
                let endpoints = *endpoints.downcast::<O::Endpoints>().map_err(|_| ServingError::EndpointTypes)?;
                Ok(Box::new(Instance::<O> {
                    request,
                    endpoints,
                    resources,
                    streams: None,
                    epoch: 0,
                    marker: PhantomData,
                }))
            }),
            endpoints_valid: || {
                let (input, output) = O::Endpoints::descriptors();
                let types = O::Endpoints::item_types();
                input.is_some() == types.0.is_some() && output.is_some() == types.1.is_some()
            },
            native_types: || (TypeId::of::<O::Request>(), TypeId::of::<O::Response>(), TypeId::of::<O::Endpoints>()),
            encoded: Arc::new(|_definition, resources, payload, _, limits| {
                let (endpoints, streams) = match O::Endpoints::empty() {
                    Some(endpoints) => (endpoints, None),
                    None => {
                        let (endpoints, streams) = O::Endpoints::bind_wire(limits.ok_or(ServingError::StreamLimits)?)?;
                        (endpoints, Some(streams))
                    }
                };
                memory::<O::Request>(payload).map_err(ServingError::Codec)?;
                let request = O::Request::decode(payload).map_err(|error| ServingError::Codec(MessageError::new(error.to_string())))?;
                let grant = O::grant(&request).map_err(ServingError::Codec)?;
                Ok(PreparedNative {
                    instance: Box::new(Instance::<O> {
                        request,
                        endpoints,
                        resources,
                        streams: streams.clone(),
                        epoch: 0,
                        marker: PhantomData,
                    }),
                    grant,
                    streams,
                })
            }),
            grpc_memory: memory::<O::Request>,
            decoding_memory: memory::<O::Request>,
        })
    }
}

impl<O: ProtobufOperation> NativeInstance for Instance<O> {
    fn admit(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        admit_wait(O::admitted_wait(&self.request), &self.resources);
    }
    fn classify(&self, error: &(dyn Error + Send + Sync + 'static)) -> FailureKind {
        O::classify(error)
    }
    fn initialize(&mut self, _: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ErasedResponse> {
        Box::pin(async move {
            let attempt = if let Some(streams) = &self.streams {
                self.epoch = self.epoch.checked_add(1).ok_or(ServingError::StreamPhase)?;
                Some(streams.start(self.epoch)?)
            } else {
                None
            };
            let context = context.with_resources(self.resources.clone());
            let response = O::run(context, self.request.clone(), &self.endpoints).await?;
            if let Some(attempt) = attempt {
                attempt.end();
            }
            Ok(Response::new(response.encode_to_vec()).into_erased())
        })
    }
    fn run_stream(&mut self, context: ExecutionContext, epoch: u64) -> ServiceFuture<'_, ErasedResponse> {
        if self.streams.is_none() || self.epoch.checked_add(1) != Some(epoch) {
            return Box::pin(async { Err(ServingError::StreamPhase) });
        }
        self.run(context)
    }
    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn finalize(&mut self) -> ServiceFuture<'_, ()> {
        if let Some(streams) = &self.streams {
            streams.complete();
        }
        Box::pin(async { Ok(()) })
    }
    fn transferred(&self) -> bool {
        self.endpoints.transferred()
    }
    fn progress(&self) -> Vec<Receiver<Option<Instant>>> {
        self.endpoints.progress()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn decode_response<O: ProtobufOperation>(bytes: &[u8]) -> Result<ErasedResponse, MessageError> {
    let response = O::Response::decode(bytes).map_err(|error| MessageError::new(error.to_string()))?;
    Ok(Response::new(response).into_erased())
}

fn contract<O: ProtobufOperation>(settings: &SchemaSettings) -> Result<Contract, ContractError> {
    let request = O::Request::default().descriptor();
    let response = O::Response::default().descriptor();
    let (input, output) = O::Endpoints::descriptors();
    let mut descriptors = FileDescriptorSet::default();
    for message in [Some(&request), Some(&response), input.as_ref(), output.as_ref()].into_iter().flatten() {
        let types = FileDescriptorSet {
            file: message.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        let projected = schema::message_types(&types, message.full_name()).map_err(|error| ContractError::Protobuf(error.to_string()))?;
        schema::merge_message_types(&mut descriptors, projected).map_err(|error| ContractError::Protobuf(error.to_string()))?;
    }
    let protobuf = Protobuf {
        descriptors: descriptors.encode_to_vec(),
        request: request.full_name().into(),
        response: response.full_name().into(),
        input: input.map(|message| message.full_name().into()),
        output: output.map(|message| message.full_name().into()),
    };
    let operation = OperationDefinition {
        id: Identifier::new(O::ID).map_err(operations::Error::from)?,
        description: O::DESCRIPTION.into(),
        policy: O::POLICY,
        visibility: O::VISIBILITY,
        replay: O::REPLAY,
        fingerprint: Fingerprint([0; 32]),
    };
    if matches!(O::ID, "harmos.host.v1.Time/Sleep" | "harmos.host.v1.Time/Get") {
        let clock = Contract::clock(O::ID == "harmos.host.v1.Time/Sleep", O::POLICY, settings)?;
        let same_fingerprint = protobuf.fingerprint(O::ID)? == clock.fingerprint();
        let same_mapping = clock.grpc() == O::GRPC.as_ref();
        if !same_fingerprint || !same_mapping || operation.visibility != clock.operation().visibility {
            return Err(ContractError::Protobuf("clock declaration differs from the protocol-owned contract".into()));
        }
        return Ok(clock);
    }
    let contract = Contract::from_protobuf(operation, protobuf, O::GRPC, settings)?;
    if O::ID.starts_with("harmos.host.v1.") {
        return Ok(contract);
    }
    let Some((_, method)) = O::ID.split_once('/') else {
        return Ok(contract);
    };
    let service = O::service()
        .ok_or_else(|| ContractError::Protobuf(format!("Declare {}: operation service and callers option are missing", O::ID)))?;
    let method = service.methods().find(|candidate| candidate.name() == method).ok_or(ContractError::Grpc)?;
    let callers = schema::method_callers(&method).map_err(|error| ContractError::Protobuf(error.to_string()))?;
    Ok(contract.with_callers(callers))
}

pub(crate) fn memory<T: ReflectMessage + Default>(bytes: &[u8]) -> Result<usize, MessageError> {
    let descriptor = T::default().descriptor();
    let mut path = [const { None }; 32];
    let (_, largest) = layout(&descriptor, &mut path, 0)?;
    let size = measure(&descriptor, DecodeInput::new(bytes), largest)?;
    // Retain the Original Request and Its Attempt Clone Together.
    size.checked_mul(2)
        .and_then(|size| size.checked_add(size_of::<Value<T>>()))
        .ok_or_else(|| MessageError::borrowed("protobuf decoded allocation overflows"))
}

fn layout(
    descriptor: &MessageDescriptor, path: &mut [Option<MessageDescriptor>; 32], depth: usize,
) -> Result<(usize, usize), MessageError> {
    if path[..depth].iter().flatten().any(|parent| parent == descriptor) {
        return Ok((16, 16));
    }
    if depth == path.len() {
        return Err(MessageError::borrowed("protobuf descriptor nesting exceeds 32 levels"));
    }
    path[depth] = Some(descriptor.clone());
    let mut size = 32usize;
    let mut largest = 0;
    for field in descriptor.fields() {
        let field_size = if let Kind::Message(message) = field.kind() {
            let (inline, nested) = layout(&message, path, depth + 1)?;
            largest = largest.max(nested);
            if field.is_list() || field.is_map() {
                32
            } else {
                inline.checked_add(16).ok_or_else(|| MessageError::borrowed("protobuf inline size overflows"))?
            }
        } else {
            32
        };
        size = size.checked_add(field_size).ok_or_else(|| MessageError::borrowed("protobuf inline size overflows"))?;
    }
    path[depth] = None;
    Ok((size, largest.max(size)))
}

// Count Allocation from Fields Without Constructing a Generated Message.
fn measure(descriptor: &MessageDescriptor, input: DecodeInput<'_>, largest: usize) -> Result<usize, MessageError> {
    let mut memory = largest;
    input.fields(&mut |field| {
        let Some(definition) = descriptor.get_field(field.tag) else { return Ok(()) };
        let mut extra = 0;
        let scalar = match definition.kind() {
            Kind::Message(message) => {
                extra = measure(&message, input.child(field.bytes()?)?, largest)?;
                if definition.is_list() || definition.is_map() {
                    // Include Vector Growth or Hash-Table Nodes and Control Bytes.
                    extra = decoding_add(
                        extra,
                        largest.checked_mul(4).ok_or_else(|| MessageError::borrowed("protobuf allocation overflows"))?,
                    )?;
                    extra = decoding_add(extra, 128)?;
                }
                memory = decoding_add(memory, extra)?;
                return Ok(());
            }
            Kind::String | Kind::Bytes => {
                let length = if matches!(definition.kind(), Kind::String) { field.text()?.len() } else { field.bytes()?.len() };
                extra = length.checked_mul(2).ok_or_else(|| MessageError::borrowed("protobuf allocation overflows"))?;
                if definition.is_list() {
                    extra = decoding_add(extra, 4 * size_of::<String>())?;
                }
                memory = decoding_add(memory, extra)?;
                return Ok(());
            }
            Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => DecodeScalar::Bits64,
            Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => DecodeScalar::Bits32,
            _ => DecodeScalar::Varint,
        };
        if definition.is_list() {
            field.scalars(scalar, &mut |_| {
                extra = decoding_add(extra, 4 * size_of::<u64>())?;
                Ok(())
            })?;
        } else {
            if matches!(field.value, DecodeValue::Bytes(_)) {
                return Err(MessageError::borrowed("protobuf scalar wire type mismatch"));
            }
            field.scalar(scalar)?;
        }
        memory = decoding_add(memory, extra)?;
        Ok(())
    })?;
    Ok(memory)
}

/* === Tests === */
