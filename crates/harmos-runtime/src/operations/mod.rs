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
pub mod authorization;
mod binding;
mod caller;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
mod catalog;
pub(crate) mod context;
mod surface;

#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
pub(crate) mod children;

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
mod component;
mod execution;
mod finalization;
#[cfg(feature = "grpc")]
mod grpc;
mod handler;
pub(crate) mod instances;
mod invocation;
pub(crate) mod lane;
mod manager;
pub mod middleware;
pub(crate) mod refusal;
pub mod resident;
pub(crate) mod scope;
mod session;
mod streaming;

#[cfg(feature = "plugin-sidecar")]
mod native;
mod registration;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
mod remote;

#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
mod sdk;
mod wire;

/* === Re-Exports === */

pub use crate::supervision::instance::{Outcome as OperationOutcome, Phase as OperationPhase, Status as OperationStatus};
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub use caller::Caller;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
pub use catalog::Binding;
pub use context::ContextMut;
#[cfg(feature = "grpc")]
pub use grpc::GrpcBinding;
#[cfg(feature = "grpc")]
pub use grpc::GrpcMethod;
pub use harmos_sdk::operations::{
    Backoff, CHILD_METHOD, CacheKey, CacheKeyError, CallValue, Callees, Catalog, Codec, ContextError, ContextValue, Contract,
    ContractError, Contracts, Declarations, Dependent, Direction, Entry, Executable, FailureKind, Grpc, InvocationError, Invokes, Jitter,
    MAX_CACHE_KEY_BYTES, NoStream, Operation, OperationCatalog, OperationDeclaration, OperationDefinition, OperationPolicy, PORTABLE_BYTES,
    PORTABLE_VALUES, PolicyError, ReplaySafety, Requires, Resource, RestartPolicy, RetryPolicy, ServingError, Stream, Visibility,
    WIRE_METHOD, WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCodecError, WireCommand, WireDeclaration,
    WireEndpoints, WireStreamLimits, WireStreams, item_types,
};
pub use harmos_sdk::operations::{CallContext, Host, Instance, RequestClaims, Response, instance};
pub use lane::{Capacity, Lane, LaneSet, LaneSettings};
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
pub use remote::{RemoteStreamError, StreamEndpoints, StreamPeer};
pub use scope::Scope;
pub use surface::Invocation;

pub use crate::config::OperationSettings as Settings;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub use component::{Component, ComponentBindings, ComponentError, ComponentOperation};
pub use handler::{Context, HostContext, OperationGeneration, OperationInstanceId, Running};
pub use harmos_domain::observers;
pub use harmos_sdk::operations::StreamError;
pub use harmos_sdk::operations::{EndpointSet, InputEndpoint, OutputEndpoint};
pub(crate) use manager::Manager;
pub use manager::OperationManager;
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub(crate) use manager::WeakOperationManager;
pub use session::{Completion, Download, Exchange, Session, SessionInput, SessionOutput, Upload};
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub(crate) use manager::Registry;

#[cfg(feature = "plugin-sidecar")]
pub use native::{Native, NativeBindings, NativeError, NativeOperation};
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub use registration::Bindings;
pub(crate) use registration::CatalogBinding;
pub(crate) use registration::{Registration, RegistrationExt};

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub use registration::PluginRegistration;

#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
pub use sdk::PluginSdkRegistration;
pub use wire::EncodedInvocation;

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub use wire::PluginCallback;

#[cfg(feature = "plugin-sidecar")]
pub(crate) use wire::PluginControl;

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
