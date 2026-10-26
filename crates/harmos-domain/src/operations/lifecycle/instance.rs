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
use super::{CancelRequest, InstanceState, StatusRequest, WatchRequest};
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

pub struct Status;

impl crate::operations::OperationDeclaration for Status {
    type Request = StatusRequest;
    type Response = InstanceState;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "host_operations_status";
    const POLICY: crate::operations::OperationPolicy = crate::operations::host::POLICY;
    const DESCRIPTION: &'static str = "Report the status of an operation instance the caller submitted";
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.host.v1", "Operations", "Status"));
}

impl crate::operations::RequestFor for StatusRequest {
    type Operation = Status;
}

pub struct Cancel;

impl crate::operations::OperationDeclaration for Cancel {
    type Request = CancelRequest;
    type Response = InstanceState;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "host_operations_cancel";
    const POLICY: crate::operations::OperationPolicy = crate::operations::host::POLICY;
    const DESCRIPTION: &'static str = "Request cancellation of an operation instance the caller submitted";
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.host.v1", "Operations", "Cancel"));
}

impl crate::operations::RequestFor for CancelRequest {
    type Operation = Cancel;
}

pub struct Watch;

impl crate::operations::OperationDeclaration for Watch {
    type Request = WatchRequest;
    type Response = InstanceState;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<InstanceState>;
    const ID: &'static str = "host_operations_watch";
    const POLICY: crate::operations::OperationPolicy = WATCH_POLICY;
    const DESCRIPTION: &'static str = "Stream the status changes of an operation instance the caller submitted until it settles";
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.host.v1", "Operations", "Watch"));
}

impl crate::operations::RequestFor for WatchRequest {
    type Operation = Watch;
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StatusRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

impl WatchRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

impl CancelRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

impl From<instance::Status> for InstanceState {
    fn from(status: instance::Status) -> Self {
        Self {
            phase: status.phase.as_str().to_owned(),
            outcome: status.outcome.map(Outcome::as_str).unwrap_or_default().to_owned(),
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
        Ok(Self { phase, outcome, cancellation_requested: state.cancellation_requested, attempt: state.attempt })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
