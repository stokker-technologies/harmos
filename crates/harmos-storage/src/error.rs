/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Type Aliases === */
pub type Result<T> = std::result::Result<T, Error>;

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
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

/* === Implementations === */
impl Error {
    pub fn backend_failure(&self) -> Option<BackendFailure> {
        match self {
            Self::Backend { kind, .. } => Some(*kind),
            Self::Persistence { source } => source.backend_failure(),
            _ => None,
        }
    }

    pub fn backend(
        operation: &'static str,
        kind: BackendFailure,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Backend {
            operation,
            kind,
            source: Box::new(source),
        }
    }
}
