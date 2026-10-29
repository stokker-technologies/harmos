/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub mod access;
mod authored;
mod calls;
pub mod instance;
pub mod instances;
pub mod lifecycle;
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

#[doc(hidden)]
pub use authored::Carrier;
pub use authored::{EndpointSet, Executable, InputEndpoint, OutputEndpoint};
pub use calls::{CallFuture, Caller, Calls, InvocationError, Target};
pub use harmos_domain::operations::*;
pub use instance::Instance;
pub use operation::{Authored, Context, Dependent, Operation, Resource, Shape, Streams};
pub(crate) use registration::RegistrationBatch;
pub use registration::{Catalog, NativeInstance, OperationCatalog, OperationFactory, ServiceFactory, ServiceInstance};
pub(crate) use resources::ResourceSet;
pub use resources::{AssignedOperation, ExecutionContext, OperationAssignments, ResourceError};
pub use response::Response;
pub(crate) use serving::WireTable;
pub use serving::{
    CHILD_METHOD, ServingError, WIRE_METHOD, WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCodecError,
    WireCommand, decode_prepared_grant, encode_prepared_grant,
};

#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreamLimits, WireStreams};

pub use startup::StartupError;
#[cfg(feature = "streaming")]
pub use streaming::{Error as StreamError, StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};
pub use surface::{Host, RequestClaims};
