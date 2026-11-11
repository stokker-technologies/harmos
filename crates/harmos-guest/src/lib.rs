/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "kit")]
mod dispatch;
mod envelope;
mod error;
pub mod lifecycle;
#[cfg(feature = "kit")]
pub mod plugin;

#[cfg(feature = "kit")]
pub mod kit;

/* === Re-Exports === */

pub use error::{Error, Result};
pub use harmos_operations::inventory;
pub use harmos_operations::listeners;
pub use harmos_operations::model;
pub use harmos_operations::{ConfigError, encode_config, validate_config};
pub use harmos_operations::{Failure, Refusal, Registrar, Registration, RegistrationError, StreamSpec, reserved};
pub mod builtins;

pub mod operations {
    /* === Modules === */

    /* === Re-Exports === */

    #[doc(hidden)]
    pub use crate::envelope::{WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCodecError, WireCommand};
    pub use harmos_operations::execution::operations::*;
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
}
pub use harmos_operations::execution::operations::{Backoff, Jitter, OperationPolicy, RequestShape, RestartPolicy, RetryPolicy};
pub use harmos_operations::execution::operations::{Catalog, CatalogDescription, OperationHost};
pub use harmos_operations::execution::{Context, Lifecycle, Operation, Resource, Response, Service, Setup};

pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{Digest, Fingerprint, MemorySize, Message, MessageError, Portable, QualifiedId, RowMetadata, Storable};
pub use harmos_domain::{Metadata, StreamField};
pub use harmos_operations::execution::__private;
pub use harmos_operations::{OperationDefinition, ServicePolicy};
#[cfg(feature = "kit")]
pub use kit::{OperationInstance, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT};

#[cfg(feature = "derive")]
pub use harmos_domain::Row;
pub use harmos_domain::StreamEntry;
#[cfg(feature = "derive")]
pub use harmos_operations::Request;

#[cfg(feature = "derive")]
pub use harmos_guest_macros::plugin as guest;
#[cfg(feature = "kit")]
pub use plugin::Plugin;

/* === Imports === */

extern crate self as harmos_guest;

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
impl harmos_operations::__private::CarrierContext for kit::Context {
    const FACILITY: &'static str = "guest";
}

/* === Public Functions === */

/* === Private Functions === */

pub mod catalog {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_operations::catalog::*;

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
}

/* === Tests === */
