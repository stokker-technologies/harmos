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
#[cfg(feature = "transport")]
pub use attach::{Attach, Attached, Executable, ExecutableError};
pub use error::{Error, Result};
pub use harmos_domain::inventory;
pub use harmos_domain::operations::Host;
pub use harmos_domain::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
};
pub use harmos_domain::{
    ConfigError, Failure, Plugin, RESERVED, Refusal, Registrar, Registration, RegistrationError,
    StreamSpec, UNKNOWN, UNREADABLE, encode_config, reserved, validate_config,
};
pub use harmos_domain::{catalog, failure, listeners, operations};

pub use harmos_domain::StreamDeclaration;
#[cfg(feature = "kit")]
pub use kit::{
    Activation, ActivationId, Activity, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError,
    DemandKey, Instance, Observer, ObserverAttachment, ObserverCaller, ObserverContext,
    ObserverError, ObserverHost, ObserverPublisher, ObserverStreamPublisher, OperationInstance,
    PluginNative, PublicationError, PublishError, Publisher, Supervisor, Tick, run_plugin,
    serve_plugin_channels,
};

#[cfg(feature = "transport")]
pub use wire::{Frame, PluginCatalog, Publication};

#[cfg(feature = "derive")]
pub use harmos_domain::observer;
pub use harmos_domain::plugin;
pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{
    Axial, Batched, Definition, Metadata, Prefix, StreamField, Track, Tracked,
};
pub use harmos_domain::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_domain::{listener, message, operation, stream};

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
