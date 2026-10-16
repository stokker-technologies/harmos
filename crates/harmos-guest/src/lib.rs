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
pub use harmos_contract::operations::Host;
pub use harmos_contract::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
};
pub use harmos_contract::{builtin, catalog, listeners, operations};

#[cfg(all(not(target_arch = "wasm32"), feature = "componentize"))]
pub use component::componentize;
pub use harmos_contract::inventory;
pub use harmos_contract::{
    ConfigError, Failure, Plugin, Refusal, Registrar, Registration, RegistrationError, StreamSpec,
    encode_config, reserved, validate_config,
};

#[cfg(feature = "kit")]
pub use kit::{
    Context, OperationInstance, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT,
};

pub use harmos_contract::__private;

pub use harmos_contract::plugin;
pub use harmos_contract::schema;
pub use harmos_contract::streams;
pub use harmos_contract::{
    Axial, Batched, Definition, Metadata, Prefix, StreamField, Track, Tracked,
};
pub use harmos_contract::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_contract::{listener, message, operation, stream};

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
impl harmos_contract::__private::CarrierContext for Context {
    const FACILITY: &'static str = "guest";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
