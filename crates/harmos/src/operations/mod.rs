/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod admission;
mod authored;
mod binding;
mod caller;
mod catalog;
pub(crate) mod context;

#[cfg(any(
    feature = "sidecar-host",
    all(feature = "guest-host", not(target_arch = "wasm32"))
))]
pub(crate) mod children;

#[cfg(all(feature = "guest-host", not(target_arch = "wasm32")))]
mod component;
mod execution;
mod finalization;
#[cfg(feature = "grpc")]
mod grpc;
mod handler;
pub(crate) mod instances;
pub(crate) mod lane;
mod manager;
pub mod middleware;
pub(crate) mod refusal;
pub(crate) mod scope;
mod session;
mod settings;
mod streaming;

#[cfg(feature = "sidecar-host")]
mod native;
mod registration;
#[cfg(any(
    feature = "sidecar-host",
    all(feature = "guest-host", not(target_arch = "wasm32"))
))]
mod remote;

#[cfg(any(
    feature = "sidecar-host",
    all(feature = "guest-host", not(target_arch = "wasm32"))
))]
mod sdk;
mod wire;

/* === Re-Exports === */
pub use crate::supervision::instance::{
    Outcome as OperationOutcome, Phase as OperationPhase, Status as OperationStatus,
};
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use caller::Caller;
pub use catalog::{Binding, Implementations, WireImplementations};
pub use context::ContextMut;
#[cfg(feature = "grpc")]
pub use grpc::GrpcBinding;
#[cfg(feature = "grpc")]
pub use grpc::GrpcMethod;
pub use harmos_contract::operations::CallContext;
#[cfg(any(
    feature = "sidecar-host",
    all(feature = "guest-host", not(target_arch = "wasm32"))
))]
pub use remote::{RemoteStreamError, StreamEndpoints, StreamPeer};

#[cfg(all(feature = "guest-host", not(target_arch = "wasm32")))]
pub use component::{Component, ComponentBindings, ComponentError, ComponentOperation};
pub use handler::{Context, OperationGeneration, OperationInstance, OperationInstanceId};
pub use harmos_contract::observers;
pub use harmos_contract::operations::{
    EndpointSet, InputEndpoint, OutputEndpoint, WireVisitor, host,
};
pub use manager::OperationManager;
pub use session::{Completion, Download, Exchange, Session, SessionInput, SessionOutput, Upload};
pub use settings::Settings;
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub(crate) use manager::Registry;

#[cfg(feature = "sidecar-host")]
pub use native::{Native, NativeBindings, NativeError, NativeOperation};
pub use registration::Bindings;
pub(crate) use registration::CatalogBinding;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use registration::PluginRegistration;

#[cfg(any(
    feature = "sidecar-host",
    all(feature = "guest-host", not(target_arch = "wasm32"))
))]
pub use sdk::PluginSdkRegistration;
pub use wire::EncodedInvocation;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use wire::PluginCallback;

#[cfg(feature = "sidecar-host")]
pub(crate) use wire::PluginControl;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_contract::operations::Operation;

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
