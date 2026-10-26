/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod authored;
mod dependencies;
pub mod instance;
pub mod lifecycle;
pub mod registration;
pub(crate) mod response;
mod serving;

#[cfg(feature = "streaming")]
pub mod streaming;
mod surface;

/* === Re-Exports === */

pub use authored::{EndpointSet, InputEndpoint, Operation, OutputEndpoint};
pub use dependencies::{Dependencies, DependencyError, OperationContext};
pub use harmos_domain::operations::*;
pub use instance::Instance;
pub use registration::{NativeInstance, OperationFactory, RegistrationBatch, ServiceFactory, ServiceInstance};
pub use response::Response;
pub use serving::{
    CHILD_METHOD, ServingError, WIRE_METHOD, WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCodecError,
    WireCommand, WireTable, decode_prepared_grant, encode_prepared_grant,
};

#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreamLimits, WireStreams};

#[cfg(feature = "streaming")]
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};
pub use surface::Host;

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
