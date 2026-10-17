/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable identity, status and handle contract of one submitted operation.
// Every host binding reports the same status projection; the in-process
// manager keeps its richer status alongside it.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt;
use std::future::Future;
use std::str::FromStr;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::{OperationDeclaration, Response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub outcome: Option<Outcome>,
    pub cancellation_requested: bool,
    pub attempt: u32,
}

/* === Enums === */
// Resident phases are reserved for long-running declarations; request-scoped
// operations never report them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Queued,
    Initializing,
    Running,
    RetryWaiting,
    Restarting,
    Stopping,
    CleaningUp,
    Stopped,
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown operation instance {kind} `{value}`")]
pub struct ParseError {
    kind: &'static str,
    value: String,
}

/* === Traits === */
// One submitted operation. Dropping an unsettled instance requests its
// cancellation; `detach` hands the instance to the host and keeps it running.
pub trait Instance<O: OperationDeclaration>: Send + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn id(&self) -> Id;

    fn status(&self) -> impl Future<Output = Result<Status, Self::Error>> + Send;

    fn cancel(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send;

    fn cancel_and_join(
        self,
    ) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        async move {
            self.cancel().await?;
            self.join().await
        }
    }

    fn detach(self) -> Id;
}

/* === Implementations === */
impl Id {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Status {
    pub fn is_settled(&self) -> bool {
        matches!(self.phase, Phase::Settled | Phase::Stopped)
    }
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Initializing => "initializing",
            Self::Running => "running",
            Self::RetryWaiting => "retry_waiting",
            Self::Restarting => "restarting",
            Self::Stopping => "stopping",
            Self::CleaningUp => "cleaning_up",
            Self::Stopped => "stopped",
            Self::Settled => "settled",
        }
    }
}

impl FromStr for Phase {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, ParseError> {
        let phase = match value {
            "queued" => Self::Queued,
            "initializing" => Self::Initializing,
            "running" => Self::Running,
            "retry_waiting" => Self::RetryWaiting,
            "restarting" => Self::Restarting,
            "stopping" => Self::Stopping,
            "cleaning_up" => Self::CleaningUp,
            "stopped" => Self::Stopped,
            "settled" => Self::Settled,
            _ => {
                return Err(ParseError {
                    kind: "phase",
                    value: value.to_owned(),
                });
            }
        };
        Ok(phase)
    }
}

impl Outcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Unavailable => "unavailable",
        }
    }
}

impl FromStr for Outcome {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, ParseError> {
        let outcome = match value {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "timed_out" => Self::TimedOut,
            "unavailable" => Self::Unavailable,
            _ => {
                return Err(ParseError {
                    kind: "outcome",
                    value: value.to_owned(),
                });
            }
        };
        Ok(outcome)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
