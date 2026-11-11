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
    #[error(transparent)]
    Identifier(#[from] crate::IdentifierError),

    #[error(transparent)]
    Message(#[from] crate::MessageError),

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

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
