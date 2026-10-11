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
use crate::operations::{GrpcVisitor, InputDeclaration, OutputDeclaration, WireOperation};
use crate::{MemorySize, Protobuf};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Handler =
    fn(server::Authority, server::Settings, http::Request<Body>) -> server::HttpFuture;

/* === Structs === */
pub(crate) struct Project;

#[derive(Clone)]
pub(super) struct Router {
    operations: crate::OperationManager,
    settings: server::Settings,
    calls: server::Calls,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Router {
    pub(super) fn new(
        operations: crate::OperationManager,
        settings: server::Settings,
    ) -> Result<Self, tonic::Status> {
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
                return tonic::Status::unimplemented("operation provider has no gRPC adapter")
                    .into_http();
            };
            handler(
                server::Authority::Binding(binding),
                settings,
                request.map(Body::new),
            )
            .await
        }));
        Box::pin(async move { Ok(pending.await) })
    }
}

impl GrpcVisitor for Project {
    type Entry = Handler;
    fn unary<O: WireOperation>(self) -> Handler
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
    {
        server::unary::<O>
    }
    fn upload<O: WireOperation + InputDeclaration>(self) -> Handler
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        O::Item: Protobuf + MemorySize,
    {
        server::upload::<O>
    }
    fn download<O: WireOperation + OutputDeclaration>(self) -> Handler
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        O::Item: Protobuf + MemorySize,
    {
        server::download::<O>
    }
    fn exchange<O: WireOperation + InputDeclaration + OutputDeclaration>(self) -> Handler
    where
        O::Request: Protobuf + MemorySize,
        O::Response: Protobuf,
        <O as InputDeclaration>::Item: Protobuf + MemorySize,
        <O as OutputDeclaration>::Item: Protobuf + MemorySize,
    {
        server::exchange::<O>
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
