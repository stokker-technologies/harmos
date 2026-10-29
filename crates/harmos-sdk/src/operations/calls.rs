/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::response::ErasedResponse;
use super::{CacheKey, OperationDeclaration, OperationDefinition, Response};
use crate::{Message, MessageError};

pub type CallFuture<'a> = Pin<Box<dyn Future<Output = Result<ErasedResponse, InvocationError>> + Send + 'a>>;

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
    pub grant: fn(&(dyn Any + Send)) -> Option<crate::plugin::Grant>,
    pub key: fn(&(dyn Any + Send)) -> Option<CacheKey>,
}

#[derive(Debug, thiserror::Error)]
pub enum InvocationError {
    #[error("operation `{operation}` cannot be invoked from this carrier")]
    Unavailable { operation: &'static str },

    #[error("operation `{operation}` answered a response of another type")]
    Response { operation: &'static str },

    #[error("operation `{operation}` failed: {source}")]
    Failed { operation: &'static str, source: Arc<dyn std::error::Error + Send + Sync> },
}

// Carriers Route A Declared Child Operation With The Calling Attempt's Metadata.
pub trait Caller: Send + Sync + 'static {
    fn call(&self, target: Target, request: Box<dyn Any + Send>) -> CallFuture<'_>;
}

impl Target {
    pub fn of<T: OperationDeclaration>() -> Self {
        Self {
            id: T::ID,
            definition: T::definition,
            request: TypeId::of::<T::Request>,
            memory: |request| request.downcast_ref::<T::Request>().map_or(0, crate::MemorySize::memory_size),
            response: TypeId::of::<T::Response>,
            encode: |request| request.downcast_ref::<T::Request>().map(Message::encode),
            decode: decode::<T>,
            grant: |request| request.downcast_ref::<T::Request>().and_then(T::grant),
            key: |request| request.downcast_ref::<T::Request>().and_then(T::key),
        }
    }
}

impl InvocationError {
    pub fn failed(operation: &'static str, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Failed { operation, source: Arc::new(source) }
    }
}

fn decode<T: OperationDeclaration>(bytes: &[u8]) -> Result<ErasedResponse, MessageError> {
    let response = T::Response::decode(bytes)?;
    Ok(Response::new(response).into_erased())
}
