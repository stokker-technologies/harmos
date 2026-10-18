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
mod catalog;
pub mod instance;
pub mod lifecycle;
pub(crate) mod response;
mod serving;
#[cfg(feature = "streaming")]
pub mod streaming;
mod surface;

/* === Re-Exports === */
pub use authored::{EndpointSet, InputEndpoint, Operation, OutputEndpoint};
pub use catalog::{AuthoredContracts, AuthoredDeclarations};
pub use harmos_domain::operations::*;
pub use instance::Instance;
pub use response::Response;
pub use serving::{
    CHILD_METHOD, Serves, ServingError, WIRE_METHOD, WireAnswer, WireBinding, WireChild, WireChildAction, WireChildCall, WireChildControl,
    WireCodecError, WireCommand, WireTable, decode_prepared_grant, encode_prepared_grant,
};
#[cfg(feature = "streaming")]
pub use serving::{WireEndpoints, WireStreamLimits, WireStreams};
#[cfg(feature = "streaming")]
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};
pub use surface::Host;
