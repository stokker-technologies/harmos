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
pub use harmos_contract::operations::{
    CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy,
};
pub use harmos_contract::{catalog, operations};

#[cfg(all(not(target_arch = "wasm32"), feature = "componentize"))]
pub use component::componentize;
pub use harmos_contract::inventory;
pub use harmos_contract::{
    ConfigError, Plugin, Registrar, Registration, RegistrationError, StreamSpec, encode_config,
    reserved, validate_config,
};

#[cfg(feature = "kit")]
pub use kit::{Context, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT};

pub use harmos_contract::__private;

pub use harmos_contract::schema;
pub use harmos_contract::{MemorySize, Message, MessageError, RowMetadata, Storable};
#[cfg(feature = "derive")]
pub use harmos_contract::{message, operation};

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
