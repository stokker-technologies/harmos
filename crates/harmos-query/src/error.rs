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

/* === Imports === */

// Standard Library Imports
use std::error::Error as StdError;
use std::result::Result as StdResult;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = StdResult<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{domain} is not ready")]
    NotReady { domain: &'static str },

    #[error("{domain} admission is closed")]
    Closing { domain: &'static str },

    #[error("{domain} `{name}` is already registered")]
    AlreadyRegistered { domain: &'static str, name: String },

    #[error("backend {operation} failed ({kind:?}): {source}")]
    Backend {
        operation: &'static str,
        kind: BackendFailure,

        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },

    #[error("invalid {field}: {reason}")]
    InvalidInput { field: &'static str, reason: &'static str },

    #[error("{resource} capacity is exhausted")]
    ResourceExhausted { resource: &'static str },

    #[error("retained data begins at {available}, but {requested} was requested")]
    Gap { requested: u64, available: u64 },

    #[error(transparent)]
    Domain(#[from] harmos_domain::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendFailure {
    Transient,
    Permanent,
    Corrupt,
    Conflict,
    Unsupported,
}

/* === Traits === */

/* === Implementations === */

impl Error {
    pub fn backend_failure(&self) -> Option<BackendFailure> {
        match self {
            Self::Backend { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    pub fn backend(operation: &'static str, kind: BackendFailure, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Backend {
            operation,
            kind,
            source: Box::new(source),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
