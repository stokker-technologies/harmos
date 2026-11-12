/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_protocol::v1;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", .0.message)]
    Failure(Box<v1::Failure>),

    #[error(transparent)]
    Transport(#[from] tonic::Status),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Server(#[from] tonic::transport::Error),

    #[error("{0}")]
    Invalid(String),
}

/* === Traits === */

/* === Implementations === */

impl From<v1::Failure> for Error {
    fn from(failure: v1::Failure) -> Self {
        Self::Failure(Box::new(failure))
    }
}

impl Error {
    pub fn failure(
        classification: v1::Classification, code: impl Into<String>, message: impl Into<String>, action: impl Into<String>,
    ) -> Self {
        v1::Failure {
            classification: classification as i32,
            code: code.into(),
            message: message.into(),
            action: action.into(),
            ..Default::default()
        }
        .into()
    }

    #[cfg(feature = "plugin")]
    pub(crate) fn response(self, operation: &str, plugin: &str) -> std::result::Result<v1::response::Result, tonic::Status> {
        match self {
            Self::Transport(status) => Err(status),
            Self::Failure(mut failure) => {
                if failure.operation.is_empty() {
                    failure.operation = operation.into();
                }
                if failure.plugin.is_empty() {
                    failure.plugin = plugin.into();
                }
                Ok(v1::response::Result::Failure(*failure))
            }
            error => Ok(v1::response::Result::Failure(v1::Failure {
                classification: v1::Classification::Permanent as i32,
                code: "sdk_error".into(),
                message: error.to_string(),
                action: "Check Plugin Configuration".into(),
                operation: operation.into(),
                plugin: plugin.into(),
                ..Default::default()
            })),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
