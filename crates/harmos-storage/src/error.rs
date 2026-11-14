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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = std::result::Result<T, Error>;

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

    pub fn backend(operation: &'static str, kind: BackendFailure, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Backend {
            operation,
            kind,
            source: Box::new(source),
        }
    }
}


impl From<harmos_query::Error> for Error {
    fn from(error: harmos_query::Error) -> Self {
        match error {
            harmos_query::Error::InvalidInput { field, reason } => Self::InvalidInput { field, reason },
            harmos_query::Error::ResourceExhausted { resource } => Self::ResourceExhausted { resource },
            harmos_query::Error::Closing { domain } => Self::Closing { domain },
            harmos_query::Error::NotReady { domain } => Self::NotReady { domain },
            harmos_query::Error::Domain(error) => Self::Domain(error),
            harmos_query::Error::Backend { operation, kind, source } => {
                let source = match source.downcast::<Self>() { Ok(error) => return *error, Err(source) => source };
                Self::Backend {
                operation, source, kind: match kind {
                    harmos_query::BackendFailure::Transient => BackendFailure::Transient,
                    harmos_query::BackendFailure::Permanent => BackendFailure::Permanent,
                    harmos_query::BackendFailure::Corrupt => BackendFailure::Corrupt,
                    harmos_query::BackendFailure::Conflict => BackendFailure::Conflict,
                    harmos_query::BackendFailure::Unsupported => BackendFailure::Unsupported,
                },
            }
            },
            other => Self::backend("execute query", BackendFailure::Permanent, other),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
