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

#[cfg(feature = "kit")]
pub mod kit;

/* === Re-Exports === */

pub use harmos_operations as builtins;

pub use error::{Error, Result};
pub use harmos_domain::{catalog, listeners};
pub use harmos_sdk::operations;
pub use harmos_sdk::operations::Host;
pub use harmos_sdk::operations::{Backoff, CacheKey, Jitter, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy};
pub use harmos_sdk::{Dependencies, Operation, OperationContext, Plugin, RegistrationBatch, Service};

pub use harmos_domain::inventory;
pub use harmos_domain::{
    ConfigError, Failure, Refusal, Registrar, Registration, RegistrationError, StreamSpec, encode_config, reserved, validate_config,
};

#[cfg(feature = "kit")]
pub use kit::{Context, OperationInstance, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT};

pub use harmos_sdk::__private;

pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{Axial, Batched, Metadata, Prefix, StreamField, Track, Tracked};
pub use harmos_domain::{
    Digest, Fingerprint, MemorySize, Message, MessageError, OperationDefinition, Portable, QualifiedId, RequestFor, RowMetadata,
    ServicePolicy, Storable,
};
#[cfg(feature = "derive")]
pub use harmos_domain::{listener, message, operation, stream};
pub use harmos_sdk::plugin;

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
