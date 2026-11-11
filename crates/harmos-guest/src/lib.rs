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
pub use harmos_domain::inventory;
pub use harmos_domain::listeners;
pub use harmos_domain::model;
pub use harmos_domain::{
    ConfigError, Failure, Refusal, Registrar, Registration, RegistrationError, StreamSpec, encode_config, reserved, validate_config,
};
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
pub use harmos_operations::execution::operations::{Catalog, OperationHost};
pub use harmos_operations::execution::{Context, Lifecycle, Operation, Resource, Response, Service, Setup};

pub use harmos_domain::schema;
pub use harmos_domain::streams;
pub use harmos_domain::{
    Digest, Fingerprint, MemorySize, Message, MessageError, OperationDefinition, Portable, QualifiedId, RowMetadata, ServicePolicy,
    Storable,
};
pub use harmos_domain::{Metadata, StreamField};
pub use harmos_operations::execution::__private;
#[cfg(feature = "kit")]
pub use kit::{OperationInstance, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT};

#[cfg(feature = "derive")]
pub use harmos_domain::{Request, Row, StreamEntry, message};

#[cfg(feature = "derive")]
pub use harmos_guest_macros::plugin as guest;
#[cfg(feature = "derive")]
pub use harmos_macros::guest_catalog as catalog;
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
impl harmos_domain::__private::CarrierContext for kit::Context {
    const FACILITY: &'static str = "guest";
}

/* === Public Functions === */

/* === Private Functions === */

pub mod catalog {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::catalog::*;

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
