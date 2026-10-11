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
#[cfg(all(feature = "packaging", not(target_arch = "wasm32")))]
pub mod packaging;

#[cfg(feature = "transport")]
mod wire;

/* === Re-Exports === */
#[cfg(feature = "transport")]
pub use attach::{Attach, Attached, Executable, ExecutableError};
pub use error::{Error, Result};
pub use harmos_contract::inventory;
pub use harmos_contract::observers::{Exit as ObserverExit, State as ObserverStatus};
pub use harmos_contract::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy,
};
pub use harmos_contract::{
    ConfigError, Plugin, RESERVED, Refusal, Registrar, Registration, RegistrationError, StreamSpec,
    UNKNOWN, UNREADABLE, encode_config, reserved, validate_config,
};
pub use harmos_contract::{catalog, operations};

pub use harmos_contract::StreamDeclaration;
#[cfg(feature = "kit")]
pub use kit::{
    Activation, ActivationId, CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT, CallError, DemandKey,
    Instance, Observer, ObserverAttachment, ObserverCaller, ObserverContext, ObserverHost,
    ObserverPublisher, ObserverStreamPublisher, ObserverTask, PluginNative, PublicationError,
    PublishError, Publisher, Supervisor, Tick, run_plugin, serve_plugin_channels,
};

#[cfg(feature = "transport")]
pub use wire::{Frame, ObserverControl, ObserverUpdate, PluginCatalog, Publication};

pub use harmos_contract::schema;
pub use harmos_contract::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_contract::{message, operation};

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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
