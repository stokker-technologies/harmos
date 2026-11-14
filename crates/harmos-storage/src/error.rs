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
use harmos_query::{BackendFailure as QueryFailure, Error as QueryError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = StdResult<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("storage index `{name}` is not registered")]
    IndexNotFound { name: crate::Identifier },

    #[error("operation `{operation}` failed")]
    OperationFailed {
        operation: String,

        #[source]
        source: std::sync::Arc<dyn std::error::Error + Send + Sync>,
    },

    #[error("{domain} is not ready")]
    NotReady { domain: &'static str },

    #[error("persistence failed: {source}")]
    Persistence {
        #[source]
        source: std::sync::Arc<Error>,
    },

    #[error("{domain} admission is closed")]
    Closing { domain: &'static str },

    #[error("{domain} `{name}` is already registered")]
    AlreadyRegistered { domain: &'static str, name: String },

    #[error("backend {operation} failed ({kind:?}): {source}")]
    Backend {
        operation: &'static str,
        kind: BackendFailure,

        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
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
            Self::Persistence { source } => source.backend_failure(),
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

impl From<QueryError> for Error {
    fn from(error: QueryError) -> Self {
        match error {
            QueryError::InvalidInput { field, reason } => Self::InvalidInput { field, reason },
            QueryError::ResourceExhausted { resource } => Self::ResourceExhausted { resource },
            QueryError::Closing { domain } => Self::Closing { domain },
            QueryError::NotReady { domain } => Self::NotReady { domain },
            QueryError::Domain(error) => Self::Domain(error),
            QueryError::Backend { operation, kind, source } => {
                let source = match source.downcast::<Self>() {
                    Ok(error) => return *error,
                    Err(source) => source,
                };
                Self::Backend {
                    operation,
                    source,
                    kind: match kind {
                        QueryFailure::Transient => BackendFailure::Transient,
                        QueryFailure::Permanent => BackendFailure::Permanent,
                        QueryFailure::Corrupt => BackendFailure::Corrupt,
                        QueryFailure::Conflict => BackendFailure::Conflict,
                        QueryFailure::Unsupported => BackendFailure::Unsupported,
                    },
                }
            }
            other => Self::backend("execute query", BackendFailure::Permanent, other),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
