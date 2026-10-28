/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resident Service Stop Deadlines And Restart Credit

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::operations::policy::{Backoff, PolicyError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicePolicy {
    pub stop_timeout: Duration,
    pub restart: RestartPolicy,
}

// Failed Residents Spend Restart Credit; Sustained Health Replenishes It.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RestartPolicy {
    Never,
    OnFailure { credit: usize, sustained_health: Duration, backoff: Backoff },
}

impl ServicePolicy {
    pub fn validate(self) -> Result<(), PolicyError> {
        if self.stop_timeout.is_zero() {
            return Err(PolicyError::Deadline {
                timeout: self.stop_timeout,
                attempt_timeout: self.stop_timeout,
            });
        }
        self.restart.validate()
    }
}

impl RestartPolicy {
    pub fn validate(self) -> Result<(), PolicyError> {
        match self {
            Self::Never => Ok(()),
            Self::OnFailure { credit, sustained_health, .. } if credit == 0 || sustained_health.is_zero() => {
                Err(PolicyError::EmptyRestarts)
            }
            Self::OnFailure { backoff, .. } => backoff.validate(),
        }
    }

    pub const fn credit(self) -> usize {
        match self {
            Self::Never => 0,
            Self::OnFailure { credit, .. } => credit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Backoff;
    use crate::Jitter;

    #[test]
    fn service_credit_and_backoff() {
        let delay = Duration::from_millis(10);
        let backoff = Backoff::Exponential {
            initial: delay,
            maximum: delay * 4,
            jitter: Jitter::None,
        };
        let restart = RestartPolicy::OnFailure {
            credit: 2,
            sustained_health: Duration::from_secs(1),
            backoff,
        };
        assert_eq!(restart.credit(), 2);
        assert!(ServicePolicy { stop_timeout: delay, restart }.validate().is_ok());
        assert!(ServicePolicy { stop_timeout: Duration::ZERO, restart }.validate().is_err());
        assert!(
            RestartPolicy::OnFailure {
                credit: 0,
                sustained_health: delay,
                backoff
            }
            .validate()
            .is_err()
        );
        assert_eq!(backoff.delay(0, 0).unwrap(), delay);
        assert_eq!(backoff.delay(usize::MAX, 0).unwrap(), delay * 4);
    }
}
