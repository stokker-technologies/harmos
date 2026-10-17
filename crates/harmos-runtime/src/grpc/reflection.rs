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
use std::{collections::BTreeSet, pin::Pin};

// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;
use prost_reflect::{DescriptorPool, FileDescriptor, prost::Message as _};
use tonic::{Request, Response, Status, Streaming};
use tonic_reflection::pb::v1::{
    self as pb,
    server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse,
    server_reflection_server::{ServerReflection, ServerReflectionServer},
};

// Local Imports
use crate::operations::OperationManager;

/* === Constants === */
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub(crate) struct Reflection {
    operations: OperationManager,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Reflection {
    pub(crate) fn service(operations: OperationManager) -> ServerReflectionServer<Self> {
        ServerReflectionServer::new(Self { operations })
            .max_decoding_message_size(MAX_REQUEST_BYTES)
            .max_encoding_message_size(MAX_RESPONSE_BYTES)
    }

    fn respond(&self, request: pb::ServerReflectionRequest) -> pb::ServerReflectionResponse {
        let response = self.snapshot().and_then(|pool| answer(&pool, request.message_request.as_ref()));
        pb::ServerReflectionResponse {
            valid_host: request.host.clone(),
            original_request: Some(request),
            message_response: Some(response.unwrap_or_else(|error| {
                MessageResponse::ErrorResponse(pb::ErrorResponse {
                    error_code: error.code() as i32,
                    error_message: error.message().to_owned(),
                })
            })),
        }
    }

    fn snapshot(&self) -> Result<DescriptorPool, Status> {
        let methods = self.operations.grpc_methods();
        let artifacts = harmos_packaging::proto::Artifacts::new(methods.iter().map(|method| &method.contract))
            .map_err(|_| Status::internal("operation descriptor snapshot failed"))?;
        let mut pool =
            DescriptorPool::decode(artifacts.descriptors()).map_err(|_| Status::internal("operation descriptors are invalid"))?;
        pool.decode_file_descriptor_set(pb::FILE_DESCRIPTOR_SET).map_err(|_| Status::internal("reflection descriptors are invalid"))?;
        Ok(pool)
    }
}

#[tonic::async_trait]
impl ServerReflection for Reflection {
    type ServerReflectionInfoStream = Pin<Box<dyn Stream<Item = Result<pb::ServerReflectionResponse, Status>> + Send>>;

    async fn server_reflection_info(
        &self, request: Request<Streaming<pb::ServerReflectionRequest>>,
    ) -> Result<Response<Self::ServerReflectionInfoStream>, Status> {
        let provider = self.clone();
        // Each Message Sees Current Readiness, Even On A Long-Lived Stream.
        // Listener Middleware Authenticates This RPC Before Service Dispatch.
        let stream = request.into_inner().map(move |request| request.map(|request| provider.respond(request)));
        Ok(Response::new(Box::pin(stream)))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn answer(pool: &DescriptorPool, request: Option<&MessageRequest>) -> Result<MessageResponse, Status> {
    let missing = || Status::not_found("descriptor not found");
    match request.ok_or_else(|| Status::invalid_argument("missing reflection request"))? {
        MessageRequest::ListServices(_) => {
            let mut names: Vec<_> = pool.services().map(|service| service.full_name().to_owned()).collect();
            names.sort_unstable();
            Ok(MessageResponse::ListServicesResponse(pb::ListServiceResponse {
                service: names.into_iter().map(|name| pb::ServiceResponse { name }).collect(),
            }))
        }
        MessageRequest::FileByFilename(name) => file_response(pool.get_file_by_name(name).ok_or_else(missing)?),
        MessageRequest::FileContainingSymbol(name) => file_response(symbol_file(pool, name).ok_or_else(missing)?),
        MessageRequest::FileContainingExtension(extension) => {
            let message = pool.get_message_by_name(&extension.containing_type).ok_or_else(missing)?;
            let number = u32::try_from(extension.extension_number).map_err(|_| missing())?;
            let extension = message.get_extension(number).ok_or_else(missing)?;
            file_response(extension.parent_file())
        }
        MessageRequest::AllExtensionNumbersOfType(name) => {
            let message = pool.get_message_by_name(name).ok_or_else(missing)?;
            let mut numbers: Vec<_> = message.extensions().map(|extension| extension.number() as i32).collect();
            numbers.sort_unstable();
            Ok(MessageResponse::AllExtensionNumbersResponse(pb::ExtensionNumberResponse {
                base_type_name: name.clone(),
                extension_number: numbers,
            }))
        }
    }
}

fn symbol_file(pool: &DescriptorPool, name: &str) -> Option<FileDescriptor> {
    if let Some(message) = pool.get_message_by_name(name) {
        return Some(message.parent_file());
    }
    if let Some(service) = pool.get_service_by_name(name) {
        return Some(service.parent_file());
    }
    if let Some(enumeration) = pool.get_enum_by_name(name) {
        return Some(enumeration.parent_file());
    }
    if let Some(extension) = pool.get_extension_by_name(name) {
        return Some(extension.parent_file());
    }
    for service in pool.services() {
        if service.methods().any(|method| method.full_name() == name) {
            return Some(service.parent_file());
        }
    }
    for message in pool.all_messages() {
        if message.fields().any(|field| field.full_name() == name) || message.oneofs().any(|oneof| oneof.full_name() == name) {
            return Some(message.parent_file());
        }
    }
    for enumeration in pool.all_enums() {
        if enumeration.values().any(|value| value.full_name() == name) {
            return Some(enumeration.parent_file());
        }
    }
    None
}

fn file_response(file: FileDescriptor) -> Result<MessageResponse, Status> {
    let mut pending = vec![file];
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    let mut bytes = 0usize;
    while let Some(file) = pending.pop() {
        if !seen.insert(file.name().to_owned()) {
            continue;
        }
        let encoded = file.file_descriptor_proto().encode_to_vec();
        bytes = bytes.checked_add(encoded.len() + 10).ok_or_else(|| Status::resource_exhausted("descriptor response too large"))?;
        if bytes > MAX_RESPONSE_BYTES - MAX_REQUEST_BYTES {
            return Err(Status::resource_exhausted("descriptor response too large"));
        }
        files.push(encoded);
        pending.extend(file.dependencies());
    }
    Ok(MessageResponse::FileDescriptorResponse(pb::FileDescriptorResponse { file_descriptor_proto: files }))
}

/* === Tests === */
