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
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::Arc;
// Third Party Imports
use bytes::{Buf, BufMut};
use prost_reflect::{DynamicMessage, Kind, MessageDescriptor, MethodDescriptor, ReflectMessage, Value};
use tonic::Status;
use tonic::codec::{BufferSettings, DecodeBuf, EncodeBuf};
// Local Imports
use crate::operations::Contract;
use harmos_sdk::grpc::{DecodeInput, Validator as ProtobufValidator, prost};
use prost::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct Operation {
    binding: Arc<Binding>,
}

pub struct Payload {
    binding: Arc<Binding>,
    bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct Codec {
    binding: Arc<Binding>,
}

pub struct Encoder {
    binding: Arc<Binding>,
}

pub struct Decoder {
    binding: Arc<Binding>,
}

struct Binding {
    path: String,
    method: MethodDescriptor,
    request: MessageDescriptor,
    response: MessageDescriptor,
    input: Option<MessageDescriptor>,
    output: Option<MessageDescriptor>,
    request_validator: ProtobufValidator,
    response_validator: ProtobufValidator,
    input_validator: Option<ProtobufValidator>,
    output_validator: Option<ProtobufValidator>,
    limit: usize,
}

struct Json {
    bytes: Vec<u8>,
    limit: usize,
}

/* === Enums === */

#[derive(Debug, PartialEq)]
pub enum Event {
    Item(serde_json::Value),
    Completion(serde_json::Value),
}

/* === Traits === */

/* === Implementations === */

impl Operation {
    pub fn new(contract: &Contract, message_bytes: NonZeroUsize) -> Result<Self, Status> {
        let grpc = contract.grpc().ok_or_else(|| invalid("operation has no gRPC declaration"))?;
        let limit = message_bytes.get();
        if limit > u32::MAX as usize {
            return Err(invalid("dynamic message limit exceeds gRPC capacity"));
        }
        let service =
            super::descriptor(&grpc.package, &grpc.service, &[(&grpc.method, contract)]).map_err(|error| invalid(error.to_string()))?;
        let method = service.methods().next().ok_or_else(|| invalid("missing operation method"))?;
        let request = direction(method.input(), "request", contract.input().is_some())?;
        let response = direction(method.output(), "response", contract.output().is_some())?;
        let input = contract.input().map(|_| direction(method.input(), "item", true)).transpose()?;
        let output = contract.output().map(|_| direction(method.output(), "item", true)).transpose()?;
        let validators =
            ProtobufValidator::from_schema(contract.schema(), &Default::default()).map_err(|error| invalid(error.to_string()))?;
        let request_validator = validators.at(contract.request().index()).map_err(|error| invalid(error.to_string()))?;
        let response_validator = validators.at(contract.response().index()).map_err(|error| invalid(error.to_string()))?;
        let input_validator =
            contract.input().map(|view| validators.at(view.index())).transpose().map_err(|error| invalid(error.to_string()))?;
        let output_validator =
            contract.output().map(|view| validators.at(view.index())).transpose().map_err(|error| invalid(error.to_string()))?;
        Ok(Self {
            binding: Arc::new(Binding {
                path: grpc.path(),
                method,
                request,
                response,
                input,
                output,
                request_validator,
                response_validator,
                input_validator,
                output_validator,
                limit,
            }),
        })
    }

    pub fn path(&self) -> &str {
        &self.binding.path
    }

    pub fn client_streaming(&self) -> bool {
        self.binding.input.is_some()
    }

    pub fn server_streaming(&self) -> bool {
        self.binding.output.is_some()
    }

    pub fn request(&self, json: &[u8]) -> Result<Payload, Status> {
        self.payload(json, false)
    }

    pub fn input(&self, json: &[u8]) -> Result<Payload, Status> {
        self.payload(json, true)
    }

    pub fn codec(&self) -> Codec {
        Codec { binding: Arc::clone(&self.binding) }
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<Event, Status> {
        self.binding.decode(bytes)
    }

    #[tracing::instrument(target = "harmos::grpc", name = "grpc.dynamic.encode", level = "trace", skip_all, fields(json_bytes = json.len(), item))]
    fn payload(&self, json: &[u8], item: bool) -> Result<Payload, Status> {
        self.binding.check(json.len())?;
        let (descriptor, validator) = if item {
            (
                self.binding.input.clone().ok_or_else(|| invalid("operation has no input stream"))?,
                self.binding.input_validator.as_ref().ok_or_else(|| invalid("operation has no input validator"))?,
            )
        } else {
            (self.binding.request.clone(), &self.binding.request_validator)
        };
        let mut deserializer = serde_json::Deserializer::from_slice(json);
        let message = DynamicMessage::deserialize(descriptor, &mut deserializer).map_err(|error| invalid(error.to_string()))?;
        deserializer.end().map_err(|error| invalid(error.to_string()))?;
        let length = message.encoded_len();
        self.binding.check(length)?;
        let mut header = [0_u8; 16];
        let prefix = if self.client_streaming() {
            let name = if item { "item" } else { "request" };
            let field = self.binding.method.input().get_field_by_name(name).ok_or_else(|| invalid("missing input envelope descriptor"))?;
            let mut remaining = &mut header[..];
            prost::encoding::encode_key(field.number(), prost::encoding::WireType::LengthDelimited, &mut remaining);
            prost::encoding::encode_varint(length as u64, &mut remaining);
            16 - remaining.len()
        } else {
            0
        };
        let total = prefix.checked_add(length).ok_or_else(|| invalid("dynamic envelope size overflow"))?;
        self.binding.check(total)?;
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(&header[..prefix]);
        message.encode(&mut bytes).map_err(|error| invalid(error.to_string()))?;
        validator.validate(&bytes[prefix..]).map_err(|error| invalid(error.to_string()))?;
        Ok(Payload {
            binding: Arc::clone(&self.binding),
            bytes,
        })
    }
}

impl Payload {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Binding {
    fn check(&self, length: usize) -> Result<(), Status> {
        if length > self.limit {
            return Err(Status::resource_exhausted("dynamic message exceeds byte ceiling"));
        }
        Ok(())
    }

    #[tracing::instrument(target = "harmos::grpc", name = "grpc.dynamic.decode", level = "trace", skip_all, fields(wire_bytes = bytes.len()))]
    fn decode(&self, bytes: &[u8]) -> Result<Event, Status> {
        self.check(bytes.len())?;
        if let Some(output_validator) = &self.output_validator {
            // Validate Each Wire Fragment Before The Reflection Decoder Allocates It.
            DecodeInput::new(bytes)
                .fields(&mut |field| {
                    let validator = match field.tag {
                        1 => output_validator,
                        2 => &self.response_validator,
                        _ => return Ok(()),
                    };
                    validator.validate(field.bytes()?).map_err(|error| crate::MessageError::new(error.to_string()))
                })
                .map_err(|error| invalid(error.to_string()))?;
            let envelope = DynamicMessage::decode(self.method.output(), bytes).map_err(|error| invalid(error.to_string()))?;
            let item = envelope.descriptor().get_field_by_name("item").ok_or_else(|| invalid("missing item descriptor"))?;
            let completion = envelope.descriptor().get_field_by_name("response").ok_or_else(|| invalid("missing completion descriptor"))?;
            let (field, is_item) = if envelope.has_field(&item) {
                (item, true)
            } else if envelope.has_field(&completion) {
                (completion, false)
            } else {
                return Err(invalid("empty output envelope"));
            };
            let value = envelope.get_field(&field);
            let Value::Message(message) = value.as_ref() else {
                return Err(invalid("invalid output envelope"));
            };
            let json = self.json(message)?;
            return Ok(if is_item { Event::Item(json) } else { Event::Completion(json) });
        }
        self.response_validator.validate(bytes).map_err(|error| invalid(error.to_string()))?;
        let message = DynamicMessage::decode(self.response.clone(), bytes).map_err(|error| invalid(error.to_string()))?;
        self.json(&message).map(Event::Completion)
    }

    fn json(&self, message: &DynamicMessage) -> Result<serde_json::Value, Status> {
        let mut writer = Json { bytes: Vec::new(), limit: self.limit };
        serde_json::to_writer(&mut writer, message).map_err(|error| {
            if error.is_io() { Status::resource_exhausted("dynamic JSON exceeds byte ceiling") } else { invalid(error.to_string()) }
        })?;
        serde_json::from_slice(&writer.bytes).map_err(|error| invalid(error.to_string()))
    }
}

impl Write for Json {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let length = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| std::io::Error::other("JSON size overflow"))?;
        if length > self.limit {
            return Err(std::io::Error::other("JSON byte ceiling"));
        }
        self.bytes.try_reserve(bytes.len()).map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl tonic::codec::Codec for Codec {
    type Encode = Payload;
    type Decode = Event;
    type Encoder = Encoder;
    type Decoder = Decoder;
    fn encoder(&mut self) -> Encoder {
        Encoder { binding: Arc::clone(&self.binding) }
    }
    fn decoder(&mut self) -> Decoder {
        Decoder { binding: Arc::clone(&self.binding) }
    }
}

impl tonic::codec::Encoder for Encoder {
    type Item = Payload;
    type Error = Status;
    fn encode(&mut self, item: Payload, destination: &mut EncodeBuf<'_>) -> Result<(), Status> {
        if !Arc::ptr_eq(&self.binding, &item.binding) {
            return Err(invalid("dynamic payload belongs to another operation"));
        }
        self.binding.check(item.bytes.len())?;
        destination.put_slice(&item.bytes);
        Ok(())
    }
    fn buffer_settings(&self) -> BufferSettings {
        BufferSettings::new(4096.min(self.binding.limit), 1)
    }
}

impl tonic::codec::Decoder for Decoder {
    type Item = Event;
    type Error = Status;
    fn decode(&mut self, source: &mut DecodeBuf<'_>) -> Result<Option<Event>, Status> {
        self.binding.check(source.remaining())?;
        let bytes = source.copy_to_bytes(source.remaining());
        self.binding.decode(&bytes).map(Some)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn direction(message: MessageDescriptor, field: &str, envelope: bool) -> Result<MessageDescriptor, Status> {
    if !envelope {
        return Ok(message);
    }
    let field = message.get_field_by_name(field).ok_or_else(|| invalid("missing stream direction"))?;
    match field.kind() {
        Kind::Message(message) => Ok(message),
        _ => Err(invalid("stream direction is not a message")),
    }
}

fn invalid(message: impl Into<String>) -> Status {
    Status::invalid_argument(message.into())
}

/* === Tests === */
