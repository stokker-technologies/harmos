/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod error;

#[cfg(feature = "transport")]
pub mod attach;

#[cfg(feature = "transport")]
pub mod transport;

#[cfg(feature = "kit")]
pub mod kit;

#[cfg(feature = "transport")]
mod wire;

/* === Re-Exports === */

pub use harmos_domain::model;
pub use harmos_operations as builtins;

#[cfg(feature = "transport")]
pub use attach::{Attach, Attached, Executable, ExecutableError};
pub use error::{Error, Result};
pub mod catalog {
    pub use harmos_domain::catalog::*;
}
pub use harmos_domain::inventory;
pub use harmos_domain::{
    ConfigError, Failure, RESERVED, Refusal, Registrar, Registration, RegistrationError, StreamSpec, UNKNOWN, UNREADABLE, encode_config,
    reserved, validate_config,
};
pub use harmos_domain::{failure, listeners};
pub use harmos_sdk::operations;
pub use harmos_sdk::operations::Host;
pub use harmos_sdk::operations::{Backoff, CacheKey, Jitter, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy};
pub use harmos_sdk::{Context, Dependent, ExecutionContext, Lifecycle, Operation, Plugin, Resource, Service, Setup};

pub use harmos_domain::StreamDeclaration;
#[cfg(feature = "kit")]
pub use kit::{
    Activation, ActivationId, Activity, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError, DemandKey, Instance, Observer,
    ObserverAttachment, ObserverCaller, ObserverContext, ObserverError, ObserverHost, ObserverPublisher, ObserverStreamPublisher,
    OperationInstance, PluginNative, PublicationError, PublishError, Publisher, Supervisor, Tick, run_plugin, run_plugin_with_settings,
    serve_plugin_channels, serve_plugin_channels_with_settings,
};

#[cfg(feature = "transport")]
pub use wire::{Frame, FramePayload, PluginCatalog, Publication};

#[cfg(feature = "derive")]
pub use harmos_domain::observer;
pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{Axial, Batched, Metadata, Prefix, StreamField, Track, Tracked};
pub use harmos_domain::{
    Digest, Fingerprint, MemorySize, Message, MessageError, OperationDefinition, Portable, QualifiedId, RowMetadata, ServicePolicy,
    Storable,
};
#[cfg(feature = "derive")]
pub use harmos_domain::{listener, message, stream};
#[cfg(feature = "derive")]
pub use harmos_macros::{sidecar_catalog as catalog, sidecar_operation as operation};
pub use harmos_sdk::plugin;

#[cfg(feature = "kit")]
pub use kit::__private;

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

#[cfg(feature = "kit")]
impl harmos_domain::__private::CarrierContext for Supervisor {
    const FACILITY: &'static str = "sidecar";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
