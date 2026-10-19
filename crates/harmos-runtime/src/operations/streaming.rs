/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

pub(super) use harmos_sdk::operations::streaming::RequestClaims;
pub use harmos_sdk::operations::streaming::{StreamInput, StreamItem, StreamSettings};

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::{Error, Result};
use harmos_sdk::operations::streaming as portable;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type StreamOutput<T> = portable::StreamOutput<T, Error>;
pub type StreamPermit<'a, T> = portable::StreamPermit<'a, T, Error>;
/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<portable::Error> for Error {
    fn from(error: portable::Error) -> Self {
        match error {
            portable::Error::Settings => {
                Self::InvalidInput { field: "operation stream limits", reason: "capacity exceeds the semaphore claim limit" }
            }
            portable::Error::Capacity => Self::ResourceExhausted { resource: "operation stream bytes" },
            portable::Error::Construction => {
                Self::InvalidInput { field: "operation stream construction bound", reason: "constructed value exceeds its reserved bytes" }
            }
            portable::Error::Closed => Self::NotReady { domain: "operation stream input" },
            portable::Error::Ownership => {
                Self::InvalidInput { field: "operation stream payload", reason: "original owned payload cannot be moved into a request" }
            }
            portable::Error::Codec(source) => Self::TaskFailed { domain: "operation stream decode", source: Arc::new(source) },
        }
    }
}
/* === Public Functions === */

pub fn stream<T>(settings: StreamSettings) -> Result<(StreamOutput<T>, StreamInput<T>)> {
    portable::channel(settings)
}
/* === Private Functions === */

/* === Tests === */
