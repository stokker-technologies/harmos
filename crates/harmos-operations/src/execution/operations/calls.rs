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
use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::response::ErasedResponse;
use super::{OperationDeclaration, OperationDefinition, Response};
use crate::execution::{Message, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type StartFuture<'a> = Pin<Box<dyn Future<Output = Result<Box<dyn ChildInstance>, InvocationError>> + Send + 'a>>;

pub type ControlFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, InvocationError>> + Send + 'a>>;

pub type CallFuture<'a> = Pin<Box<dyn Future<Output = Result<ErasedResponse, InvocationError>> + Send + 'a>>;

/* === Structs === */

// The Carrier Caller Of One Attempt, Inserted Into Its Execution Snapshot.
#[derive(Clone)]
pub struct Calls(pub Arc<dyn Caller>);

// A Child Operation Erased For Carrier Dispatch; Every Function Is Monomorphized For The Callee.
#[derive(Clone, Copy)]
pub struct Target {
    pub id: &'static str,
    pub definition: fn() -> OperationDefinition,
    pub request: fn() -> TypeId,
    pub memory: fn(&(dyn Any + Send)) -> usize,
    pub response: fn() -> TypeId,
    pub encode: fn(&(dyn Any + Send)) -> Option<Vec<u8>>,
    pub decode: fn(&[u8]) -> Result<ErasedResponse, MessageError>,
    pub grant: fn(&(dyn Any + Send)) -> Option<crate::execution::plugin::Grant>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum InvocationError {
    #[error("operation `{operation}` cannot be invoked from this carrier")]
    Unavailable { operation: &'static str },

    #[error("operation `{operation}` does not support these invocation options")]
    Options { operation: &'static str },

    #[error("operation `{operation}` answered a response of another type")]
    Response { operation: &'static str },

    #[error("operation `{operation}` failed: {source}")]
    Failed { operation: &'static str, source: Arc<dyn std::error::Error + Send + Sync> },
}

/* === Traits === */

pub trait ChildInstance: Send {
    fn id(&self) -> super::instance::Id;
    fn status(&self) -> ControlFuture<'_, super::instance::Status>;
    fn request_cancel(&self) -> ControlFuture<'_, ()>;
    fn join(self: Box<Self>) -> CallFuture<'static>;
    fn detach(self: Box<Self>) -> super::instance::Id;
}

// Carriers Route A Declared Child Operation With The Calling Attempt's Metadata.
pub trait Caller: Send + Sync + 'static {
    fn start(
        &self, target: Target, request: Box<dyn Any + Send>, retry: Option<super::RetryPolicy>, endpoints: Box<dyn Any + Send>,
        context: super::CallContext,
    ) -> StartFuture<'_>;
}

/* === Implementations === */

impl Target {
    pub fn of<T: OperationDeclaration>() -> Self {
        Self {
            id: T::ID,
            definition: T::definition,
            request: TypeId::of::<T::Request>,
            memory: |request| request.downcast_ref::<T::Request>().map_or(0, crate::execution::MemorySize::memory_size),
            response: TypeId::of::<T::Response>,
            encode: |request| request.downcast_ref::<T::Request>().map(Message::encode),
            decode: decode::<T>,
            grant: |request| request.downcast_ref::<T::Request>().and_then(T::grant),
        }
    }
}

impl From<InvocationError> for crate::execution::Refusal {
    fn from(error: InvocationError) -> Self {
        let operation = match &error {
            InvocationError::Unavailable { operation }
            | InvocationError::Response { operation }
            | InvocationError::Options { operation }
            | InvocationError::Failed { operation, .. } => *operation,
        };
        let failure =
            crate::execution::Failure::originate(super::FailureKind::Permanent, &error, crate::execution::failure::Origin::Plugin)
                .in_operation(operation);
        Self::failed(failure.refusal_code().unwrap_or(crate::execution::FAILED), failure)
    }
}

impl InvocationError {
    pub fn failed(operation: &'static str, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Failed { operation, source: Arc::new(source) }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn decode<T: OperationDeclaration>(bytes: &[u8]) -> Result<ErasedResponse, MessageError> {
    let response = T::Response::decode(bytes)?;
    Ok(Response::new(response).into_erased())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_conversion_preserves_child_failure_and_code() {
        let original = crate::execution::Failure::new(super::super::FailureKind::Transient, "transport failed").in_operation("child");
        let child = crate::execution::Refusal::failed(-32_100, original.clone());
        let forwarded = crate::execution::Refusal::from(InvocationError::failed("child", child));
        assert_eq!(forwarded.code(), -32_100);
        let failure = forwarded.failure();
        assert_eq!(failure.code(), original.code());
        assert_eq!(failure.cause(), original.cause());
        assert_eq!(failure.context().operation(), Some("child"));
    }
}
