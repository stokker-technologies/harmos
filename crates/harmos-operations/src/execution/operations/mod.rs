/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod access;
mod authored;
mod calls;
mod cancellation;
pub mod instance;
pub mod instances;
mod operation;
pub mod publication;
pub(crate) mod registration;
pub(crate) mod resources;
pub(crate) mod response;
pub(crate) mod serving;
mod startup;

#[cfg(feature = "streaming")]
pub mod streaming;
mod surface;

/* === Re-Exports === */

#[doc(hidden)]
pub use authored::Carrier;
pub use authored::{EndpointSet, InputEndpoint, OutputEndpoint};
pub use calls::{CallFuture, Caller, Calls, ChildInstance, ControlFuture, InvocationError, OperationDependency, StartFuture, Target};
pub use cancellation::Cancellation;
pub use harmos_domain::operations::*;
pub use instance::Instance;
pub use operation::{Resource, Shape, Streams};
#[cfg(all(feature = "grpc", feature = "streaming"))]
pub use registration::generated::{
    ProtobufDependency, ProtobufEndpoints, ProtobufExchangeEndpoints, ProtobufHost, ProtobufInput, ProtobufItem, ProtobufOperation,
    ProtobufOutput,
};
pub use registration::{
    Catalog, CatalogDescription, FunctionContext, FunctionHandle, FunctionInvocation, NativeInstance, OperationFactory, ServiceFactory,
    ServiceInstance,
};
pub use registration::{FunctionContext as Context, OperationFactory as Operation};
pub(crate) use resources::ResourceSet;
pub use resources::{AssignedOperation, ExecutionContext, OperationAssignments, ResourceError};
pub use response::Response;
pub use serving::{ServingError, WireStreamLimits};

#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreams};

pub use startup::{StartupError, StartupIssue, StartupReport};
#[cfg(feature = "streaming")]
pub use streaming::{Error as StreamError, StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};
pub use surface::{Invocation, OperationHost, RequestClaims};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
