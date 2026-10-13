/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Declarations of the operations Harmos implements over its own state. A host
// serves none of them until it registers them explicitly; kits call them by
// id through the ordinary host call path.

/* === Modules === */

/* === Re-Exports === */
pub use crate::operations::host::builtin::{Cancel, InstanceState, Status};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::instance::{self, Id, Outcome, Phase};
use crate::operations::{OperationDeclaration, OperationPolicy, Visibility, WireOperation};

/* === Constants === */

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
