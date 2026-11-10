/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc};

// Third Party Imports

// Local Imports
use crate::{Context, Message, Result, Visibility};
use harmos_protocol::v1;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/* === Structs === */

pub struct Operation {
    pub(crate) contract: v1::OperationContract,
    pub(crate) types: prost_types::FileDescriptorSet,
    pub(crate) handler: Arc<dyn Handler>,
}

struct TypedHandler<Q, R, H> {
    handler: H,
    marker: std::marker::PhantomData<fn(Q) -> R>,
}

/* === Enums === */

/* === Traits === */

pub(crate) trait Handler: Send + Sync {
    fn validate(&self, payload: &[u8]) -> std::result::Result<(), tonic::Status>;
    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>>;
}

/* === Implementations === */

impl Operation {
    pub fn new<Q, R, H>(id: impl Into<String>, handler: H) -> Self
    where
        Q: Message,
        R: Message,
        H: for<'a> AsyncFn(&'a Context, Q) -> Result<R> + Send + Sync + 'static,
    {
        let id = id.into();
        let mut types = Q::types();
        crate::message::merge_descriptor_types(&mut types, R::types());
        Self {
            contract: v1::OperationContract {
                id,
                request: Q::name(),
                response: R::name(),
                visibility: Visibility::Private as i32,
                ..Default::default()
            },
            types,
            handler: Arc::new(TypedHandler {
                handler,
                marker: std::marker::PhantomData,
            }),
        }
    }

    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.contract.visibility = visibility as i32;
        self
    }
    pub fn policy(mut self, policy: v1::OperationPolicy) -> Self {
        self.contract.policy = Some(policy);
        self
    }
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.contract.description = description.into();
        self
    }
}

impl<Q, R, H> Handler for TypedHandler<Q, R, H>
where
    Q: Message,
    R: Message,
    H: for<'a> AsyncFn(&'a Context, Q) -> Result<R> + Send + Sync + 'static,
{
    fn validate(&self, payload: &[u8]) -> std::result::Result<(), tonic::Status> {
        Q::decode(payload).map(|_| ()).map_err(|error| tonic::Status::invalid_argument(error.to_string()))
    }

    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let request = Q::decode(payload.as_slice()).map_err(|error| tonic::Status::invalid_argument(error.to_string()))?;
            Ok((self.handler)(context, request).await?.encode_to_vec())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
