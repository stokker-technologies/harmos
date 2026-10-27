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
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

// Third Party Imports
use tonic::{body::Body, codegen::http};

// Local Imports
use super::server;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Handler = fn(server::Authority, server::Settings, http::Request<Body>) -> server::HttpFuture;

/* === Structs === */

#[derive(Clone)]
pub(super) struct Router {
    operations: crate::operations::Manager,
    settings: server::Settings,
    calls: server::Calls,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Router {
    pub(super) fn new(operations: crate::operations::Manager, settings: server::Settings) -> Result<Self, tonic::Status> {
        settings.validate()?;
        Ok(Self {
            operations,
            settings,
            calls: server::Calls::new(settings.calls)?,
        })
    }
}

impl<B> tonic::codegen::Service<http::Request<B>> for Router
where
    B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError>,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let binding = self.operations.grpc_binding(request.uri().path());
        let settings = self.settings;
        let pending = self.calls.run(Box::pin(async move {
            let binding = match binding {
                Ok(binding) => binding,
                Err(crate::Error::NotRegistered { .. }) => {
                    return tonic::Status::unimplemented("unknown operation method").into_http();
                }
                Err(error) => return server::status(error).into_http(),
            };
            let Some(handler) = binding.handler() else {
                return tonic::Status::unimplemented("operation provider has no gRPC adapter").into_http();
            };
            handler(server::Authority::Binding(binding), settings, request.map(Body::new)).await
        }));
        Box::pin(async move { Ok(pending.await) })
    }
}

/* === Public Functions === */

// The gRPC handler of one declaration, selected by the directions it streams.
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
pub(crate) fn handler<O: crate::operations::OperationDeclaration>() -> Handler {
    use crate::operations::Direction;
    match (<O::Input as Direction>::STREAMED, <O::Output as Direction>::STREAMED) {
        (false, false) => server::unary::<O, super::Transcoded>,
        (true, false) => server::upload::<O, super::Transcoded>,
        (false, true) => server::download::<O, super::Transcoded>,
        (true, true) => server::exchange::<O, super::Transcoded>,
    }
}

/* === Private Functions === */

/* === Tests === */
