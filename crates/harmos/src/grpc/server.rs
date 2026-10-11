/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "concurrency.rs"]
mod concurrency;

/* === Re-Exports === */
pub use concurrency::Calls;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc, time::Duration};

// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status, Streaming, body::Body, codegen::http};

// Local Imports
use super::{Codec, DecodeSettings, EncodeSettings, Event, InputEnvelope, InputError};
use crate::operations::{
    InputDeclaration, OperationDeclaration, OperationInstance, OutputDeclaration, StreamItem,
    StreamSettings,
};
use crate::{Error, MemorySize, OperationManager, Protobuf, Refusal};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type HttpFuture = Pin<Box<dyn Future<Output = http::Response<Body>> + Send>>;
type Items<I, R> = Pin<Box<dyn Stream<Item = Result<Event<StreamItem<I>, Arc<R>>, Status>> + Send>>;

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub calls: std::num::NonZeroUsize,
    pub decode: DecodeSettings,
    pub encode: EncodeSettings,
    pub input: StreamSettings,
    pub output: StreamSettings,
    pub opening_timeout: Duration,
}

struct Endpoint<O> {
    authority: Authority,
    settings: Settings,
    operation: PhantomData<fn() -> O>,
}

/* === Enums === */
#[derive(Clone)]
pub enum Authority {
    TrustedHost(OperationManager),

    Binding(crate::operations::GrpcBinding),
    #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
    Plugin(crate::operations::Caller),
}

/* === Traits === */

/* === Implementations === */
impl Authority {
    fn contextual<T>(&self, request: &Request<T>) -> Self {
        let Some(context) = request.extensions().get::<super::ingress::TrustedContext>() else {
            return self.clone();
        };
        match self {
            Self::Binding(binding) => Self::Binding(binding.with_context(context.0.clone())),
            Self::TrustedHost(operations) => {
                Self::TrustedHost(operations.with_context(context.0.clone()))
            }
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            Self::Plugin(caller) => Self::Plugin(caller.with_context(context.0.clone())),
        }
    }

    fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: StreamItem<O::Request>,
        endpoints: P,
    ) -> crate::Result<OperationInstance<O>> {
        match self {
            Self::Binding(binding) => binding.submit::<O, P>(request, endpoints),
            Self::TrustedHost(operations) => operations.submit_charged::<O, P>(request, endpoints),
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            Self::Plugin(caller) => caller.submit_charged::<O, P>(request, endpoints),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        let bytes = std::num::NonZeroUsize::new(16 * 1024 * 1024).unwrap();
        let message = std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap();
        let items = std::num::NonZeroUsize::new(16).unwrap();
        Self {
            calls: std::num::NonZeroUsize::new(64).unwrap(),
            decode: DecodeSettings {
                message_bytes: message,
                retained_bytes: bytes,
            },
            encode: EncodeSettings {
                message_bytes: message,
                buffer_bytes: std::num::NonZeroUsize::new(8192).unwrap(),
            },
            input: StreamSettings { items, bytes },
            output: StreamSettings { items, bytes },
            opening_timeout: Duration::from_secs(5),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), Status> {
        Calls::validate(self.calls)?;
        self.decode.validate()?;
        self.encode.validate()?;
        self.input
            .validate()
            .map_err(|error| status(error.into()))?;
        self.output
            .validate()
            .map_err(|error| status(error.into()))?;
        if self.opening_timeout.is_zero() {
            return Err(Status::invalid_argument(
                "gRPC opening timeout must be positive",
            ));
        }
        Ok(())
    }
}

impl<O> tonic::server::UnaryService<StreamItem<O::Request>> for Endpoint<O>
where
    O: OperationDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
{
    type Response = Arc<O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<StreamItem<O::Request>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        Box::pin(async move {
            let instance = authority
                .submit_charged::<O, _>(request.into_inner(), ())
                .map_err(status)?;
            super::response(instance)
                .await
                .map(Response::new)
                .map_err(status)
        })
    }
}
impl<O> tonic::server::ServerStreamingService<StreamItem<O::Request>> for Endpoint<O>
where
    O: OutputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    O::Item: Protobuf + MemorySize,
{
    type Response = Event<StreamItem<O::Item>, Arc<O::Response>>;
    type ResponseStream = Items<O::Item, O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(&mut self, request: Request<StreamItem<O::Request>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (output, items) = crate::operations::stream(settings.output).map_err(status)?;
            let instance = authority
                .submit_charged::<O, _>(request.into_inner(), output)
                .map_err(status)?;
            let output = super::stream(instance, items).map(|result| result.map_err(status));
            Ok(Response::new(Box::pin(output) as Self::ResponseStream))
        })
    }
}
impl<O> tonic::server::ClientStreamingService<StreamItem<InputEnvelope<O::Request, O::Item>>>
    for Endpoint<O>
where
    O: InputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    O::Item: Protobuf + MemorySize,
{
    type Response = Arc<O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::Response>, Status>;
    fn call(
        &mut self,
        request: Request<Streaming<StreamItem<InputEnvelope<O::Request, O::Item>>>>,
    ) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (request, incoming) =
                tokio::time::timeout(settings.opening_timeout, super::open(request.into_inner()))
                    .await
                    .map_err(|_| Status::deadline_exceeded("operation opening timed out"))??;
            let (input, endpoint) = crate::operations::stream(settings.input).map_err(status)?;
            let instance = authority
                .submit_charged::<O, _>(request, endpoint)
                .map_err(status)?;
            super::input(instance, input, incoming)
                .await
                .map(Response::new)
                .map_err(input_status)
        })
    }
}
impl<O>
    tonic::server::StreamingService<
        StreamItem<InputEnvelope<O::Request, <O as InputDeclaration>::Item>>,
    > for Endpoint<O>
where
    O: InputDeclaration + OutputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    <O as InputDeclaration>::Item: Protobuf + MemorySize,
    <O as OutputDeclaration>::Item: Protobuf + MemorySize,
{
    type Response = Event<StreamItem<<O as OutputDeclaration>::Item>, Arc<O::Response>>;
    type ResponseStream = Items<<O as OutputDeclaration>::Item, O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(
        &mut self,
        request: Request<
            Streaming<StreamItem<InputEnvelope<O::Request, <O as InputDeclaration>::Item>>>,
        >,
    ) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (request, incoming) =
                tokio::time::timeout(settings.opening_timeout, super::open(request.into_inner()))
                    .await
                    .map_err(|_| Status::deadline_exceeded("operation opening timed out"))??;
            let (input, receiving) = crate::operations::stream(settings.input).map_err(status)?;
            let (sending, output) = crate::operations::stream(settings.output).map_err(status)?;
            let instance = authority
                .submit_charged::<O, _>(request, (receiving, sending))
                .map_err(status)?;
            let output = super::exchange(instance, input, incoming, output)
                .map(|result| result.map_err(input_status));
            Ok(Response::new(Box::pin(output) as Self::ResponseStream))
        })
    }
}

/* === Public Functions === */
pub fn unary<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: OperationDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec =
            match Codec::<O::Request, Arc<O::Response>>::new(settings.decode, settings.encode) {
                Ok(codec) => codec,
                Err(error) => return error.into_http(),
            };
        let call = Endpoint::<O> {
            authority,
            settings,
            operation: PhantomData,
        };
        codec.server().unary(call, request).await
    })
}
pub fn download<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: OutputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    O::Item: Protobuf + MemorySize,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<O::Request, Event<StreamItem<O::Item>, Arc<O::Response>>>::new(
            settings.decode,
            settings.encode,
        ) {
            Ok(codec) => codec,
            Err(error) => return error.into_http(),
        };
        let call = Endpoint::<O> {
            authority,
            settings,
            operation: PhantomData,
        };
        codec.server().server_streaming(call, request).await
    })
}
pub fn upload<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: InputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    O::Item: Protobuf + MemorySize,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<InputEnvelope<O::Request, O::Item>, Arc<O::Response>>::new(
            settings.decode,
            settings.encode,
        ) {
            Ok(codec) => codec,
            Err(error) => return error.into_http(),
        };
        let call = Endpoint::<O> {
            authority,
            settings,
            operation: PhantomData,
        };
        codec.server().client_streaming(call, request).await
    })
}
pub fn exchange<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: InputDeclaration + OutputDeclaration,
    O::Request: Protobuf + MemorySize,
    O::Response: Protobuf,
    <O as InputDeclaration>::Item: Protobuf + MemorySize,
    <O as OutputDeclaration>::Item: Protobuf + MemorySize,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<
            InputEnvelope<O::Request, <O as InputDeclaration>::Item>,
            Event<StreamItem<<O as OutputDeclaration>::Item>, Arc<O::Response>>,
        >::new(settings.decode, settings.encode)
        {
            Ok(codec) => codec,
            Err(error) => return error.into_http(),
        };
        let call = Endpoint::<O> {
            authority,
            settings,
            operation: PhantomData,
        };
        codec.server().streaming(call, request).await
    })
}

/* === Private Functions === */
fn code(error: &Error) -> tonic::Code {
    match error {
        Error::OperationRefused(Refusal::TimedOut) => tonic::Code::DeadlineExceeded,
        Error::OperationRefused(Refusal::Cancelled) => tonic::Code::Cancelled,
        Error::OperationRefused(Refusal::Unavailable)
        | Error::NotReady { .. }
        | Error::Closing { .. } => tonic::Code::Unavailable,
        Error::ResourceExhausted { .. } => tonic::Code::ResourceExhausted,
        Error::InvalidInput { .. } | Error::OperationPayload { .. } => tonic::Code::InvalidArgument,
        Error::Unauthenticated => tonic::Code::Unauthenticated,
        Error::PermissionDenied | Error::OperationUnauthorized { .. } => {
            tonic::Code::PermissionDenied
        }
        #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
        Error::OperationCaller { .. } => tonic::Code::Unauthenticated,
        Error::TaskFailed { source, .. } => source
            .downcast_ref::<Status>()
            .map_or(tonic::Code::Internal, Status::code),
        _ => tonic::Code::Internal,
    }
}
pub(super) fn status(error: Error) -> Status {
    let mut status = Status::new(code(&error), "operation request failed");
    status.set_source(Arc::new(error));
    status
}
fn input_status(error: InputError) -> Status {
    let code = match &error {
        InputError::Operation(error) => code(error),
        InputError::Forward { source, settlement } => {
            if settlement.as_deref().is_some_and(|error| {
                matches!(
                    error,
                    Error::OperationFinalization { .. } | Error::OperationSettlement { .. }
                )
            }) {
                tonic::Code::Internal
            } else {
                code(source)
            }
        }
    };
    let mut status = Status::new(code, "operation input failed");
    status.set_source(Arc::new(error));
    status
}

/* === Tests === */
