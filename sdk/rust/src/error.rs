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
use std::{error, io, result, sync::Arc};

// Third Party Imports
use tonic::Status;

// Local Imports
use crate::model::Failure;
use crate::{Classification, OperationRetry};
use harmos_protocol::plugin::response;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type FailureMapper = Arc<dyn Fn(&(dyn error::Error + 'static)) -> Option<Error> + Send + Sync>;

pub type Result<T> = result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Renders "message; action" When the Failure Names an Action.
    #[error("{}{}{}", .0.message, if .0.action.is_empty() { "" } else { "; " }, .0.action)]
    Failure(Box<Failure>),

    #[error(transparent)]
    Transport(#[from] Status),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Other(Box<dyn error::Error + Send + Sync>),

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Server(#[from] tonic::transport::Error),

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
    pub(crate) fn mapped(self, mapper: Option<&FailureMapper>) -> Self {
        if matches!(&self, Self::Failure(_)) {
            return self;
        }
        let source: &(dyn error::Error + 'static) = match &self {
            Self::Other(source) => source.as_ref(),
            _ => &self,
        };
        mapper.and_then(|mapper| mapper(source)).unwrap_or(self)
    }

    /// Preserve an Application Error for the Plugin's Failure Mapper.
    pub fn other(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self::Other(Box::new(error))
    }

    pub fn kind(&self) -> &str {
        match self {
            Self::Failure(failure) => match failure.classification() {
                Classification::Transient => "transient",
                Classification::Refused => "refused",
                Classification::Invalid => "invalid",
                Classification::Cancelled | Classification::TimedOut => "cancelled",
                _ => "permanent",
            },
            Self::Invalid(_) => "invalid",
            _ => "permanent",
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::Failure(failure) => &failure.code,
            Self::Invalid(_) => "invalid",
            _ => "sdk_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Failure(failure) => failure.message.clone(),
            error => error.to_string(),
        }
    }

    pub fn action(&self) -> &str {
        match self {
            Self::Failure(failure) => &failure.action,
            _ => "Inspect Plugin Diagnostics",
        }
    }

    pub fn operation(&self) -> &str {
        match self {
            Self::Failure(failure) => &failure.operation,
            _ => "",
        }
    }

    pub fn retry(&self) -> OperationRetry {
        match self {
            Self::Failure(failure) => failure.retry.clone().unwrap_or_default(),
            _ => OperationRetry::default(),
        }
    }

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

    pub(crate) fn response(self, operation: &str, plugin: &str) -> result::Result<response::Result, Status> {
        Ok(response::Result::Failure(self.into_failure(operation, plugin)))
    }

    pub(crate) fn into_failure(self, operation: &str, plugin: &str) -> Failure {
        match self {
            Self::Failure(mut failure) => {
                if failure.operation.is_empty() {
                    failure.operation = operation.into();
                }
                if failure.plugin.is_empty() {
                    failure.plugin = plugin.into();
                }
                *failure
            }
            error => Failure {
                classification: Classification::Permanent as i32,
                code: "sdk_error".into(),
                message: error.to_string(),
                action: "Check Plugin Configuration".into(),
                operation: operation.into(),
                plugin: plugin.into(),
                ..Default::default()
            },
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_name_their_action_after_the_message() {
        let error = Error::failure(Classification::Refused, "fixture_failure", "Apply Failed", "Choose Another Input");
        assert_eq!(error.to_string(), "Apply Failed; Choose Another Input");
        assert_eq!(error.message(), "Apply Failed");
        assert_eq!(error.action(), "Choose Another Input");
        assert_eq!(Error::failure(Classification::Refused, "fixture_failure", "Apply Failed", "").to_string(), "Apply Failed");
    }
}
