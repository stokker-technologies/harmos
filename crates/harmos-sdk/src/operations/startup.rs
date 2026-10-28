/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Startup Configuration Errors

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::ServingError;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("resource `{name}` is already configured")]
    DuplicateResource { name: &'static str },

    #[error("catalog `{name}` is already configured")]
    DuplicateCatalog { name: &'static str },

    #[error("operation `{operation}` requires missing resource `{resource}`")]
    MissingResource { operation: String, resource: &'static str },

    #[error("lane `{lane}` is not declared by the application")]
    UnknownLane { lane: &'static str },

    #[error("startup configuration has {} errors: {errors:?}", .errors.len())]
    Multiple { errors: Vec<StartupError> },

    #[error("operation binding failed: {source}")]
    Binding {
        #[source]
        source: Box<ServingError>,
    },

    #[error("runtime startup failed: {source}")]
    Runtime {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/* === Traits === */

/* === Implementations === */

impl StartupError {
    pub fn aggregate(mut errors: Vec<Self>) -> Option<Self> {
        match errors.len() {
            0 => None,
            1 => errors.pop(),
            _ => Some(Self::Multiple { errors }),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
