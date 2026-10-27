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

#[cfg(feature = "runtime")]
use harmos_sdk::operations::instances::InstanceAccess;

#[cfg(feature = "runtime")]
use crate::{Error, Operation, Response};
use harmos_domain::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    harmos_path = crate,
    resources = [InstanceAccess],
    id = "host.operations.status",
    response = super::InstanceState,
    policy = super::POLICY,
    visibility = Shared,
    grpc(package = "harmos.host.v1", service = "Operations", method = "Status"),
)]
#[derive(Clone, Default, PartialEq)]
pub struct Status {
    pub instance: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Status {
    pub fn of(instance: Id) -> Self {
        Self { instance: instance.value() }
    }
}

#[cfg(feature = "runtime")]
impl Operation for Status {
    type Error = Error;

    async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<super::InstanceState>, Error> {
        let access = context.resource::<harmos_sdk::operations::instances::InstanceAccess>();

        let instance = access.0.owned(Id::new(self.instance)).ok_or(Error::UnknownInstance(self.instance))?;
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
