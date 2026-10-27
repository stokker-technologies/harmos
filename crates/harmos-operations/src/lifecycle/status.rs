/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

#[cfg(feature = "runtime")]
use crate::{Error, Operation, OperationContext, Response};
use harmos_domain::operations::instance::Id;

/* === Structs === */

#[harmos::message(harmos_path = crate)]
#[derive(Clone, Default, PartialEq)]
pub struct StatusRequest {
    pub instance: u64,
}

#[harmos::operation(harmos_path = crate, id = "host_operations_status", request = StatusRequest,
    response = super::InstanceState, policy = super::POLICY, infer_request,
    visibility = harmos_domain::operations::Visibility::Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Status"))]
pub struct Status;

/* === Implementations === */

impl StatusRequest {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Status {
    type Request = StatusRequest;
    type Response = super::InstanceState;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(_: OperationContext) -> Result<Self, Error> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<Response<Self::Response>, Error> {
        let access = context.dependency::<harmos_sdk::operations::instances::InstanceAccess>()?;
        let instance = access.0.owned(Id::new(request.instance)).ok_or(Error::UnknownInstance(request.instance))?;
        let _ = output;
        Ok(super::InstanceState::from(instance.status()).into())
    }
}

#[harmos::message(harmos_path = crate)]
#[derive(Clone, Default, PartialEq)]
pub struct InstanceState {
    pub phase: String,
    pub outcome: String,
    pub cancellation_requested: bool,
    pub attempt: u32,
}
impl From<harmos_domain::operations::instance::Status> for InstanceState {
    fn from(status: harmos_domain::operations::instance::Status) -> Self {
        Self {
            phase: status.phase.as_str().to_owned(),
            outcome: status.outcome.map(harmos_domain::operations::instance::Outcome::as_str).unwrap_or_default().to_owned(),
            cancellation_requested: status.cancellation_requested,
            attempt: status.attempt,
        }
    }
}

impl TryFrom<&InstanceState> for harmos_domain::operations::instance::Status {
    type Error = harmos_domain::operations::instance::ParseError;

    fn try_from(state: &InstanceState) -> Result<Self, Self::Error> {
        let phase = state.phase.parse::<harmos_domain::operations::instance::Phase>()?;
        let outcome = match state.outcome.as_str() {
            "" => None,
            outcome => Some(outcome.parse::<harmos_domain::operations::instance::Outcome>()?),
        };
        Ok(Self { phase, outcome, cancellation_requested: state.cancellation_requested, attempt: state.attempt })
    }
}
