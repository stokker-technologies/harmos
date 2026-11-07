/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Failures Translated At The C ABI Boundary

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidInput(&'static str),

    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Operation(harmos_domain::Failure),

    #[error("Rust panic contained at the Harmos ABI")]
    Panic,
}

/* === Traits === */

/* === Implementations === */

impl Error {
    // The Prototype ABI Uses One Foreign Failure Code For Every Rust Failure.
    pub(crate) fn code(&self) -> i32 {
        1
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
