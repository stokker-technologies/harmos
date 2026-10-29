/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{id}` is not a local portable identity")]
    InvalidOperation { id: String },

    #[error(transparent)]
    OperationPolicy(#[from] crate::operations::PolicyError),

    #[error("operation `{id}` appears more than once in the catalog")]
    DuplicateOperation { id: String },

    #[error("operation `{id}` uses the reserved facility namespace")]
    ReservedOperation { id: String },

    #[error(transparent)]
    Config(#[from] crate::ConfigError),

    #[error(transparent)]
    Identifier(#[from] crate::IdentifierError),

    #[error(transparent)]
    Message(#[from] crate::MessageError),

    #[error(transparent)]
    Registration(#[from] crate::RegistrationError),

    #[error(transparent)]
    Scan(#[from] crate::ScanError),

    #[error(transparent)]
    Value(#[from] crate::ValueError),

    #[error("invalid {field}: {reason}")]
    InvalidInput { field: &'static str, reason: &'static str },

    #[error("{resource} capacity is exhausted")]
    ResourceExhausted { resource: &'static str },

    #[error("positions belong to different store histories")]
    DifferentHistory,

    #[error("{operation} found a corrupt stored value: {source}")]
    Corrupt {
        operation: &'static str,

        #[source]
        source: crate::MessageError,
    },
}
