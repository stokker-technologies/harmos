/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod concurrency;

/* === Re-Exports === */

pub use concurrency::Calls;

/* === Imports === */

// Standard Library Imports
use std::{error, future::Future, marker::PhantomData, pin::Pin, sync::Arc};

// Third Party Imports
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use prost::Message;
use tokio::sync::Semaphore;
use tonic::{
    Code, Request, Response, Status, Streaming,
    body::Body,
    codegen::{BoxFuture, http},
    server::{ClientStreamingService, ServerStreamingService, StreamingService, UnaryService},
};

// Local Imports
use super::inbound::TrustedContext;
use super::{Codec, Event, InputEnvelope, InputError, Wire};
use crate::clock::Clock;
use crate::config::GrpcServerSettings;

#[cfg(feature = "plugin-guest")]
use crate::operations::Caller;

use crate::operations::{Direction, GrpcBinding, OperationManager, RequestShape, Running, StreamItem};
use crate::{
    Error, Refusal,
    operations::{self, Manager},
};
use harmos_operations::WRONG_CALLER;
use harmos_operations::execution::ServingError;
use harmos_operations::execution::operations::streaming;
use harmos_operations::operations::FailureKind;
use harmos_protocol::model::Failure;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type HttpFuture = Pin<Box<dyn Future<Output = http::Response<Body>> + Send>>;
type Items<I> = Pin<Box<dyn Stream<Item = Result<StreamItem<I>, Status>> + Send>>;

/* === Structs === */

struct Endpoint<O> {
    authority: Authority,
    settings: GrpcServerSettings,
    operation: PhantomData<fn() -> O>,
}

/* === Enums === */

#[derive(Clone)]
pub enum Authority {
    TrustedHost(Manager),

    Binding(GrpcBinding),

    #[cfg(feature = "plugin-guest")]
    Plugin(Caller),
}

/* === Traits === */

/* === Implementations === */

impl Authority {
    pub(super) fn clock(&self) -> &Clock {
        match self {
            Self::TrustedHost(operations) => operations.clock(),
            Self::Binding(binding) => binding.operations.clock(),
            #[cfg(feature = "plugin-guest")]
            Self::Plugin(caller) => caller.operations.clock(),
        }
    }

    pub fn trusted_host(operations: &OperationManager) -> Self {
        Self::TrustedHost(operations.execution())
    }

    pub(super) fn preparation(&self) -> Option<Arc<Semaphore>> {
        match self {
            Self::TrustedHost(operations) => operations.preparation.clone(),
            Self::Binding(binding) => binding.operations.preparation.clone(),
            #[cfg(feature = "plugin-guest")]
            Self::Plugin(caller) => caller.operations.preparation.clone(),
        }
    }

    pub(super) fn contextual<T>(&self, request: &Request<T>) -> Self {
        let Some(context) = request.extensions().get::<TrustedContext>() else {
            return self.clone();
        };
        match self {
            Self::Binding(binding) => Self::Binding(binding.with_context(context.0.clone())),
            Self::TrustedHost(operations) => Self::TrustedHost(operations.with_context(context.0.clone())),
            #[cfg(feature = "plugin-guest")]
            Self::Plugin(caller) => Self::Plugin(caller.with_context(context.0.clone())),
        }
    }

    pub(super) fn submit_charged<O: RequestShape, P: Send + 'static>(
        &self, request: StreamItem<O::Request>, endpoints: P,
    ) -> crate::Result<Running<O>> {
        let instance = match self {
            Self::Binding(binding) => binding.submit(request, endpoints),
            Self::TrustedHost(operations) => operations.submit_external_charged::<O, P>(request, endpoints),
            #[cfg(feature = "plugin-guest")]
            Self::Plugin(caller) => caller.submit_external_charged::<O, P>(request, endpoints),
        }?;
        let context = match self {
            Self::Binding(binding) => binding.operations.call_context(),
            Self::TrustedHost(operations) => operations.call_context(),
            #[cfg(feature = "plugin-guest")]
            Self::Plugin(caller) => caller.operations.call_context(),
        };
        if let Some(scope) = context.get::<Arc<super::connection::Scope>>() {
            scope.track(instance.control.clone());
        }
        Ok(instance)
    }
}

impl GrpcServerSettings {
    pub fn validate(&self) -> Result<(), Status> {
        Calls::validate(self.calls)?;
        self.decode.validate()?;
        self.encode.validate()?;
        streaming::validate(&self.input).map_err(|error| status(error.into()))?;
        streaming::validate(&self.output).map_err(|error| status(error.into()))?;
        if self.opening_timeout.is_zero() {
            return Err(Status::invalid_argument("gRPC opening timeout must be positive"));
        }
        Ok(())
    }
}

impl<O> UnaryService<StreamItem<O::Request>> for Endpoint<O>
where
    O: RequestShape,
{
    type Response = operations::Response<O::Response>;
    type Future = BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<StreamItem<O::Request>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        Box::pin(async move {
            let instance = authority.submit_charged::<O, _>(request.into_inner(), ()).map_err(status)?;
            super::response(instance).await.map(Response::new).map_err(status)
        })
    }
}
impl<O> ServerStreamingService<StreamItem<O::Request>> for Endpoint<O>
where
    O: RequestShape,
{
    type Response = StreamItem<<O::Output as Direction>::Item>;
    type ResponseStream = Items<<O::Output as Direction>::Item>;
    type Future = BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(&mut self, request: Request<StreamItem<O::Request>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (output, items) = operations::stream(settings.output).map_err(status)?;
            let instance = authority.submit_charged::<O, _>(request.into_inner(), output).map_err(status)?;
            let operation = instance.operation().to_owned();
            let output = super::stream(instance, items).map(move |result| result.map_err(|error| in_operation(status(error), &operation)));
            let output = output.filter_map(|event| async move {
                match event {
                    Ok(Event::Item(item)) => Some(Ok(item)),
                    Ok(Event::Completion(_)) => None,
                    Err(error) => Some(Err(error)),
                }
            });
            Ok(Response::new(Box::pin(output) as Self::ResponseStream))
        })
    }
}
impl<O> ClientStreamingService<StreamItem<InputEnvelope<O::Request, <O::Input as Direction>::Item>>> for Endpoint<O>
where
    O: RequestShape,
{
    type Response = operations::Response<O::Response>;
    type Future = BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<Streaming<StreamItem<InputEnvelope<O::Request, <O::Input as Direction>::Item>>>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (request, incoming) = authority
                .clock()
                .timeout(settings.opening_timeout, super::open(request.into_inner()))
                .await
                .map_err(|_| Status::deadline_exceeded("operation opening timed out"))??;
            let (input, endpoint) = operations::stream(settings.input).map_err(status)?;
            let instance = authority.submit_charged::<O, _>(request, endpoint).map_err(status)?;
            let operation = instance.operation().to_owned();
            super::input(instance, input, incoming).await.map(Response::new).map_err(|error| in_operation(input_status(error), &operation))
        })
    }
}
impl<O> StreamingService<StreamItem<InputEnvelope<O::Request, <O::Input as Direction>::Item>>> for Endpoint<O>
where
    O: RequestShape,
{
    type Response = StreamItem<<O::Output as Direction>::Item>;
    type ResponseStream = Items<<O::Output as Direction>::Item>;
    type Future = BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(&mut self, request: Request<Streaming<StreamItem<InputEnvelope<O::Request, <O::Input as Direction>::Item>>>>) -> Self::Future {
        let authority = self.authority.contextual(&request);
        let settings = self.settings;
        Box::pin(async move {
            let (request, incoming) = authority
                .clock()
                .timeout(settings.opening_timeout, super::open(request.into_inner()))
                .await
                .map_err(|_| Status::deadline_exceeded("operation opening timed out"))??;
            let (input, receiving) = operations::stream(settings.input).map_err(status)?;
            let (sending, output) = operations::stream(settings.output).map_err(status)?;
            let instance = authority.submit_charged::<O, _>(request, (receiving, sending)).map_err(status)?;
            let operation = instance.operation().to_owned();
            let output = super::exchange(instance, input, incoming, output)
                .map(move |result| result.map_err(|error| in_operation(input_status(error), &operation)));
            let output = output.filter_map(|event| async move {
                match event {
                    Ok(Event::Item(item)) => Some(Ok(item)),
                    Ok(Event::Completion(_)) => None,
                    Err(error) => Some(Err(error)),
                }
            });
            Ok(Response::new(Box::pin(output) as Self::ResponseStream))
        })
    }
}

/* === Public Functions === */

pub fn unary<O, W>(authority: Authority, settings: GrpcServerSettings, request: http::Request<Body>) -> HttpFuture
where
    O: RequestShape,
    W: Wire<O::Request> + Wire<<O::Input as Direction>::Item> + Wire<<O::Output as Direction>::Item> + Wire<O::Response>,
{
    if super::controlled::is_managed(&authority, request.uri().path()) {
        return super::controlled::serve::<O, W>(authority, settings, request);
    }
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<W, O::Request, operations::Response<O::Response>>::new(settings.decode, settings.encode) {
            Ok(codec) => codec.with_preparation(authority.preparation()),
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
pub fn download<O, W>(authority: Authority, settings: GrpcServerSettings, request: http::Request<Body>) -> HttpFuture
where
    O: RequestShape,
    W: Wire<O::Request> + Wire<<O::Input as Direction>::Item> + Wire<<O::Output as Direction>::Item> + Wire<O::Response>,
{
    if super::controlled::is_managed(&authority, request.uri().path()) {
        return super::controlled::serve::<O, W>(authority, settings, request);
    }
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<W, O::Request, StreamItem<<O::Output as Direction>::Item>>::new(settings.decode, settings.encode) {
            Ok(codec) => codec.with_preparation(authority.preparation()),
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
pub fn upload<O, W>(authority: Authority, settings: GrpcServerSettings, request: http::Request<Body>) -> HttpFuture
where
    O: RequestShape,
    W: Wire<O::Request> + Wire<<O::Input as Direction>::Item> + Wire<<O::Output as Direction>::Item> + Wire<O::Response>,
{
    if super::controlled::is_managed(&authority, request.uri().path()) {
        return super::controlled::serve::<O, W>(authority, settings, request);
    }
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<W, InputEnvelope<O::Request, <O::Input as Direction>::Item>, operations::Response<O::Response>>::new(
            settings.decode,
            settings.encode,
        ) {
            Ok(codec) => codec.with_preparation(authority.preparation()),
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
pub fn exchange<O, W>(authority: Authority, settings: GrpcServerSettings, request: http::Request<Body>) -> HttpFuture
where
    O: RequestShape,
    W: Wire<O::Request> + Wire<<O::Input as Direction>::Item> + Wire<<O::Output as Direction>::Item> + Wire<O::Response>,
{
    if super::controlled::is_managed(&authority, request.uri().path()) {
        return super::controlled::serve::<O, W>(authority, settings, request);
    }
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec =
            match Codec::<W, InputEnvelope<O::Request, <O::Input as Direction>::Item>, StreamItem<<O::Output as Direction>::Item>>::new(
                settings.decode,
                settings.encode,
            ) {
                Ok(codec) => codec.with_preparation(authority.preparation()),
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

fn code(error: &Error) -> Code {
    // Preserve Known Runtime Denials Through Authored Child Failures. Bound Wrapping.
    let mut error = error;
    let mut wrapped = false;
    for _ in 0..32 {
        match error {
            Error::TaskFailed { source, .. } | Error::OperationFailed { source, .. } => {
                if let Some(status) = source.downcast_ref::<Status>() {
                    return status.code();
                }
                if matches!(source.downcast_ref::<ServingError>(), Some(ServingError::Codec(_))) {
                    return Code::InvalidArgument;
                }
                if let Some(error) = source.downcast_ref::<harmos_operations::Error>() {
                    return match error {
                        harmos_operations::Error::NotFound { .. } => Code::NotFound,
                        harmos_operations::Error::NotReady { .. } => Code::Unavailable,
                        harmos_operations::Error::ResourceExhausted { .. } => Code::ResourceExhausted,
                        harmos_operations::Error::InvalidInput { .. } | harmos_operations::Error::Message(_) => Code::InvalidArgument,
                        _ => Code::Internal,
                    };
                }
                if let Some(inner) = source.downcast_ref::<Error>().or_else(|| ungranted(source.as_ref())) {
                    error = inner;
                    wrapped = true;
                } else {
                    return Code::Internal;
                }
            }
            _ => {
                if !wrapped {
                    return leaf_code(error);
                }
                // Authored Application Failures Stay Internal Unless They Carry
                // An Explicit Status Or A Known Runtime Authority/Lifecycle Denial.
                return match error {
                    Error::OperationRefused(Refusal::TimedOut { .. } | Refusal::Cancelled { .. })
                    | Error::Unauthenticated
                    | Error::PermissionDenied
                    | Error::OperationUnauthorized { .. } => leaf_code(error),
                    #[cfg(feature = "plugin-guest")]
                    Error::OperationCaller { .. } => leaf_code(error),
                    _ => Code::Internal,
                };
            }
        }
    }
    Code::Internal
}

fn leaf_code(error: &Error) -> Code {
    match error {
        Error::OperationRefused(Refusal::TimedOut { .. }) => Code::DeadlineExceeded,
        Error::OperationRefused(Refusal::Cancelled { .. }) => Code::Cancelled,
        Error::OperationRefused(Refusal::Unavailable) | Error::NotReady { .. } | Error::Closing { .. } => Code::Unavailable,
        Error::ResourceExhausted { .. } => Code::ResourceExhausted,
        Error::InvalidInput { .. } | Error::OperationPayload { .. } => Code::InvalidArgument,
        Error::Unauthenticated => Code::Unauthenticated,
        Error::PermissionDenied | Error::OperationUnauthorized { .. } => Code::PermissionDenied,
        #[cfg(feature = "plugin-guest")]
        Error::OperationCaller { .. } => Code::Unauthenticated,
        _ => Code::Internal,
    }
}
// An Encoded Provider's Prepared Grant The Caller Lacks Keeps Its Runtime Denial.
fn ungranted<'a>(source: &'a (dyn error::Error + Send + Sync + 'static)) -> Option<&'a Error> {
    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    if let Some(operations::ComponentError::Unauthorized(inner)) = source.downcast_ref() {
        return Some(inner);
    }
    let _ = source;
    None
}

// A Sidecar That Rejects the Request During Preparation (an Unreadable Or
// Wrong-Shape Payload) Names The Caller's Fault In Its Own Words.

// A carried plugin failure reaches a remote client as the same structured
// value in the status details. Origin diagnostics stay inside the host: chains
// and backtraces are not disclosed to network callers.
fn with_failure(status: Status, error: &Error) -> Status {
    let Some(failure) = error.operation_failure() else {
        return status;
    };
    if !status.details().is_empty() {
        return status;
    }

    let details = failure.without_diagnostics().encode();
    Status::with_details_and_metadata(status.code(), status.message(), details.into(), status.metadata().clone())
}

fn caller_refusal(mut error: &Error) -> Option<&str> {
    for _ in 0..32 {
        match error {
            Error::OperationUnauthorized { operation, .. } => return Some(operation),
            Error::TaskFailed { source, .. } | Error::OperationFailed { source, .. } => {
                error = source.downcast_ref::<Error>().or_else(|| ungranted(source.as_ref()))?;
            }
            _ => return None,
        }
    }
    None
}

pub(super) fn status(error: Error) -> Status {
    if let Some(failure) = error.operation_failure()
        && failure.code() == FailureKind::Invalid
    {
        let status = Status::invalid_argument(failure.cause());
        let mut status = with_failure(status, &error);
        status.set_source(Arc::new(error));
        return status;
    }
    if let Some(failure) = error.operation_failure()
        && failure.refusal_code() == Some(WRONG_CALLER)
        && failure.code() == FailureKind::Unauthorized
    {
        let details = Failure::from(&failure.clone().without_diagnostics()).encode_to_vec();
        let mut status = Status::with_details(Code::PermissionDenied, failure.cause(), details.into());
        status.set_source(Arc::new(error));
        return status;
    }
    let status = error.explicit_status().unwrap_or_else(|| match caller_refusal(&error) {
        Some(operation) => Status::permission_denied(format!("Call {operation}: caller is not allowed")),
        None => Status::new(code(&error), "operation request failed"),
    });
    let mut status = with_failure(status, &error);
    status.set_source(Arc::new(error));
    status
}
pub(super) fn in_operation(status: Status, operation: &str) -> Status {
    if status.code() != Code::DeadlineExceeded {
        return status;
    }
    let mut named = Status::with_details_and_metadata(
        status.code(),
        format!("operation {operation}: {}", status.message()),
        Bytes::copy_from_slice(status.details()),
        status.metadata().clone(),
    );
    named.set_source(Arc::new(status));
    named
}

fn input_status(error: InputError) -> Status {
    let explicit = match &error {
        InputError::Operation(error) => error.explicit_status(),
        InputError::Forward { source, settlement }
            if !settlement
                .as_deref()
                .is_some_and(|error| matches!(error, Error::OperationFinalization { .. } | Error::OperationSettlement { .. })) =>
        {
            source.explicit_status()
        }
        InputError::Forward { .. } => None,
    };
    let failed = match &error {
        InputError::Operation(error) | InputError::Forward { source: error, .. } => error,
    };
    if let Some(status) = explicit {
        let mut status = with_failure(status, failed);
        status.set_source(Arc::new(error));
        return status;
    }

    let code = match &error {
        InputError::Operation(error) => code(error),
        InputError::Forward { source, settlement } => {
            if settlement
                .as_deref()
                .is_some_and(|error| matches!(error, Error::OperationFinalization { .. } | Error::OperationSettlement { .. }))
            {
                Code::Internal
            } else {
                code(source)
            }
        }
    };
    let status = Status::new(code, "operation input failed");
    let mut status = with_failure(status, failed);
    status.set_source(Arc::new(error));
    status
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tonic::Code;

    use super::status;
    use crate::Error;
    use harmos_operations::MessageError;
    use harmos_operations::execution::ServingError;

    #[test]
    fn native_host_statuses_survive_wrapped_failures() {
        let cases = [
            (harmos_operations::Error::NotFound { entity: "operation" }, Code::NotFound),
            (harmos_operations::Error::NotReady { domain: "operation discovery" }, Code::Unavailable),
            (harmos_operations::Error::ResourceExhausted { resource: "operations" }, Code::ResourceExhausted),
            (
                harmos_operations::Error::InvalidInput {
                    field: "operation",
                    reason: "invalid identity",
                },
                Code::InvalidArgument,
            ),
            (harmos_operations::Error::Message(MessageError::new("invalid identity")), Code::InvalidArgument),
            (harmos_operations::Error::UnknownInstance(1), Code::Internal),
        ];
        for (source, expected) in cases {
            let source: Error = Error::OperationFailed {
                operation: "operations.get".into(),
                source: Arc::new(source),
            };
            let error = Error::TaskFailed {
                domain: "host operation",
                source: Arc::new(source),
            };
            assert_eq!(status(error).code(), expected);
        }

        let source = ServingError::Codec(MessageError::new("invalid identity"));
        let source: Error = Error::OperationFailed {
            operation: "operations.get".into(),
            source: Arc::new(source),
        };
        let error = Error::TaskFailed {
            domain: "host preparation",
            source: Arc::new(source),
        };
        assert_eq!(status(error).code(), Code::InvalidArgument);
    }
}
