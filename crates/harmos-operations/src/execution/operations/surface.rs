/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The host one authored body uses wherever it runs: in process over
// the operation manager, in a sidecar over the connection frames, or in a guest
// over the Wasm imports. Storage and other host services are reached as
// declared operations through the shared invocation and instance contract.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    any::Any,
    future::{Future, IntoFuture},
    sync::Arc,
};

// Third Party Imports

// Local Imports
use super::instance::Instance;
use super::{CallContext, ContextError, ContextValue, EndpointSet, RequestShape, Response, RetryPolicy};
use crate::execution::{Message, StreamSpec};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Native Ownership Is Never Part Of A Serialized Operation Declaration.
#[derive(Clone)]
pub struct RequestClaims {
    owner: Arc<dyn Any + Send + Sync>,
}

#[must_use = "an invocation runs when awaited or started"]
pub struct Invocation<H: OperationHost, O: RequestShape, E: EndpointSet = ()> {
    host: H,
    request: O::Request,
    retry: Option<RetryPolicy>,
    endpoints: E,
}

/* === Enums === */

/* === Traits === */

// The Carrier OperationHost Preserves Authorization For Child Calls.
pub trait OperationHost: Send + Sync + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    type Instance<O: RequestShape>: Instance<O, Error = Self::Error>;

    fn operation<O: RequestShape<Request = O>>(&self, request: O) -> Invocation<Self, O>
    where
        Self: Clone,
    {
        Invocation::new(self.clone(), request)
    }

    fn invoke<O: RequestShape, E: EndpointSet>(
        &self, request: O::Request, retry: Option<RetryPolicy>, endpoints: E,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    fn resolve<O: RequestShape, E: EndpointSet>(
        &self, request: O::Request, retry: Option<RetryPolicy>, endpoints: E,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        async move { self.invoke::<O, E>(request, retry, endpoints).await?.join().await }
    }

    fn context(&self) -> &CallContext;

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError>;

    fn submit<O: RequestShape>(&self, request: O::Request) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    // The caller admits the owner before allocating the request payload. Native
    // hosts retain it with accepted work; remote hosts refuse local ownership.
    fn submit_owned<O: RequestShape>(
        &self, request: O::Request, claims: RequestClaims,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    fn call_owned<O: RequestShape>(
        &self, request: O::Request, claims: RequestClaims,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        async move { self.submit_owned::<O>(request, claims).await?.join().await }
    }

    // Publish one row on a declared stream through the host's `harmos.context.v1.Samples/Append`.
    fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        S::Row: Message;

    fn publish<S: StreamSpec<Row = S> + Message>(&self, row: &S) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.publish_on::<S>(row)
    }
}

/* === Implementations === */

impl<H: OperationHost, O: RequestShape> Invocation<H, O> {
    pub fn new(host: H, request: O::Request) -> Self {
        Self {
            host,
            request,
            retry: None,
            endpoints: (),
        }
    }
}

impl<H: OperationHost, O: RequestShape, E: EndpointSet> Invocation<H, O, E> {
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    pub fn endpoints<F: EndpointSet>(self, endpoints: F) -> Invocation<H, O, F> {
        Invocation {
            host: self.host,
            request: self.request,
            retry: self.retry,
            endpoints,
        }
    }

    #[doc(hidden)]
    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub async fn start(self) -> Result<H::Instance<O>, H::Error> {
        self.host.invoke::<O, E>(self.request, self.retry, self.endpoints).await
    }
}

impl<H: OperationHost + 'static, O: RequestShape, E: EndpointSet> IntoFuture for Invocation<H, O, E> {
    type Output = Result<Response<O::Response>, H::Error>;
    type IntoFuture = std::pin::Pin<Box<dyn Future<Output = Self::Output> + Send>>;
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.host.resolve::<O, E>(self.request, self.retry, self.endpoints).await })
    }
}

impl RequestClaims {
    pub fn retain(owner: Arc<dyn Any + Send + Sync>) -> Self {
        Self { owner }
    }

    pub fn owner<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.owner.downcast_ref()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
