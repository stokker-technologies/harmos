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

// Third Party Imports

// Local Imports
use super::{FailureKind, OperationContext, OperationDeclaration};
use crate::Portable;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait EndpointSet: Send + Sized + 'static {
    fn empty() -> Option<Self> {
        None
    }

    #[cfg(feature = "streaming")]
    fn bind_wire(_limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        Err(super::ServingError::StreamUnsupported)
    }

    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>);
    fn transferred(&self) -> bool {
        Self::item_types() != (None, None)
    }
}

pub trait InputEndpoint: Send + Sized + 'static {
    #[cfg(feature = "streaming")]
    fn bind_wire(_limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        Err(super::ServingError::StreamUnsupported)
    }

    type Item: Send + 'static;
    fn transferred(&self) -> bool {
        true
    }
}
pub trait OutputEndpoint: Send + Sized + 'static {
    #[cfg(feature = "streaming")]
    fn bind_wire(_limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        Err(super::ServingError::StreamUnsupported)
    }

    type Item: Send + 'static;
    fn transferred(&self) -> bool {
        true
    }
}

// The Instance Contract Behind Every Registered Operation: Carrier Proxies,
// Lifecycle Built-Ins And Authored Operations Through `Authored`.
pub trait Handler: Send + Sized + 'static {
    type Request: Portable;
    type Response: Portable;
    type Error: std::error::Error + Send + Sync + 'static;
    type StreamEndpoints: EndpointSet;

    fn initialize(context: OperationContext) -> impl Future<Output = Result<Self, Self::Error>> + Send;

    fn run(
        &mut self, context: OperationContext, request: &mut Self::Request, endpoints: &mut Self::StreamEndpoints,
    ) -> impl Future<Output = Result<super::Response<Self::Response>, Self::Error>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn transferred(&self) -> bool {
        false
    }

    fn classify(_error: &Self::Error) -> FailureKind {
        FailureKind::Permanent
    }
}

// Selects The Handler That Executes A Declaration.
pub trait Executable: OperationDeclaration {
    type Handler: Handler<Request = <Self as OperationDeclaration>::Request, Response = <Self as OperationDeclaration>::Response>;

    // Resources the handler reads; registration refuses a snapshot lacking one.
    const RESOURCES: &'static [super::Resource] = &[];
}

/* === Implementations === */

impl EndpointSet for () {
    fn empty() -> Option<Self> {
        Some(())
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, None)
    }
}

impl<I: InputEndpoint, O: OutputEndpoint> EndpointSet for (I, O) {
    #[cfg(feature = "streaming")]
    fn bind_wire(limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        let (input, incoming) = I::bind_wire(limits)?;
        let (output, outgoing) = O::bind_wire(limits)?;
        Ok(((input, output), incoming.combine(outgoing)))
    }

    fn transferred(&self) -> bool {
        self.0.transferred() || self.1.transferred()
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (Some(std::any::TypeId::of::<I::Item>()), Some(std::any::TypeId::of::<O::Item>()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
