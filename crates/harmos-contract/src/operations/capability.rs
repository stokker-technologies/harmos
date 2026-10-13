/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The host capability one authored body uses wherever it runs: in process over
// the operation manager, in a sidecar over the connection frames, or in a guest
// over the Wasm imports. Storage and other host services are reached as
// declared operations through `call` and `submit`, never as per-kit methods.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;

// Third Party Imports

// Local Imports
use super::host::{Publication, Publish};
use super::instance::Instance;
use super::{CallContext, ContextError, ContextValue, OperationDeclaration, Response};
use crate::{Message, StreamSpec};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
// An operation whose request and response cross every binding's codec.
pub trait Callable: OperationDeclaration<Request: Message, Response: Message> {}

// `call` and `submit` take the declaration itself as the request when the
// declaration is its own request (`Request = Self`); `call_as::<O>` and
// `submit_as::<O>` name the operation for a separate request type.
pub trait Host: Send + Sync + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    type Instance<O: Callable>: Instance<O, Error = Self::Error>;

    fn context(&self) -> &CallContext;

    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError>;

    fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send;

    fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send;

    fn submit<O: Callable<Request = O>>(
        &self,
        request: O,
    ) -> impl Future<Output = Result<Self::Instance<O>, Self::Error>> + Send {
        self.submit_as::<O>(request)
    }

    fn call<O: Callable<Request = O>>(
        &self,
        request: O,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        self.call_as::<O>(request)
    }

    // Publish one row on a declared stream through the host's `host.publish`.
    fn publish_on<S: StreamSpec>(
        &self,
        row: &S::Row,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        S::Row: Message,
    {
        let publication = Publication {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: None,
            published_unix_micros: None,
            context: None,
            payload: row.encode_to_vec(),
        };
        let published = self.call_as::<Publish>(publication);
        async move {
            published.await?;
            Ok(())
        }
    }

    fn publish<S: StreamSpec<Row = S> + Message>(
        &self,
        row: &S,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.publish_on::<S>(row)
    }
}

/* === Implementations === */
impl<O> Callable for O
where
    O: OperationDeclaration,
    O::Request: Message,
    O::Response: Message,
{
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
