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
// declared operations through `call` and `submit`, never as per-kit methods.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{any::Any, future::Future, sync::Arc};

// Third Party Imports

// Local Imports
use super::instance::Instance;
use super::{CallContext, ContextError, ContextValue, OperationDeclaration, Response};
use crate::operations::streams::Publish;
use crate::{Message, StreamSpec};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Native Ownership Is Never Part Of A Serialized Operation Declaration.
#[derive(Clone)]
pub struct RequestClaims {
    owner: Arc<dyn Any + Send + Sync>,
}

/* === Enums === */

/* === Traits === */

// The host one authored body calls through wherever it runs. The request is
// the operation, so `call` and `submit` infer the declaration from it.
pub trait Host: Send + Sync + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    type Instance<O: OperationDeclaration>: Instance<O, Error = Self::Error>;

    fn context(&self) -> &CallContext;

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError>;

    fn submit<O: OperationDeclaration>(&self, request: O) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    // The caller admits the owner before allocating the request payload. Native
    // hosts retain it with accepted work; remote hosts refuse local ownership.
    fn submit_owned<O: OperationDeclaration>(
        &self, request: O, claims: RequestClaims,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    fn call_owned<O: OperationDeclaration>(
        &self, request: O, claims: RequestClaims,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        async move { self.submit_owned(request, claims).await?.join().await }
    }

    fn call<O: OperationDeclaration>(&self, request: O) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send;

    // Activate a resident declaration (an observer or a service) over
    // caller-owned resources. Activations sharing the same resources share one
    // instance and each handle holds its own lease; releasing the last lease
    // stops it. Bindings that cannot carry process-local resources refuse.
    fn submit_with_resources<O: OperationDeclaration<Response = ()>, R: Send + Sync + 'static>(
        &self, request: O, resources: std::sync::Arc<R>,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    // Publish one row on a declared stream through the host's `host.publish`.
    fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        S::Row: Message,
    {
        let published = self.call(Publish {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: None,
            published_unix_micros: None,
            context: None,
            payload: row.encode_to_vec(),
            following: Vec::new(),
        });
        async move {
            published.await?;
            Ok(())
        }
    }

    fn publish<S: StreamSpec<Row = S> + Message>(&self, row: &S) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.publish_on::<S>(row)
    }
}

/* === Implementations === */

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
