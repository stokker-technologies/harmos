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
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

// Third Party Imports
use tokio::sync::watch;
use tonic::{
    Code, Status,
    body::Body,
    codegen::{Service, http},
    metadata::MetadataMap,
};
use tracing::Instrument;

// Local Imports
use super::middleware::{Middleware, Session};
use crate::operations::CallContext;
use crate::supervision::executor::InContext;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Response = http::Response<Body>;
type Shutdown = Pin<Box<dyn Future<Output = ()> + Send>>;

/* === Structs === */

#[derive(Clone)]
pub(super) struct Inbound {
    pub(super) routes: tonic::service::Routes,
    pub(super) middleware: Arc<[Arc<dyn Middleware>]>,
    pub(super) timeout: Duration,
    pub(super) stopping: watch::Receiver<bool>,
}

#[derive(Clone)]
pub(super) struct TrustedContext(pub(super) CallContext);

struct Observed {
    trace: Option<tracing::Span>,
    dispatcher: tracing::Dispatch,
    body: Body,
    session: Session,
    terminal: Option<Code>,
    deadline: Pin<Box<tokio::time::Sleep>>,
    stopping: Shutdown,
    ended: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Service<http::Request<Body>> for Inbound {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        <tonic::service::Routes as Service<http::Request<Body>>>::poll_ready(&mut self.routes, context)
    }

    #[tracing::instrument(target = "harmos::grpc", name = "grpc.call", level = "debug", skip_all, fields(status = tracing::field::Empty))]
    fn call(&mut self, mut request: http::Request<Body>) -> Self::Future {
        let trace = tracing::Span::current();
        let mut routes = self.routes.clone();
        let entries = self.middleware.clone();
        let timeout = self.timeout;
        let stopping = self.stopping.clone();
        Box::pin(
            async move {
                if *stopping.borrow() {
                    return Ok(Status::unavailable("gRPC listener is stopping").into_http());
                }
                let timeout = match request_timeout(request.headers(), timeout) {
                    Ok(timeout) => timeout,
                    Err(status) => return Ok(status.into_http()),
                };
                let deadline = tokio::time::Instant::now() + timeout;
                let metadata = MetadataMap::from_headers(request.headers().clone());
                let peer = request.extensions().get::<tonic::transport::server::TcpConnectInfo>().and_then(|info| info.remote_addr);

                let peer = request
                    .extensions()
                    .get::<tonic::transport::server::TlsConnectInfo<tonic::transport::server::TcpConnectInfo>>()
                    .and_then(|info| info.get_ref().remote_addr)
                    .or(peer);
                let mut session = Session::new(request.uri().path().to_owned(), metadata, request.extensions().clone(), peer, entries);
                let entered = tokio::select! {
                    biased;
                    _ = shutdown(stopping.clone()) => Err(Status::unavailable("gRPC listener is stopping")),
                    _ = tokio::time::sleep_until(deadline) => Err(Status::deadline_exceeded("RPC deadline elapsed")),
                    result = session.enter() => result.map_err(super::server::status),
                };
                if let Err(status) = entered {
                    session.observe(status.code());
                    return Ok(status.into_http());
                }
                let mut context = session.context();
                context.insert(crate::operations::context::Deadline(deadline)).expect("transport owns the deadline type");
                request.extensions_mut().insert(TrustedContext(context));
                let response = tokio::select! {
                    biased;
                    _ = shutdown(stopping.clone()) => {
                        session.observe(Code::Unavailable);
                        return Ok(Status::unavailable("gRPC listener is stopping").into_http());
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        session.observe(Code::DeadlineExceeded);
                        return Ok(Status::deadline_exceeded("RPC deadline elapsed").into_http());
                    }
                    response = routes.call(request) => response.expect("router is infallible"),
                };
                let terminal = status(response.headers());
                Ok(response.map(|body| {
                    use http_body::Body as _;
                    if body.is_end_stream() {
                        session.observe(terminal.unwrap_or(Code::Unknown));
                        return body;
                    }
                    Body::new(Observed {
                        trace: Some(tracing::Span::current()),
                        dispatcher: tracing::dispatcher::get_default(Clone::clone),
                        body,
                        session,
                        terminal,
                        deadline: Box::pin(tokio::time::sleep_until(deadline)),
                        stopping: Box::pin(shutdown(stopping)),
                        ended: false,
                    })
                }))
            }
            .instrument(trace)
            .in_current_context(),
        )
    }
}

impl Drop for Observed {
    fn drop(&mut self) {
        let span = self.trace.take();
        let body = std::mem::replace(&mut self.body, Body::empty());
        tracing::dispatcher::with_default(&self.dispatcher, || drop((span, body)));
    }
}

impl http_body::Body for Observed {
    type Data = bytes::Bytes;
    type Error = Status;
    fn poll_frame(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<http_body::Frame<Self::Data>, Status>>> {
        let dispatcher = self.dispatcher.clone();
        let _dispatcher = tracing::dispatcher::set_default(&dispatcher);
        let trace = self.trace.clone().unwrap_or_else(tracing::Span::none);
        let _trace = trace.enter();
        if self.ended {
            return Poll::Ready(None);
        }
        if self.stopping.as_mut().poll(cx).is_ready() {
            self.session.observe(Code::Unavailable);
            self.ended = true;
            self.trace.take();
            self.body = Body::empty();
            return Poll::Ready(Some(Ok(trailers(Status::unavailable("gRPC listener is stopping")))));
        }
        if self.deadline.as_mut().poll(cx).is_ready() {
            self.session.observe(Code::DeadlineExceeded);
            self.ended = true;
            self.trace.take();
            self.body = Body::empty();
            return Poll::Ready(Some(Ok(trailers(Status::deadline_exceeded("RPC deadline elapsed")))));
        }
        match Pin::new(&mut self.body).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(trailers) = frame.trailers_ref() {
                    self.ended = true;
                    self.trace.take();
                    self.terminal = status(trailers);
                    let code = self.terminal.unwrap_or(Code::Unknown);
                    trace.record("status", tracing::field::debug(code));
                    self.session.observe(code);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                self.ended = true;
                self.trace.take();
                self.session.observe(error.code());
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                self.ended = true;
                self.trace.take();
                let code = self.terminal.unwrap_or(Code::Unknown);
                trace.record("status", tracing::field::debug(code));
                self.session.observe(code);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn trailers(status: Status) -> http_body::Frame<bytes::Bytes> {
    let mut headers = http::HeaderMap::new();
    status.add_header(&mut headers).expect("static transport status is valid metadata");
    http_body::Frame::trailers(headers)
}

async fn shutdown(mut stopping: watch::Receiver<bool>) {
    while !*stopping.borrow_and_update() {
        if stopping.changed().await.is_err() {
            return;
        }
    }
}

fn status(headers: &http::HeaderMap) -> Option<Code> {
    headers.get("grpc-status").and_then(|value| value.to_str().ok()).and_then(|value| value.parse::<i32>().ok()).map(Code::from_i32)
}

fn request_timeout(headers: &http::HeaderMap, maximum: Duration) -> Result<Duration, Status> {
    let Some(value) = headers.get("grpc-timeout") else {
        return Ok(maximum);
    };
    let value = value.to_str().map_err(|_| Status::invalid_argument("invalid grpc-timeout"))?;
    if !(2..=9).contains(&value.len()) || !value[..value.len() - 1].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Status::invalid_argument("invalid grpc-timeout"));
    }
    let amount = value[..value.len() - 1].parse::<u64>().map_err(|_| Status::invalid_argument("invalid grpc-timeout"))?;
    let duration = match value.as_bytes()[value.len() - 1] {
        b'H' => Duration::from_secs(amount * 3600),
        b'M' => Duration::from_secs(amount * 60),
        b'S' => Duration::from_secs(amount),
        b'm' => Duration::from_millis(amount),
        b'u' => Duration::from_micros(amount),
        b'n' => Duration::from_nanos(amount),
        _ => return Err(Status::invalid_argument("invalid grpc-timeout")),
    };
    Ok(duration.min(maximum))
}

/* === Tests === */
