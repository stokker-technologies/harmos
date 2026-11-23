/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Command Failures and Their Exit Codes.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The descriptors break a protocol contract; the message names the correction.
    #[error("{0}")]
    Contract(String),

    /// The inputs cannot be read as the command requires.
    #[error("{0}")]
    Input(String),

    #[error("{action} {path}: {source}")]
    Io { action: &'static str, path: PathBuf, source: io::Error },
}

/* === Traits === */

/* === Implementations === */

impl Error {
    pub(crate) fn exit_code(&self) -> ExitCode {
        match self {
            Self::Contract(_) => ExitCode::from(1),
            Self::Input(_) => ExitCode::from(2),
            Self::Io { .. } => ExitCode::from(3),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
