/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "grpc")]
pub(crate) mod protobuf;
#[cfg(feature = "streaming")]
mod streams;

/* === Re-Exports === */

#[cfg(feature = "streaming")]
pub use streams::{WireEndpoints, WireStreams};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{ContractError, FailureKind, StartupError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct WireStreamLimits {
    pub input: harmos_operations::config::StreamSettings,
    pub output: harmos_operations::config::StreamSettings,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum ServingError {
    #[error(transparent)]
    Startup(#[from] Box<StartupError>),

    #[error("the carrier resource store is frozen")]
    StoreFrozen,

    #[error("caller may not invoke operation `{operation}` without a host assignment")]
    Unassigned { operation: String },

    #[error("operation endpoint types differ from the declaration")]
    EndpointTypes,

    #[error("carrier invocation does not support stream commands")]
    StreamUnsupported,

    #[cfg(feature = "streaming")]
    #[error("streaming catalog requires explicit provider stream limits")]
    StreamLimits,

    #[cfg(feature = "streaming")]
    #[error("carrier stream failed: {0}")]
    Stream(#[source] super::streaming::Error),

    #[cfg(feature = "streaming")]
    #[error("carrier stream direction already has an outstanding request")]
    StreamBusy,

    #[cfg(feature = "streaming")]
    #[error("carrier stream direction or attempt is not available")]
    StreamPhase,

    #[error(transparent)]
    Lifecycle(#[from] harmos_operations::model::operations::lifecycle::Error),

    #[error(transparent)]
    Contract(#[from] ContractError),

    #[error(transparent)]
    Declaration(#[from] crate::operations::Error),

    #[error(transparent)]
    Foundation(#[from] harmos_domain::Error),

    #[error("operation `{operation}` declares resource `{resource}` that its registration does not provide")]
    Resource { operation: String, resource: &'static str },

    #[error("operation `{operation}` has no bound carrier handler")]
    Unknown { operation: String },

    #[error("operation `{operation}` requires streaming endpoints unsupported by this carrier binding")]
    Streaming { operation: String },

    #[error("operation `{operation}` has a mismatched fingerprint or stale generation")]
    Identity { operation: String },

    #[error("carrier invocation `{invocation}` is already prepared")]
    Duplicate { invocation: u64 },

    #[error("carrier invocation `{invocation}` has not been prepared")]
    Missing { invocation: u64 },

    #[error("carrier operation has not completed initialization")]
    Uninitialized,

    #[error("carrier operation initialization already started and cannot be replayed")]
    InitializationStarted,

    #[error("carrier invocation capacity is exhausted")]
    Capacity,

    #[error("carrier ordinary invocations have not finished settlement")]
    Unsettled,

    #[error("carrier operation admission is closing")]
    Closing,

    #[error("carrier handler definitions differ from the selected operation catalog")]
    Catalog,

    #[error("operation payload codec failed")]
    Codec(#[source] crate::execution::MessageError),

    #[error("carrier handler panicked")]
    Panicked,

    // The handler's failure, written once at its origin.
    #[error(transparent)]
    Handler(crate::execution::Failure),

    #[error("native handler failed: {0}")]
    Native(#[source] std::sync::Arc<dyn std::error::Error + Send + Sync>),
}

/* === Traits === */

/* === Implementations === */

impl ServingError {
    pub fn classification(&self) -> FailureKind {
        match self {
            Self::Handler(failure) => failure.code(),
            Self::Panicked => FailureKind::Ambiguous,
            Self::Codec(_) | Self::Declaration(_) | Self::Lifecycle(_) | Self::Contract(_) | Self::Catalog | Self::Streaming { .. } => {
                FailureKind::Invalid
            }
            _ => FailureKind::Permanent,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
