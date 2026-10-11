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
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
