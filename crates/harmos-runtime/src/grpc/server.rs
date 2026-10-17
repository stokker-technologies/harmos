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
pub use crate::config::GrpcServerSettings as Settings;
pub use concurrency::Calls;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status, Streaming, body::Body, codegen::http};

// Local Imports
use super::{Codec, Event, InputEnvelope, InputError};
use crate::operations::{OperationDeclaration, OperationInstance, StreamItem};
use crate::{Error, Refusal, operations::OperationManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type HttpFuture = Pin<Box<dyn Future<Output = http::Response<Body>> + Send>>;
type Items<I, R> = Pin<
    Box<
        dyn Stream<Item = Result<Event<StreamItem<I>, crate::operations::Response<R>>, Status>>
            + Send,
    >,
>;

/* === Structs === */

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
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    Plugin(crate::operations::Caller),
}

/* === Traits === */

/* === Implementations === */
impl Authority {
    fn contextual<T>(&self, request: &Request<T>) -> Self {
        let Some(context) = request.extensions().get::<super::inbound::TrustedContext>() else {
            return self.clone();
        };
        match self {
            Self::Binding(binding) => Self::Binding(binding.with_context(context.0.clone())),
            Self::TrustedHost(operations) => {
                Self::TrustedHost(operations.with_context(context.0.clone()))
            }
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            Self::Plugin(caller) => Self::Plugin(caller.with_context(context.0.clone())),
        }
    }

    fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: StreamItem<O>,
        endpoints: P,
    ) -> crate::Result<OperationInstance<O>> {
        match self {
            Self::Binding(binding) => binding.submit(request, endpoints),
            Self::TrustedHost(operations) => {
                operations.submit_external_charged::<O, P>(request, endpoints)
            }
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            Self::Plugin(caller) => caller.submit_external_charged::<O, P>(request, endpoints),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), Status> {
        Calls::validate(self.calls)?;
        self.decode.validate()?;
        self.encode.validate()?;
        harmos_sdk::operations::streaming::validate(&self.input)
            .map_err(|error| status(error.into()))?;
        harmos_sdk::operations::streaming::validate(&self.output)
            .map_err(|error| status(error.into()))?;
        if self.opening_timeout.is_zero() {
            return Err(Status::invalid_argument(
                "gRPC opening timeout must be positive",
            ));
        }
        Ok(())
    }
}

impl<O> tonic::server::UnaryService<StreamItem<O>> for Endpoint<O>
where
    O: OperationDeclaration,
{
    type Response = crate::operations::Response<O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::Response>, Status>;
    fn call(&mut self, request: Request<StreamItem<O>>) -> Self::Future {
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
impl<O> tonic::server::ServerStreamingService<StreamItem<O>> for Endpoint<O>
where
    O: crate::operations::OperationDeclaration,
{
    type Response = Event<
        StreamItem<<O::Output as crate::operations::Direction>::Item>,
        crate::operations::Response<O::Response>,
    >;
    type ResponseStream = Items<<O::Output as crate::operations::Direction>::Item, O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(&mut self, request: Request<StreamItem<O>>) -> Self::Future {
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
impl<O>
    tonic::server::ClientStreamingService<
        StreamItem<InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>>,
    > for Endpoint<O>
where
    O: crate::operations::OperationDeclaration,
{
    type Response = crate::operations::Response<O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::Response>, Status>;
    fn call(
        &mut self,
        request: Request<
            Streaming<
                StreamItem<InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>>,
            >,
        >,
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
        StreamItem<InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>>,
    > for Endpoint<O>
where
    O: crate::operations::OperationDeclaration,
{
    type Response = Event<
        StreamItem<<O::Output as crate::operations::Direction>::Item>,
        crate::operations::Response<O::Response>,
    >;
    type ResponseStream = Items<<O::Output as crate::operations::Direction>::Item, O::Response>;
    type Future = tonic::codegen::BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(
        &mut self,
        request: Request<
            Streaming<
                StreamItem<InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>>,
            >,
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
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<O, crate::operations::Response<O::Response>>::new(
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
        codec.server().unary(call, request).await
    })
}
pub fn download<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: crate::operations::OperationDeclaration,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<
            O,
            Event<
                StreamItem<<O::Output as crate::operations::Direction>::Item>,
                crate::operations::Response<O::Response>,
            >,
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
        codec.server().server_streaming(call, request).await
    })
}
pub fn upload<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: crate::operations::OperationDeclaration,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<
            InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>,
            crate::operations::Response<O::Response>,
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
        codec.server().client_streaming(call, request).await
    })
}
pub fn exchange<O>(
    authority: Authority,
    settings: Settings,
    request: http::Request<Body>,
) -> HttpFuture
where
    O: crate::operations::OperationDeclaration,
{
    Box::pin(async move {
        if let Err(error) = settings.validate() {
            return error.into_http();
        }
        let codec = match Codec::<
            InputEnvelope<O, <O::Input as crate::operations::Direction>::Item>,
            Event<
                StreamItem<<O::Output as crate::operations::Direction>::Item>,
                crate::operations::Response<O::Response>,
            >,
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
    // Preserve Known Runtime Denials Through Authored Child Failures. Bound Wrapping.
    let mut error = error;
    let mut wrapped = false;
    for _ in 0..32 {
        match error {
            Error::TaskFailed { source, .. } | Error::OperationFailed { source, .. } => {
                if let Some(status) = source.downcast_ref::<Status>() {
                    return status.code();
                }
                if let Some(inner) = source
                    .downcast_ref::<Error>()
                    .or_else(|| ungranted(source.as_ref()))
                {
                    error = inner;
                    wrapped = true;
                } else {
                    return tonic::Code::Internal;
                }
            }
            _ => {
                if !wrapped {
                    return leaf_code(error);
                }
                // Authored Application Failures Stay Internal Unless They Carry
                // An Explicit Status Or A Known Runtime Authority/Lifecycle Denial.
                return match error {
                    Error::OperationRefused(Refusal::TimedOut | Refusal::Cancelled)
                    | Error::Unauthenticated
                    | Error::PermissionDenied
                    | Error::OperationUnauthorized { .. } => leaf_code(error),
                    #[cfg(any(
                        feature = "plugin",
                        feature = "plugin-guest",
                        feature = "plugin-sidecar"
                    ))]
                    Error::OperationCaller { .. } => leaf_code(error),
                    _ => tonic::Code::Internal,
                };
            }
        }
    }
    tonic::Code::Internal
}

fn leaf_code(error: &Error) -> tonic::Code {
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
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        Error::OperationCaller { .. } => tonic::Code::Unauthenticated,
        _ => tonic::Code::Internal,
    }
}
fn explicit_status(mut error: &Error) -> Option<Status> {
    for _ in 0..32 {
        match error {
            Error::TaskFailed { source, .. } | Error::OperationFailed { source, .. } => {
                if let Some(status) = source.downcast_ref::<Status>() {
                    return Some(status.clone());
                }
                #[cfg(feature = "plugin-sidecar")]
                if let Some(status) = refused_request(source.as_ref()) {
                    return Some(status);
                }
                error = source.downcast_ref::<Error>()?;
            }
            _ => return None,
        }
    }
    None
}

// An Encoded Provider's Prepared Grant The Caller Lacks Keeps Its Runtime Denial.
fn ungranted<'a>(source: &'a (dyn std::error::Error + Send + Sync + 'static)) -> Option<&'a Error> {
    #[cfg(feature = "plugin-sidecar")]
    if let Some(crate::operations::NativeError::Unauthorized(inner)) = source.downcast_ref() {
        return Some(inner);
    }
    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    if let Some(crate::operations::ComponentError::Unauthorized(inner)) = source.downcast_ref() {
        return Some(inner);
    }
    let _ = source;
    None
}

// A Sidecar That Rejects The Request During Preparation (An Unreadable Or
// Wrong-Shape Payload) Names The Caller's Fault In Its Own Words.
#[cfg(feature = "plugin-sidecar")]
fn refused_request(source: &(dyn std::error::Error + Send + Sync + 'static)) -> Option<Status> {
    let crate::operations::NativeError::Preparation(failure) = source.downcast_ref()? else {
        return None;
    };
    if failure.code() != crate::operations::FailureKind::Invalid {
        return None;
    }

    Some(Status::invalid_argument(failure.cause()))
}

// A carried plugin failure reaches a remote client as the same structured
// value in the status details. Origin diagnostics stay inside the host: chains
// and backtraces are not disclosed to network callers.
fn with_failure(status: Status, error: &Error) -> Status {
    let Some(failure) = error.failure() else {
        return status;
    };
    if !status.details().is_empty() {
        return status;
    }

    let details = failure.clone().without_diagnostics().encode();
    Status::with_details_and_metadata(
        status.code(),
        status.message(),
        details.into(),
        status.metadata().clone(),
    )
}

pub(super) fn status(error: Error) -> Status {
    let status = explicit_status(&error)
        .unwrap_or_else(|| Status::new(code(&error), "operation request failed"));
    let mut status = with_failure(status, &error);
    status.set_source(Arc::new(error));
    status
}
fn input_status(error: InputError) -> Status {
    let explicit = match &error {
        InputError::Operation(error) => explicit_status(error),
        InputError::Forward { source, settlement }
            if !settlement.as_deref().is_some_and(|error| {
                matches!(
                    error,
                    Error::OperationFinalization { .. } | Error::OperationSettlement { .. }
                )
            }) =>
        {
            explicit_status(source)
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
    let status = Status::new(code, "operation input failed");
    let mut status = with_failure(status, failed);
    status.set_source(Arc::new(error));
    status
}

/* === Tests === */
