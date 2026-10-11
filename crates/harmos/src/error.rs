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
use std::convert::Infallible;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error<E = Infallible> {
    #[cfg(feature = "runtime")]
    #[error("runtime shutdown budget expired during {phase}; cleanup remains owned")]
    ShutdownTimeout { phase: &'static str },
    #[cfg(feature = "runtime")]
    #[error("authenticated caller identity is required")]
    Unauthenticated,
    #[cfg(feature = "runtime")]
    #[error("caller is not permitted to perform this action")]
    PermissionDenied,
    #[cfg(feature = "runtime")]
    #[error("record protobuf schema is invalid: {source}")]
    RecordSchema {
        #[source]
        source: crate::schema::Error,
    },
    #[cfg(feature = "runtime")]
    #[error("record protobuf payload is invalid: {source}")]
    RecordPayload {
        #[source]
        source: crate::schema::PayloadError,
    },
    #[cfg(feature = "runtime")]
    #[error("record declaration is unavailable: {source}")]
    RecordDeclaration {
        #[source]
        source: crate::DecodeError,
    },
    #[cfg(feature = "runtime")]
    #[error(transparent)]
    OperationRefused(#[from] crate::Refusal),
    #[cfg(feature = "runtime")]
    #[error("operation child lane `{child}` would cycle with held lane `{parent}`")]
    OperationLaneCycle {
        parent: &'static str,
        child: &'static str,
    },
    #[cfg(all(
        feature = "runtime",
        any(feature = "load", feature = "guest-host", feature = "sidecar-host")
    ))]
    #[error("operation caller is unavailable: {source}")]
    OperationCaller {
        #[source]
        source: crate::load::DispatchError,
    },
    #[cfg(feature = "runtime")]
    #[error("caller `{caller}` may not invoke operation `{operation}` on `{owner}`")]
    OperationUnauthorized {
        caller: String,
        owner: String,
        operation: String,
    },
    #[cfg(feature = "runtime")]
    #[error("operation `{operation}` payload is unreadable: {source}")]
    OperationPayload {
        operation: String,
        #[source]
        source: harmos_contract::MessageError,
    },
    #[cfg(feature = "runtime")]
    #[error("operation `{operation}` panicked")]
    OperationPanicked { operation: String },
    #[cfg(feature = "runtime")]
    #[error("operation declaration is invalid: {source}")]
    OperationDeclaration {
        #[source]
        source: harmos_contract::Error,
    },
    #[cfg(feature = "runtime")]
    #[error("operation `{operation}` failed: {source}")]
    OperationFailed {
        operation: String,
        #[source]
        source: std::sync::Arc<dyn std::error::Error + Send + Sync>,
    },
    #[cfg(feature = "runtime")]
    #[error("operation `{operation}` could not finalize its invocation state")]
    OperationFinalization {
        operation: String,
        outcome: Option<std::sync::Arc<Error>>,
        #[source]
        source: std::sync::Arc<Error>,
    },
    #[cfg(feature = "runtime")]
    #[error("operation physical settlement failed: {source}")]
    OperationSettlement {
        #[source]
        source: std::sync::Arc<Error>,
    },
    #[cfg(all(
        feature = "runtime",
        any(feature = "load", feature = "guest-host", feature = "sidecar-host")
    ))]
    #[error("plugin lifecycle failed: {source}")]
    PluginLifecycle {
        #[source]
        source: Box<crate::load::PluginFailure>,
    },
    #[error("{domain} task failed: {source}")]
    TaskFailed {
        domain: &'static str,
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
    #[error("{domain} `{name}` is not registered")]
    NotRegistered { domain: &'static str, name: String },
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
    #[error("positions belong to different store histories")]
    DifferentHistory,
    #[error("the read view has expired")]
    ViewExpired,
    #[error("retained data begins at {available}, but {requested} was requested")]
    Gap { requested: u64, available: u64 },

    #[error("required journal capture failed")]
    CaptureFailed,
    #[error("the requested state boundary splits a logical commit")]
    IncompleteCommit,
    #[error("the logical commit could not be prepared: {0}")]
    Preparation(PreparationError),
    #[error("the transaction was refused: {0}")]
    Rejected(E),
    #[error("the undo conflicts with the current state: {0}")]
    UndoConflict(E),
    #[error("the redo conflicts with the current state: {0}")]
    RedoConflict(E),
    #[error("this principal has nothing to undo in scope")]
    NothingToUndo,
    #[error("this principal has nothing to redo in scope")]
    NothingToRedo,
    #[error("the supplied origin is above the requested position")]
    OriginAhead,
    #[error("the requested query frontier is above the applied position")]
    FrontierAhead,
    #[error("the requested position is beneath the retained-history horizon")]
    BeneathHorizon,
    #[error("the requested position is below the in-memory tail's floor")]
    BeyondTail,
    #[error("the journal is not admitting requests")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendFailure {
    Transient,
    Permanent,
    Corrupt,
    Conflict,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreparationError {
    #[error("required journal capture failed")]
    CaptureFailed,
    #[error("a commit member could not be encoded")]
    Encoding,
    #[error("the commit exceeds its record or payload limit")]
    Limit,
    #[error("an emitted record is not a current member of the frozen runtime catalog")]
    Membership,
    #[error("a record has an invalid or reserved source identity")]
    Identity,
}

/* === Traits === */

/* === Implementations === */
impl<E> From<harmos_contract::operations::ContextError> for Error<E> {
    fn from(error: harmos_contract::operations::ContextError) -> Self {
        use harmos_contract::operations::ContextError;
        match error {
            ContextError::Duplicate { name } => Self::AlreadyRegistered {
                domain: "call context",
                name,
            },
            ContextError::Missing { name } => Self::NotRegistered {
                domain: "call context",
                name,
            },
            ContextError::Decode { .. } => Self::InvalidInput {
                field: "call context",
                reason: "portable value is unreadable",
            },
            ContextError::Budget => Self::InvalidInput {
                field: "call context",
                reason: "portable values exceed their budget",
            },
        }
    }
}

impl<E> Error<E> {
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
