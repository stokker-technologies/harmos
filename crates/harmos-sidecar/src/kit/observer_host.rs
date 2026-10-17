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
use std::future::Future;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{CallError, ObserverContext, PublicationError, Supervisor};
use harmos_domain::{Message, StreamSpec, inventory::publications};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub trait ObserverHost<R>: Send + Sync + Sized + 'static {
    fn bind(resources: Arc<R>, supervisor: Supervisor) -> Self;
}

pub trait ObserverPublisher<R: Sync, V: Send>: Send + Sync + Sized {
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;
    const STREAM: Option<publications::Entry> = None;

    fn publish(
        &self,
        context: &ObserverContext<'_, R, Self>,
        value: V,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait ObserverStreamPublisher<R: Sync, S: StreamSpec>: Send + Sync + Sized
where
    S::Row: Sync,
{
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;

    fn publish_on(
        &self,
        context: &ObserverContext<'_, R, Self>,
        row: &S::Row,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait ObserverCaller<R: Sync, C>: Send + Sync + Sized {
    type Request: Send;
    type Response: Send;
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(
        &self,
        context: &ObserverContext<'_, R, Self>,
        request: Self::Request,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send;
}

/* === Implementations === */
impl<R> ObserverHost<R> for Supervisor {
    fn bind(_: Arc<R>, supervisor: Supervisor) -> Self {
        supervisor
    }
}

impl<R: Sync, C> ObserverCaller<R, C> for Supervisor
where
    C: harmos_domain::operations::OperationDeclaration,
    C::Request: Message,
    C::Response: Message,
{
    type Request = C::Request;
    type Response = C::Response;
    type Error = CallError;

    async fn call(
        &self,
        _: &ObserverContext<'_, R, Self>,
        request: Self::Request,
    ) -> Result<Self::Response, Self::Error> {
        let bytes = self.child_call::<C>(&request, false).await?;
        C::Response::decode(bytes.as_slice()).map_err(|error| CallError::Malformed {
            method: C::ID,
            message: error.to_string(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
