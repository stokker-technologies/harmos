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
use crate::operations::{
    OperationDeclaration, OperationPolicy, OutputDeclaration, Visibility, WireOperation,
};
use crate::schema::Described;

/* === Constants === */
// A watch lasts until its instance settles or the caller cancels it.
const WATCH_POLICY: OperationPolicy = OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    ..crate::operations::host::POLICY
};

/* === Macros === */

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

impl OperationDeclaration for Status {
    type Request = Self;
    type Response = InstanceState;
    const ID: &'static str = "host.operations.status";
    const POLICY: OperationPolicy = crate::operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const DESCRIPTION: &'static str =
        "Report the status of an operation instance the caller submitted";
}

impl WireOperation for Status {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new(
        "harmos.host.v1",
        "Operations",
        "Status",
    ));

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl OperationDeclaration for Cancel {
    type Request = Self;
    type Response = InstanceState;
    const ID: &'static str = "host.operations.cancel";
    const POLICY: OperationPolicy = crate::operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const DESCRIPTION: &'static str =
        "Request cancellation of an operation instance the caller submitted";
}

impl WireOperation for Cancel {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new(
        "harmos.host.v1",
        "Operations",
        "Cancel",
    ));

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl OperationDeclaration for Watch {
    type Request = Self;
    type Response = InstanceState;
    const ID: &'static str = "host.operations.watch";
    const POLICY: OperationPolicy = WATCH_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const DESCRIPTION: &'static str =
        "Stream the status changes of an operation instance the caller submitted until it settles";

    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<InstanceState>()))
    }
}

impl OutputDeclaration for Watch {
    type Item = InstanceState;
}

impl WireOperation for Watch {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(InstanceState::DEFINITION);
    #[cfg(feature = "grpc")]
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new(
        "harmos.host.v1",
        "Operations",
        "Watch",
    ));

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.download::<Self>())
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
