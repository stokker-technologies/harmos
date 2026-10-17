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

#[cfg(all(not(target_arch = "wasm32"), feature = "componentize"))]
pub mod component;

#[cfg(feature = "kit")]
pub mod kit;

/* === Re-Exports === */
pub use error::{Error, Result};
pub use harmos_sdk::Plugin;
pub use harmos_sdk::operations::Host;
pub use harmos_sdk::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
};
pub use harmos_domain::{catalog, listeners};
pub use harmos_sdk::operations;

#[cfg(all(not(target_arch = "wasm32"), feature = "componentize"))]
pub use component::componentize;
pub use harmos_domain::inventory;
pub use harmos_domain::{
    ConfigError, Failure, Refusal, Registrar, Registration, RegistrationError, StreamSpec,
    encode_config, reserved, validate_config,
};

#[cfg(feature = "kit")]
pub use kit::{
    Context, OperationInstance, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT,
};

pub use harmos_sdk::__private;

pub use harmos_sdk::plugin;
pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{
    Axial, Batched, Definition, Metadata, Prefix, StreamField, Track, Tracked,
};
pub use harmos_domain::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_domain::{listener, message, operation, stream};

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
impl harmos_domain::__private::CarrierContext for Context {
    const FACILITY: &'static str = "guest";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
