/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native operation errors

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
    #[error("invalid analysis input")]
    InvalidInput,

    #[error("analysis resources are unavailable")]
    Unavailable,

    #[error(transparent)]
    Host(#[from] harmos_sidecar::CallError),

    #[error(transparent)]
    Output(#[from] harmos_sidecar::operations::streaming::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Directory name is not UTF-8: {0:?}")]
    DirectoryName(std::ffi::OsString),
}

/* === Traits === */

/* === Implementations === */

impl Error {
    pub fn kind(&self) -> harmos_sidecar::operations::FailureKind {
        match self {
            Self::InvalidInput => harmos_sidecar::operations::FailureKind::Invalid,
            _ => harmos_sidecar::operations::FailureKind::Permanent,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
