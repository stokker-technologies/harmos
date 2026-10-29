/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::pin::Pin;

use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status, Streaming, body::Body, codegen::http};

use super::{Codec, InputEnvelope, server};
use crate::operations::{EncodedInvocation, GrpcBinding, StreamItem};
use harmos_sdk::operations::{WireStreamLimits, WireStreams};

type Incoming = Streaming<StreamItem<InputEnvelope<Vec<u8>, Vec<u8>>>>;
type Forward = Pin<Box<dyn Future<Output = Result<(), Status>> + Send>>;
type OutputStream = Pin<Box<dyn Stream<Item = Result<Output, Status>> + Send>>;

struct Raw;

struct Output {
    bytes: Vec<u8>,
    tag: Option<u32>,
}

struct Endpoint {
    binding: GrpcBinding,
    settings: server::Settings,
}

struct Pending {
    invocation: EncodedInvocation,
    streams: Option<WireStreams>,
    forward: Option<Forward>,
    response: Option<Vec<u8>>,
    output: bool,
    settled: bool,
}

impl super::Wire<Vec<u8>> for Raw {
    fn encode(value: &Vec<u8>, bytes: &mut Vec<u8>) -> Result<(), crate::MessageError> {
        bytes.extend_from_slice(value);
        Ok(())
    }

    fn encode_admitted(
        value: &Vec<u8>, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
    ) -> Result<(), Status> {
        super::codec::encode_bytes(value, bytes, limit, admit)
    }

    fn decode_admitted<C>(
        input: harmos_sdk::grpc::DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, Status>,
    ) -> Result<(Vec<u8>, C), Status> {
        let bytes = input.contiguous().map_err(|error| Status::invalid_argument(error.to_string()))?;
        let claim = admit(bytes.len().saturating_add(std::mem::size_of::<Vec<u8>>()))?;
        Ok((bytes.to_vec(), claim))
    }
}

impl super::Encode<Raw> for Output {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), crate::MessageError> {
        match self.tag {
            Some(tag) => super::envelope::delimited(tag, bytes, |bytes| {
                bytes.extend_from_slice(&self.bytes);
                Ok(())
            }),
            None => {
                bytes.extend_from_slice(&self.bytes);
                Ok(())
            }
        }
    }

    fn write_admitted(&self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
        match self.tag {
            Some(tag) => super::envelope::delimited_admitted(tag, bytes, limit, admit, |bytes, admit| {
                super::codec::encode_bytes(&self.bytes, bytes, limit, admit)
            }),
            None => super::codec::encode_bytes(&self.bytes, bytes, limit, admit),
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.settled {
            self.invocation.cancel();
        }
    }
}

impl Pending {
    async fn next(&mut self) -> Result<Option<Output>, Status> {
        loop {
            if !self.output
                && let Some(bytes) = self.response.take()
            {
                return Ok(Some(Output { bytes, tag: Some(2) }));
            }
            if self.settled && !self.output {
                return Ok(None);
            }
            tokio::select! {
                biased;
                result = &mut self.invocation, if !self.settled => {
                    self.settled = true;
                    self.response = Some(result.map_err(server::status)?);
                }
                result = async { self.forward.as_mut().expect("enabled input").await }, if self.forward.is_some() => {
                    self.forward = None;
                    if let Err(error) = result {
                        if !self.settled {
                            self.invocation.cancel();
                            let _ = (&mut self.invocation).await;
                            self.settled = true;
                        }
                        return Err(error);
                    }
                }
                result = async { self.streams.as_ref().expect("declared output").receive_current().await }, if self.output => {
                    match result.map_err(serving)? {
                        Some(bytes) => return Ok(Some(Output { bytes, tag: Some(1) })),
                        None => self.output = false,
                    }
                }
            }
        }
    }
    fn output(self) -> OutputStream {
        Box::pin(futures_util::stream::unfold(Some(self), |state| async {
            let mut pending = state?;
            match pending.next().await {
                Ok(Some(output)) => Some((Ok(output), Some(pending))),
                Ok(None) => None,
                Err(error) => Some((Err(error), None)),
            }
        }))
    }
    async fn response(mut self) -> Result<Response<Output>, Status> {
        let mut output = self.next().await?.ok_or_else(|| Status::internal("missing operation response"))?;
        output.tag = None;
        Ok(Response::new(output))
    }
}

impl Endpoint {
    fn binding<T>(&self, request: &Request<T>) -> GrpcBinding {
        request
            .extensions()
            .get::<super::inbound::TrustedContext>()
            .map_or_else(|| self.binding.clone(), |context| self.binding.with_context(context.0.clone()))
    }
    fn start(&self, binding: GrpcBinding, payload: StreamItem<Vec<u8>>) -> Result<Pending, Status> {
        let output = binding.contract().output().is_some();
        let (invocation, streams) = binding
            .submit_native(
                payload,
                WireStreamLimits {
                    input: self.settings.input,
                    output: self.settings.output,
                },
            )
            .map_err(server::status)?;
        Ok(Pending {
            invocation,
            streams,
            output,
            forward: None,
            response: None,
            settled: false,
        })
    }
    async fn opening(&self, request: Request<Incoming>) -> Result<Pending, Status> {
        let binding = self.binding(&request);
        let (request, incoming) = tokio::time::timeout(self.settings.opening_timeout, super::open(request.into_inner()))
            .await
            .map_err(|_| Status::deadline_exceeded("operation opening timed out"))??;
        let mut pending = self.start(binding, request)?;
        let streams = pending.streams.clone().ok_or_else(|| Status::internal("missing operation input"))?;
        pending.forward = Some(Box::pin(async move {
            tokio::pin!(incoming);
            while let Some(item) = incoming.next().await {
                streams.send(1, &item?).await.map_err(serving)?;
            }
            streams.finish_input(1).map_err(serving)
        }));
        Ok(pending)
    }
}

impl tonic::server::UnaryService<StreamItem<Vec<u8>>> for Endpoint {
    type Response = Output;
    type Future = tonic::codegen::BoxFuture<Response<Output>, Status>;
    fn call(&mut self, request: Request<StreamItem<Vec<u8>>>) -> Self::Future {
        let pending = self.start(self.binding(&request), request.into_inner());
        Box::pin(async move { pending?.response().await })
    }
}

impl tonic::server::ServerStreamingService<StreamItem<Vec<u8>>> for Endpoint {
    type Response = Output;
    type ResponseStream = OutputStream;
    type Future = tonic::codegen::BoxFuture<Response<OutputStream>, Status>;
    fn call(&mut self, request: Request<StreamItem<Vec<u8>>>) -> Self::Future {
        let pending = self.start(self.binding(&request), request.into_inner());
        Box::pin(async move { Ok(Response::new(pending?.output())) })
    }
}

impl tonic::server::ClientStreamingService<StreamItem<InputEnvelope<Vec<u8>, Vec<u8>>>> for Endpoint {
    type Response = Output;
    type Future = tonic::codegen::BoxFuture<Response<Output>, Status>;
    fn call(&mut self, request: Request<Incoming>) -> Self::Future {
        let endpoint = Self {
            binding: self.binding.clone(),
            settings: self.settings,
        };
        Box::pin(async move { endpoint.opening(request).await?.response().await })
    }
}

impl tonic::server::StreamingService<StreamItem<InputEnvelope<Vec<u8>, Vec<u8>>>> for Endpoint {
    type Response = Output;
    type ResponseStream = OutputStream;
    type Future = tonic::codegen::BoxFuture<Response<OutputStream>, Status>;
    fn call(&mut self, request: Request<Incoming>) -> Self::Future {
        let endpoint = Self {
            binding: self.binding.clone(),
            settings: self.settings,
        };
        Box::pin(async move { Ok(Response::new(endpoint.opening(request).await?.output())) })
    }
}

pub(crate) fn serve(authority: server::Authority, settings: server::Settings, request: http::Request<Body>) -> server::HttpFuture {
    Box::pin(async move {
        let server::Authority::Binding(binding) = authority else {
            return Status::internal("native operation binding required").into_http();
        };
        let input = binding.contract().input().is_some();
        let output = binding.contract().output().is_some();
        let factory = match binding.native_factory() {
            Ok(factory) => factory,
            Err(error) => return server::status(error).into_http(),
        };
        let memory: super::decoding::AdditionalMemory = std::sync::Arc::new(move |bytes| {
            let encoded = harmos_sdk::grpc::DecodeInput::new(bytes);
            let payload = if input {
                let (tag, payload) = encoded
                    .variant(&[1, 2])
                    .map_err(|error| Status::invalid_argument(error.to_string()))?
                    .ok_or_else(|| Status::invalid_argument("empty operation envelope"))?;
                if tag == 2 {
                    return Ok(0);
                }
                payload.contiguous().map_err(|error| Status::invalid_argument(error.to_string()))?
            } else {
                bytes
            };
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| factory.grpc_decoding_memory(payload)))
                .map_err(|_| Status::internal("operation decoding panicked"))?
                .map_err(|error| Status::invalid_argument(error.to_string()))
        });
        let preparation = binding.operations.preparation.clone();
        let endpoint = Endpoint { binding, settings };
        if input {
            let codec = match Codec::<Raw, InputEnvelope<Vec<u8>, Vec<u8>>, Output>::new(settings.decode, settings.encode) {
                Ok(codec) => codec.with_additional_memory(memory).with_preparation(preparation),
                Err(error) => return error.into_http(),
            };
            if output {
                codec.server().streaming(endpoint, request).await
            } else {
                codec.server().client_streaming(endpoint, request).await
            }
        } else {
            let codec = match Codec::<Raw, Vec<u8>, Output>::new(settings.decode, settings.encode) {
                Ok(codec) => codec.with_additional_memory(memory).with_preparation(preparation),
                Err(error) => return error.into_http(),
            };
            if output { codec.server().server_streaming(endpoint, request).await } else { codec.server().unary(endpoint, request).await }
        }
    })
}

fn serving(error: harmos_sdk::ServingError) -> Status {
    Status::internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::{Encode, Wire};

    #[test]
    fn raw_encoding_admits_actual_workspace() {
        let payload = vec![37; 32];
        let mut bytes = Vec::new();
        let mut peak = 0;
        let mut admit = |required| {
            peak = peak.max(required);
            if required > 4096 { Err(Status::resource_exhausted("fixture credit")) } else { Ok(()) }
        };
        Raw::encode_admitted(&payload, &mut bytes, 65536, &mut admit).unwrap();
        assert_eq!(bytes, payload);

        for tag in [None, Some(1), Some(2)] {
            let output = Output { bytes: payload.clone(), tag };
            let mut expected = Vec::new();
            output.write(&mut expected).unwrap();
            bytes.clear();
            output.write_admitted(&mut bytes, 65536, &mut admit).unwrap();
            assert_eq!(bytes, expected);
        }
        assert!(peak < 4096);
    }

    #[test]
    fn raw_growth_refused_before_allocation() {
        let payload = vec![37; 65];
        let mut bytes = Vec::with_capacity(32);
        let capacity = bytes.capacity();
        let mut admit = |required| {
            if required > 64 { Err(Status::resource_exhausted("fixture credit")) } else { Ok(()) }
        };
        let error = Raw::encode_admitted(&payload, &mut bytes, 65536, &mut admit).unwrap_err();
        assert_eq!(error.code(), tonic::Code::ResourceExhausted);
        assert!(bytes.is_empty());
        assert_eq!(bytes.capacity(), capacity);

        for tag in [None, Some(1), Some(2)] {
            let output = Output { bytes: payload.clone(), tag };
            let error = output.write_admitted(&mut bytes, 65536, &mut admit).unwrap_err();
            assert_eq!(error.code(), tonic::Code::ResourceExhausted);
            assert!(bytes.is_empty());
            assert_eq!(bytes.capacity(), capacity);
        }

        let error = Raw::encode_admitted(&payload, &mut bytes, 64, &mut |_| Ok(())).unwrap_err();
        assert_eq!(error.code(), tonic::Code::ResourceExhausted);
        assert!(bytes.is_empty());
        assert_eq!(bytes.capacity(), capacity);
    }
}
