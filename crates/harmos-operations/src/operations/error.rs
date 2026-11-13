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

pub type Result<T, E = Error> = std::result::Result<T, E>;

/* === Structs === */

/* === Enums === */

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
    Identifier(#[from] harmos_domain::IdentifierError),

    #[error(transparent)]
    Message(#[from] harmos_domain::MessageError),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
