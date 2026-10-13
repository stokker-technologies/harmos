/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Browser Origins And Cross-Origin Handling For The gRPC-Web Listener

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

// Third Party Imports
use futures_util::{
    FutureExt,
    future::{self, Either, Map, Ready},
};
use tonic::codegen::http::{HeaderValue, uri::Authority};
use tonic::{
    body::Body,
    codegen::http::{HeaderName, Method, Request, Response, StatusCode, header},
};
use tower::{Layer, Service};
use tower_http::cors::{AllowOrigin, CorsLayer};

// Local Imports
use crate::{Error, Result};

/* === Constants === */
const MAX_AGE: Duration = Duration::from_secs(600);

/* === Macros === */

/* === Type Aliases === */
type Accepted<E> =
    fn(std::result::Result<Response<Body>, E>) -> std::result::Result<Response<Body>, E>;

/* === Structs === */
// An exact browser origin such as `tauri://localhost`: scheme and authority,
// with no path, credentials or wildcard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin(HeaderValue);

// Refuses cross-origin requests from origins outside the allowed set, then
// answers preflights and marks responses for the allowed ones.
#[derive(Clone)]
pub(super) struct CrossOrigin {
    origins: Arc<[HeaderValue]>,
}

#[derive(Clone)]
pub(super) struct Gate<S> {
    origins: Arc<[HeaderValue]>,
    inner: S,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Origin {
    pub fn parse(origin: &str) -> Result<Self> {
        let invalid = |reason| Error::InvalidInput {
            field: "gRPC allowed origin",
            reason,
        };
        let (scheme, authority) = origin
            .split_once("://")
            .ok_or_else(|| invalid("an origin is a scheme and an authority"))?;
        let scheme_valid = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        let authority_valid = !authority.contains(['/', '?', '#', '@', '*'])
            && authority.parse::<Authority>().is_ok();
        if !scheme_valid || !authority_valid {
            return Err(invalid("an origin has no path, credentials or wildcard"));
        }
        let value = HeaderValue::from_str(origin)
            .map_err(|_| invalid("an origin is a visible ASCII string"))?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.to_str().expect("origin is visible ASCII")
    }
}

impl std::str::FromStr for Origin {
    type Err = Error;

    fn from_str(origin: &str) -> Result<Self> {
        Self::parse(origin)
    }
}

impl CrossOrigin {
    pub(super) fn new(allowed: &[Origin]) -> Option<Self> {
        (!allowed.is_empty()).then(|| Self {
            origins: allowed.iter().map(|origin| origin.0.clone()).collect(),
        })
    }
}

impl<S> Layer<S> for CrossOrigin {
    type Service = Gate<tower_http::cors::Cors<S>>;

    fn layer(&self, inner: S) -> Self::Service {
        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::list(self.origins.iter().cloned()))
            .allow_methods([Method::POST, Method::OPTIONS])
            .allow_headers([
                header::CONTENT_TYPE,
                HeaderName::from_static("x-grpc-web"),
                HeaderName::from_static("x-user-agent"),
                HeaderName::from_static("grpc-timeout"),
                header::AUTHORIZATION,
            ])
            .expose_headers([
                HeaderName::from_static("grpc-status"),
                HeaderName::from_static("grpc-message"),
                HeaderName::from_static("grpc-status-details-bin"),
            ])
            .max_age(MAX_AGE);
        Gate {
            origins: self.origins.clone(),
            inner: cors.layer(inner),
        }
    }
}

impl<S, B> Service<Request<B>> for Gate<S>
where
    S: Service<Request<B>, Response = Response<Body>>,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Either<
        Ready<std::result::Result<Response<Body>, S::Error>>,
        Either<Map<S::Future, Accepted<S::Error>>, S::Future>,
    >;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<std::result::Result<(), S::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let refused = request
            .headers()
            .get(header::ORIGIN)
            .is_some_and(|origin| !self.origins.contains(origin));
        if refused {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::FORBIDDEN;
            return Either::Left(future::ready(Ok(response)));
        }
        // The CORS layer answers preflights with 200; browsers accept either.
        let preflight = request.method() == Method::OPTIONS
            && request
                .headers()
                .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
        let future = self.inner.call(request);
        if preflight {
            Either::Right(Either::Left(future.map(no_content as Accepted<S::Error>)))
        } else {
            Either::Right(Either::Right(future))
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn no_content<E>(
    result: std::result::Result<Response<Body>, E>,
) -> std::result::Result<Response<Body>, E> {
    result.map(|mut response| {
        *response.status_mut() = StatusCode::NO_CONTENT;
        response
    })
}

/* === Tests === */
