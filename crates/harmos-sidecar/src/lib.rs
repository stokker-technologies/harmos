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
pub use harmos_contract::inventory;
pub use harmos_contract::observers::{Exit as ObserverExit, State as ObserverStatus};
pub use harmos_contract::operations::Host;
pub use harmos_contract::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy,
};
pub use harmos_contract::{
    ConfigError, Plugin, RESERVED, Refusal, Registrar, Registration, RegistrationError, StreamSpec,
    UNKNOWN, UNREADABLE, encode_config, reserved, validate_config,
};
pub use harmos_contract::{builtin, catalog, listeners, operations};

pub use harmos_contract::StreamDeclaration;
#[cfg(feature = "kit")]
pub use kit::{
    Activation, ActivationId, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError, DemandKey,
    Instance, Observer, ObserverAttachment, ObserverCaller, ObserverContext, ObserverHost,
    ObserverPublisher, ObserverStreamPublisher, ObserverTask, OperationInstance, PluginNative,
    PublicationError, PublishError, Publisher, Supervisor, Tick, run_plugin, serve_plugin_channels,
};

#[cfg(feature = "transport")]
pub use wire::{Frame, ObserverControl, ObserverUpdate, PluginCatalog, Publication};

#[cfg(feature = "derive")]
pub use harmos_contract::observer;
pub use harmos_contract::plugin;
pub use harmos_contract::schema;
pub use harmos_contract::streams;
pub use harmos_contract::{
    Axial, Batched, Definition, Metadata, Prefix, StreamField, Track, Tracked,
};
pub use harmos_contract::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_contract::{listener, message, operation, stream};

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
impl harmos_contract::__private::CarrierContext for Supervisor {
    const FACILITY: &'static str = "sidecar";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
