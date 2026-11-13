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
use std::time::Duration;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

pub const UNBOUNDED: Duration = Duration::from_secs(1 << 32);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationPolicy {
    // Maximum Silence Between Stream Items, Or Total Duration Without Streams.
    pub timeout: Duration,

    // Total Attempt Bound For Operations Without Streams.
    pub attempt_timeout: Duration,

    // Grace After Cancellation; Zero Preserves Immediate Abort.
    pub cancellation_timeout: Duration,

    pub retries: RetryPolicy,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RetryPolicy {
    Never,
    OnTransient { total_attempts: usize, backoff: Backoff },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Backoff {
    Fixed { delay: Duration, jitter: Jitter },
    Exponential { initial: Duration, maximum: Duration, jitter: Jitter },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Jitter {
    None,
    Deterministic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Permanent,
    Transient,
    Ambiguous,
    Unauthorized,
    Invalid,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplaySafety {
    Unknown,
    Idempotent,
    Deduplicated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("invalid protobuf policy duration")]
    InvalidDuration,

    #[error("operation deadlines must be positive (overall {timeout:?}, attempt {attempt_timeout:?})")]
    Deadline { timeout: Duration, attempt_timeout: Duration },

    #[error("retry requires at least two total attempts; use Never for one attempt")]
    EmptyRetries,

    #[error("a restart policy needs positive credit and sustained health; use Never for zero")]
    EmptyRestarts,

    #[error("retry delays must be positive and ordered (initial {initial:?}, maximum {maximum:?})")]
    Backoff { initial: Duration, maximum: Duration },
}

/* === Traits === */

/* === Implementations === */

impl OperationPolicy {
    pub fn validate(self) -> Result<(), PolicyError> {
        if self.timeout.is_zero() || self.attempt_timeout.is_zero() {
            return Err(PolicyError::Deadline {
                timeout: self.timeout,
                attempt_timeout: self.attempt_timeout,
            });
        }

        self.retries.validate()
    }
}

impl RetryPolicy {
    pub fn validate(self) -> Result<(), PolicyError> {
        match self {
            Self::Never => Ok(()),
            Self::OnTransient { total_attempts: 0 | 1, .. } => Err(PolicyError::EmptyRetries),
            Self::OnTransient { backoff, .. } => backoff.validate(),
        }
    }

    pub fn retry_after(self, failed_attempt: usize, failure: FailureKind, replay: ReplaySafety, entropy: u64) -> Option<Duration> {
        let Self::OnTransient { total_attempts, backoff } = self else {
            return None;
        };
        if self.validate().is_err() || failed_attempt.saturating_add(1) >= total_attempts || replay == ReplaySafety::Unknown {
            return None;
        }
        if !matches!(failure, FailureKind::Transient) && !(failure == FailureKind::Ambiguous && replay != ReplaySafety::Unknown) {
            return None;
        }

        backoff.delay(failed_attempt, entropy).ok()
    }
}

impl Backoff {
    pub fn validate(self) -> Result<(), PolicyError> {
        let (initial, maximum) = match self {
            Self::Fixed { delay, .. } => (delay, delay),
            Self::Exponential { initial, maximum, .. } => (initial, maximum),
        };
        if initial.is_zero() || maximum < initial {
            return Err(PolicyError::Backoff { initial, maximum });
        }

        Ok(())
    }

    pub fn delay(self, failed_attempt: usize, entropy: u64) -> Result<Duration, PolicyError> {
        self.validate()?;

        let (nominal, maximum, jitter) = match self {
            Self::Fixed { delay, jitter } => (delay, delay.saturating_add(from_nanos(delay.as_nanos() / 4)), jitter),
            Self::Exponential { initial, maximum, jitter } => {
                let shift = u32::try_from(failed_attempt).unwrap_or(u32::MAX);
                let nanos = if shift >= 128 || initial.as_nanos() > (u128::MAX >> shift) {
                    maximum.as_nanos()
                } else {
                    (initial.as_nanos() << shift).min(maximum.as_nanos())
                };

                (from_nanos(nanos), maximum, jitter)
            }
        };
        if jitter == Jitter::None {
            return Ok(nominal.min(maximum));
        }
        let mixed = u128::from(entropy)
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add((failed_attempt as u128).wrapping_mul(17).wrapping_add(11));
        let nanos = mixed % (nominal.as_nanos() / 4 + 1);

        Ok(nominal.saturating_add(from_nanos(nanos)).min(maximum))
    }
}

impl TryFrom<Option<&crate::v1::OperationPolicy>> for OperationPolicy {
    type Error = PolicyError;
    fn try_from(wire: Option<&crate::v1::OperationPolicy>) -> Result<Self, Self::Error> {
        let wire = wire.cloned().unwrap_or_default();
        let duration = |value: Option<prost_types::Duration>, default| -> Result<Duration, PolicyError> {
            value.map(Duration::try_from).transpose().map(|value| value.unwrap_or(default)).map_err(|_| PolicyError::InvalidDuration)
        };
        let policy = Self {
            timeout: duration(wire.timeout, UNBOUNDED)?,
            attempt_timeout: duration(wire.attempt_timeout, UNBOUNDED)?,
            cancellation_timeout: duration(wire.cancellation_timeout, Duration::ZERO)?,
            retries: match wire.retries {
                None => RetryPolicy::Never,
                Some(retry) if retry.total_attempts == 0 => return Err(PolicyError::EmptyRetries),
                Some(retry) if retry.total_attempts == 1 => RetryPolicy::Never,
                Some(retry) => {
                    let initial = duration(retry.initial, Duration::ZERO)?;
                    let jitter = if retry.deterministic_jitter { Jitter::Deterministic } else { Jitter::None };
                    let backoff = match retry.maximum {
                        Some(maximum) => Backoff::Exponential {
                            initial,
                            maximum: duration(Some(maximum), Duration::ZERO)?,
                            jitter,
                        },
                        None => Backoff::Fixed { delay: initial, jitter },
                    };
                    RetryPolicy::OnTransient {
                        total_attempts: usize::try_from(retry.total_attempts).map_err(|_| PolicyError::EmptyRetries)?,
                        backoff,
                    }
                }
            },
        };
        policy.validate()?;
        Ok(policy)
    }
}

/* === Public Functions === */

/* === Private Functions === */

// Every caller bounds nanos by a representable Duration before this conversion.
fn from_nanos(nanos: u128) -> Duration {
    Duration::new((nanos / 1_000_000_000) as u64, (nanos % 1_000_000_000) as u32)
}

/* === Tests === */
