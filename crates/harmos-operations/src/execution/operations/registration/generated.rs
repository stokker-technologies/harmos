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
use std::collections::BTreeMap;
use std::{any::TypeId, future::Future, marker::PhantomData, ops::Deref};

// Third Party Imports
use prost::Message as _;
use prost_reflect::{MessageDescriptor, ReflectMessage};

// Local Imports
use super::*;
use crate::execution::operations::{
    EndpointSet, Grpc, OperationPolicy, ReplaySafety, StreamInput, StreamItem, StreamOutput, Visibility, WireEndpoints, WireStreams,
};
use crate::execution::{MemorySize, MessageError};
use harmos_operations::operations::ProtobufExt;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ProtobufDependency {
    definition: fn() -> Result<super::super::calls::OperationDependency, ServingError>,
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
    const VISIBILITY: Visibility = Visibility::Private;
    const DESCRIPTION: &'static str = "";
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[];
    const OPERATIONS: &'static [ProtobufDependency] = &[];

    fn grant(_request: &Self::Request) -> Result<Option<harmos_operations::plugin::Grant>, MessageError> {
        Ok(None)
    }

    fn classify(_error: &(dyn std::error::Error + Send + Sync + 'static)) -> super::super::FailureKind {
        super::super::FailureKind::Permanent
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

impl super::super::Target {
    pub fn protobuf<O: ProtobufOperation>() -> Result<Self, ServingError> {
        OperationFactory::protobuf::<O>()?;
        Ok(Self {
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
            encode: |request| request.downcast_ref::<O::Request>().map(prost::Message::encode_to_vec),
            decode: decode_response::<O>,
            encode_protobuf: |request| {
                request
                    .downcast_ref::<O::Request>()
                    .map(prost::Message::encode_to_vec)
                    .ok_or_else(|| MessageError::borrowed("callee payload differs"))
            },
            decode_protobuf: decode_response::<O>,
        })
    }
}
impl ExecutionContext {
    pub async fn call_protobuf<O: ProtobufOperation<Endpoints = ()>>(
        &self, request: O::Request,
    ) -> Result<super::super::Response<O::Response>, ServingError> {
        let target = super::super::Target::protobuf::<O>()?;
        let calls = self.resource::<super::super::Calls>().map_err(|_| ServingError::Catalog)?;
        let child = calls.0.start(target, Box::new(request), None, Box::new(()), self.call.clone()).await?;
        child.join().await?.restore().ok_or(ServingError::EndpointTypes)
    }
}
impl From<super::super::InvocationError> for ServingError {
    fn from(error: super::super::InvocationError) -> Self {
        Self::Native(std::sync::Arc::new(error))
    }
}

impl ProtobufDependency {
    pub const fn of<O: ProtobufOperation>() -> Self {
        Self {
            definition: || {
                let definition = OperationFactory::protobuf::<O>()?.definition;
                Ok(super::super::calls::OperationDependency {
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
    fn bind_wire(limits: super::super::WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (endpoint, streams) = StreamInput::<Value<T>>::bind(limits)?;
        Ok((Self(endpoint), streams))
    }
    fn item_types() -> (Option<TypeId>, Option<TypeId>) {
        (Some(TypeId::of::<T>()), None)
    }
    fn transferred(&self) -> bool {
        self.0.transferred()
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        self.0.progress()
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> EndpointSet for ProtobufOutput<T> {
    fn bind_wire(limits: super::super::WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        let (endpoint, streams) = StreamOutput::<Encoded>::bind(limits)?;
        Ok((Self(endpoint, PhantomData), streams))
    }
    fn item_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<T>()))
    }
    fn transferred(&self) -> bool {
        self.0.transferred()
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        self.0.progress()
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> ProtobufInput<T> {
    pub async fn recv(&self) -> Option<ProtobufItem<T>> {
        self.0.recv().await.map(ProtobufItem)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> super::super::InputEndpoint for ProtobufInput<T> {
    type Item = T;
    fn bind_wire(limits: super::super::WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        <Self as EndpointSet>::bind_wire(limits)
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(self)
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        EndpointSet::progress(self)
    }
}

impl<T: ReflectMessage + Default + Send + Sync + 'static> super::super::OutputEndpoint for ProtobufOutput<T> {
    type Item = T;
    fn bind_wire(limits: super::super::WireStreamLimits) -> Result<(Self, WireStreams), ServingError> {
        <Self as EndpointSet>::bind_wire(limits)
    }
    fn transferred(&self) -> bool {
        EndpointSet::transferred(self)
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        EndpointSet::progress(self)
    }
}

impl<T: Clone + Send + Sync + 'static> ProtobufItem<T> {
    pub fn into_owned_request(self) -> Result<(T, super::super::RequestClaims), super::super::StreamError> {
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
    pub async fn send(&self, value: T) -> Result<(), super::super::StreamError> {
        let memory = value.encoded_len().checked_add(std::mem::size_of::<Encoded>()).ok_or(super::super::StreamError::Capacity)?;
        self.send_with(memory, || Ok(value)).await
    }

    pub async fn send_with(
        &self, memory: usize, make: impl FnOnce() -> Result<T, super::super::StreamError>,
    ) -> Result<(), super::super::StreamError> {
        self.0.send_with(memory, || Ok(Encoded(make()?.encode_to_vec()))).await
    }
}

impl MemorySize for Encoded {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.0.capacity())
    }
}

impl crate::execution::Message for Encoded {
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
        harmos_domain::__private::decoding_add(std::mem::size_of::<Self>(), bytes.len())
    }
}

impl<T> MemorySize for Value<T> {
    fn memory_size(&self) -> usize {
        self.memory
    }
}

impl<T: ReflectMessage + Default + Send + Sync> crate::execution::Message for Value<T> {
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
        let declared = contract::<O>(&harmos_domain::config::SchemaSettings::default()).map_err(ServingError::Contract)?;
        Ok(Self {
            definition: declared.operation().clone(),
            operations: std::sync::Arc::new(|| O::OPERATIONS.iter().map(|dependency| (dependency.definition)()).collect()),
            declared: std::sync::Arc::new(|| {
                contract::<O>(&harmos_domain::config::SchemaSettings::default())
                    .expect("validated protobuf declaration")
                    .operation()
                    .clone()
            }),
            named: std::sync::Arc::new(|id| {
                contract::<O>(&harmos_domain::config::SchemaSettings::default())
                    .and_then(|contract| contract.with_id(id))
                    .map(|contract| contract.operation().clone())
                    .map_err(ServingError::Contract)
            }),
            registered_id: None,
            grant: std::sync::Arc::new(|_, grant| grant),
            request_grant: std::sync::Arc::new(|request| request.downcast_ref::<O::Request>().map_or(Ok(None), O::grant)),
            builtin: matches!(O::ID, "harmos.clock.sleep" | "harmos.clock.now"),
            resources: O::RESOURCES,
            contract: std::sync::Arc::new(contract::<O>),
            native: std::sync::Arc::new(|_definition, resources, request, endpoints| {
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
            encoded: std::sync::Arc::new(|_definition, resources, payload, _, limits| {
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
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> super::super::FailureKind {
        O::classify(error)
    }
    fn initialize(&mut self, _: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, super::super::response::ErasedResponse> {
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
            Ok(super::super::Response::new(response.encode_to_vec()).into_erased())
        })
    }
    fn run_stream(&mut self, context: ExecutionContext, epoch: u64) -> ServiceFuture<'_, super::super::response::ErasedResponse> {
        if self.streams.is_none() || self.epoch.checked_add(1) != Some(epoch) {
            return Box::pin(async { Err(ServingError::StreamPhase) });
        }
        self.run(context)
    }
    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        if let Some(streams) = &self.streams {
            streams.complete();
        }
        Box::pin(async { Ok(()) })
    }
    fn transferred(&self) -> bool {
        self.endpoints.transferred()
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        self.endpoints.progress()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn decode_response<O: ProtobufOperation>(bytes: &[u8]) -> Result<super::super::response::ErasedResponse, MessageError> {
    let response = O::Response::decode(bytes).map_err(|error| MessageError::new(error.to_string()))?;
    Ok(super::super::Response::new(response).into_erased())
}

fn contract<O: ProtobufOperation>(settings: &harmos_domain::config::SchemaSettings) -> Result<Contract, super::super::ContractError> {
    let request = O::Request::default().descriptor();
    let response = O::Response::default().descriptor();
    let (input, output) = O::Endpoints::descriptors();
    let mut files = BTreeMap::new();
    for message in [Some(&request), Some(&response), input.as_ref(), output.as_ref()].into_iter().flatten() {
        for file in message.parent_pool().files() {
            let file = file.file_descriptor_proto().clone();
            if let Some(previous) = files.insert(file.name.clone(), file.clone())
                && previous != file
            {
                return Err(super::super::ContractError::Protobuf("conflicting descriptor files".into()));
            }
        }
    }
    let roots: Vec<_> = [Some(&request), Some(&response), input.as_ref(), output.as_ref()]
        .into_iter()
        .flatten()
        .map(|message| message.full_name())
        .collect();
    let descriptors =
        harmos_protocol::schema::operation_types(&prost_types::FileDescriptorSet { file: files.into_values().collect() }, &roots)
            .map_err(|error| super::super::ContractError::Protobuf(error.to_string()))?;
    let protobuf = harmos_operations::operations::Protobuf {
        descriptors: descriptors.encode_to_vec(),
        request: request.full_name().into(),
        response: response.full_name().into(),
        input: input.map(|message| message.full_name().into()),
        output: output.map(|message| message.full_name().into()),
    };
    let operation = OperationDefinition {
        id: crate::execution::Identifier::new(O::ID).map_err(crate::operations::Error::from)?,
        description: O::DESCRIPTION.into(),
        policy: O::POLICY,
        visibility: O::VISIBILITY,
        replay: O::REPLAY,
        fingerprint: crate::execution::Fingerprint([0; 32]),
    };
    if matches!(O::ID, "harmos.clock.sleep" | "harmos.clock.now") {
        let clock = Contract::clock(O::ID == "harmos.clock.sleep", O::POLICY, settings)?;
        let same_fingerprint = protobuf.fingerprint(O::ID)? == clock.fingerprint();
        let same_mapping = clock.grpc() == O::GRPC.as_ref();
        if !same_fingerprint || !same_mapping || operation.visibility != clock.operation().visibility {
            return Err(super::super::ContractError::Protobuf("clock declaration differs from the protocol-owned contract".into()));
        }
        return Ok(clock);
    }
    Contract::from_protobuf(operation, protobuf, O::GRPC, settings)
}

pub(crate) fn memory<T: ReflectMessage + Default>(bytes: &[u8]) -> Result<usize, MessageError> {
    let descriptor = T::default().descriptor();
    let mut path = [const { None }; 32];
    let (_, largest) = layout(&descriptor, &mut path, 0)?;
    let size = measure(&descriptor, crate::execution::grpc::DecodeInput::new(bytes), largest)?;
    // Retain the Original Request and Its Attempt Clone Together.
    size.checked_mul(2)
        .and_then(|size| size.checked_add(std::mem::size_of::<Value<T>>()))
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
        let field_size = if let prost_reflect::Kind::Message(message) = field.kind() {
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
fn measure(descriptor: &MessageDescriptor, input: crate::execution::grpc::DecodeInput<'_>, largest: usize) -> Result<usize, MessageError> {
    use crate::execution::grpc::{DecodeScalar, DecodeValue};
    use harmos_domain::__private::decoding_add as add;
    use prost_reflect::Kind;
    let mut memory = largest;
    input.fields(&mut |field| {
        let Some(definition) = descriptor.get_field(field.tag) else { return Ok(()) };
        let mut extra = 0;
        let scalar = match definition.kind() {
            Kind::Message(message) => {
                extra = measure(&message, input.child(field.bytes()?)?, largest)?;
                if definition.is_list() || definition.is_map() {
                    // Include Vector Growth or Hash-Table Nodes and Control Bytes.
                    extra = add(extra, largest.checked_mul(4).ok_or_else(|| MessageError::borrowed("protobuf allocation overflows"))?)?;
                    extra = add(extra, 128)?;
                }
                memory = add(memory, extra)?;
                return Ok(());
            }
            Kind::String | Kind::Bytes => {
                let length = if matches!(definition.kind(), Kind::String) { field.text()?.len() } else { field.bytes()?.len() };
                extra = length.checked_mul(2).ok_or_else(|| MessageError::borrowed("protobuf allocation overflows"))?;
                if definition.is_list() {
                    extra = add(extra, 4 * std::mem::size_of::<String>())?;
                }
                memory = add(memory, extra)?;
                return Ok(());
            }
            Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => DecodeScalar::Bits64,
            Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => DecodeScalar::Bits32,
            _ => DecodeScalar::Varint,
        };
        if definition.is_list() {
            field.scalars(scalar, &mut |_| {
                extra = add(extra, 4 * std::mem::size_of::<u64>())?;
                Ok(())
            })?;
        } else {
            if matches!(field.value, DecodeValue::Bytes(_)) {
                return Err(MessageError::borrowed("protobuf scalar wire type mismatch"));
            }
            field.scalar(scalar)?;
        }
        memory = add(memory, extra)?;
        Ok(())
    })?;
    Ok(memory)
}

/* === Tests === */
