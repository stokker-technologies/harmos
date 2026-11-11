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
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
mod catalog;
mod clock;
pub(crate) mod context;
mod surface;

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub(crate) mod children;

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
mod component;
#[cfg(feature = "plugin-guest")]
mod encoded;
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

mod registration;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
mod remote;

#[cfg(feature = "grpc")]
mod protobuf;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
mod sdk;
mod wire;

/* === Re-Exports === */

#[cfg(feature = "plugin-guest")]
#[doc(hidden)]
pub use crate::guest::host::envelope::{
    WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCodecError, WireCommand,
};
pub use crate::supervision::instance::{Outcome as OperationOutcome, Phase as OperationPhase, Status as OperationStatus};
#[cfg(feature = "plugin-guest")]
pub use caller::Caller;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub use catalog::Binding;
pub use context::ContextMut;
#[cfg(feature = "grpc")]
pub use grpc::GrpcBinding;
#[cfg(feature = "grpc")]
pub use grpc::GrpcMethod;
pub use harmos_operations::execution::__private::{ErasedResponse, PreparedNative, ResourceSet, ServiceFuture};
pub use harmos_operations::execution::operations::Invocation;
pub use harmos_operations::execution::operations::{
    Backoff, CallValue, Callees, Catalog, Codec, ContextError, ContextValue, Contract, ContractError, Contracts, Declarations, Dependent,
    Direction, Entry, Executable, FailureKind, Grpc, InvocationError, Invokes, Jitter, NoStream, Operation, OperationCatalog,
    OperationDeclaration, OperationDefinition, OperationPolicy, PORTABLE_BYTES, PORTABLE_VALUES, PolicyError, ReplaySafety, Requires,
    Resource, RestartPolicy, RetryPolicy, ServingError, StartupIssue, StartupReport, Stream, Visibility, WireDeclaration, WireEndpoints,
    WireStreamLimits, WireStreams, item_types,
};
pub use harmos_operations::execution::operations::{CallContext, Cancellation, Instance, OperationHost, RequestClaims, Response, instance};
pub use harmos_operations::execution::operations::{ExecutionContext, NativeInstance};
#[cfg(feature = "grpc")]
pub use harmos_operations::execution::operations::{
    ProtobufDependency, ProtobufEndpoints, ProtobufInput, ProtobufItem, ProtobufOperation, ProtobufOutput,
};
pub use lane::{Capacity, Lane, LaneSet, LaneSettings};
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub use remote::{RemoteStreamError, StreamEndpoints, StreamPeer};
pub use scope::Scope;
pub use surface::{InvocationExt, RuntimeOperationHost};

pub use crate::config::OperationSettings as Settings;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub use component::{Component, ComponentBindings, ComponentError, ComponentOperation};
#[cfg(feature = "plugin-guest")]
pub use encoded::EncodedBinding;
pub use handler::{Context, HostContext, OperationGeneration, OperationInstanceId, Running};
pub use harmos_domain::observers;
pub use harmos_operations::execution::operations::StreamError;
pub use harmos_operations::execution::operations::{EndpointSet, InputEndpoint, OutputEndpoint};
pub(crate) use manager::Manager;
pub use manager::OperationManager;
#[cfg(feature = "plugin-guest")]
pub(crate) use manager::WeakOperationManager;
pub use protobuf::ProtobufDownload;
pub use session::{Completion, Download, Exchange, Session, SessionInput, SessionOutput, Upload};
pub use streaming::{StreamInput, StreamItem, StreamOutput, StreamPermit, StreamSettings, stream};

#[cfg(feature = "plugin-guest")]
pub(crate) use manager::Registry;

#[cfg(feature = "plugin-guest")]
pub use registration::Bindings;
pub(crate) use registration::CatalogBinding;
pub use registration::Registration;
pub(crate) use registration::RegistrationExt;

#[cfg(feature = "plugin-guest")]
pub use registration::PluginRegistration;

pub(crate) use manager::GrpcAdmission;
#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
pub use sdk::PluginSdkRegistration;
pub use wire::EncodedInvocation;

#[cfg(feature = "plugin-guest")]
pub use wire::PluginCallback;

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
