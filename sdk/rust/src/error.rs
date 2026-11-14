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
use std::{io::Error as IoError, result::Result as StdResult};

// Third Party Imports
use tonic::Status;
#[cfg(not(target_arch = "wasm32"))]
use tonic::transport::Error as ServerError;

// Local Imports
use crate::{Classification, Failure};
use harmos_protocol::v1::response::Result as ResponseResult;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = StdResult<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", .0.message)]
    Failure(Box<Failure>),

    #[error(transparent)]
    Transport(#[from] Status),

    #[error(transparent)]
    Io(#[from] IoError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Server(#[from] ServerError),

    #[error("{0}")]
    Invalid(String),
}

/* === Traits === */

/* === Implementations === */

impl From<Failure> for Error {
    fn from(failure: Failure) -> Self {
        Self::Failure(Box::new(failure))
    }
}

impl Error {
    pub fn failure(classification: Classification, code: impl Into<String>, message: impl Into<String>, action: impl Into<String>) -> Self {
        Failure {
            classification: classification as i32,
            code: code.into(),
            message: message.into(),
            action: action.into(),
            ..Default::default()
        }
        .into()
    }

    pub(crate) fn response(self, operation: &str, plugin: &str) -> StdResult<ResponseResult, Status> {
        match self {
            Self::Transport(status) => Err(status),
            Self::Failure(mut failure) => {
                if failure.operation.is_empty() {
                    failure.operation = operation.into();
                }
                if failure.plugin.is_empty() {
                    failure.plugin = plugin.into();
                }
                Ok(ResponseResult::Failure(*failure))
            }
            error => Ok(ResponseResult::Failure(Failure {
                classification: Classification::Permanent as i32,
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
