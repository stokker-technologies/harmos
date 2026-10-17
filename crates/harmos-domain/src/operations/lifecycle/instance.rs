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
use super::{Cancel, InstanceState, Status, Watch};
use crate::operations::instance::{self, Id, Outcome, Phase};
use crate::operations::{OperationPolicy, Visibility};

/* === Constants === */
// A watch lasts until its instance settles or the caller cancels it.
const WATCH_POLICY: OperationPolicy = OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    ..crate::operations::host::POLICY
};

/* === Macros === */
crate::declare!(Status {
    id: "host.operations.status",
    version: 1,
    policy: crate::operations::host::POLICY,
    response: InstanceState,
    description: "Report the status of an operation instance the caller submitted",
    visibility: Visibility::Shared,
    grpc: ("harmos.host.v1", "Operations", "Status"),
});

crate::declare!(Cancel {
    id: "host.operations.cancel",
    version: 1,
    policy: crate::operations::host::POLICY,
    response: InstanceState,
    description: "Request cancellation of an operation instance the caller submitted",
    visibility: Visibility::Shared,
    grpc: ("harmos.host.v1", "Operations", "Cancel"),
});

crate::declare!(Watch {
    id: "host.operations.watch",
    version: 1,
    policy: WATCH_POLICY,
    response: InstanceState,
    output: InstanceState,
    description: "Stream the status changes of an operation instance the caller submitted until it settles",
    visibility: Visibility::Shared,
    grpc: ("harmos.host.v1", "Operations", "Watch"),
});

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Status {
    pub fn of(instance: Id) -> Self {
        Self {
            instance: instance.value(),
        }
    }
}

impl Watch {
    pub fn of(instance: Id) -> Self {
        Self {
            instance: instance.value(),
        }
    }
}

impl Cancel {
    pub fn of(instance: Id) -> Self {
        Self {
            instance: instance.value(),
        }
    }
}

impl From<instance::Status> for InstanceState {
    fn from(status: instance::Status) -> Self {
        Self {
            phase: status.phase.as_str().to_owned(),
            outcome: status
                .outcome
                .map(Outcome::as_str)
                .unwrap_or_default()
                .to_owned(),
            cancellation_requested: status.cancellation_requested,
            attempt: status.attempt,
        }
    }
}

impl TryFrom<&InstanceState> for instance::Status {
    type Error = instance::ParseError;

    fn try_from(state: &InstanceState) -> Result<Self, Self::Error> {
        let phase = state.phase.parse::<Phase>()?;
        let outcome = match state.outcome.as_str() {
            "" => None,
            outcome => Some(outcome.parse::<Outcome>()?),
        };
        Ok(Self {
            phase,
            outcome,
            cancellation_requested: state.cancellation_requested,
            attempt: state.attempt,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
