/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Guest Child Status Payload Until the Guest Protocol Cutover.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Debug)]
#[harmos_path(harmos_domain)]
#[derive(Clone, Default, PartialEq)]
pub struct InstanceState {
    pub phase: String,
    pub outcome: String,
    pub cancellation_requested: bool,
    pub attempt: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<harmos_operations::operations::instance::Status> for InstanceState {
    fn from(status: harmos_operations::operations::instance::Status) -> Self {
        Self {
            phase: status.phase.as_str().to_owned(),
            outcome: status.outcome.map(harmos_operations::operations::instance::Outcome::as_str).unwrap_or_default().to_owned(),
            cancellation_requested: status.cancellation_requested,
            attempt: status.attempt,
        }
    }
}

impl TryFrom<&InstanceState> for harmos_operations::operations::instance::Status {
    type Error = harmos_operations::operations::instance::ParseError;

    fn try_from(state: &InstanceState) -> Result<Self, Self::Error> {
        let phase = state.phase.parse::<harmos_operations::operations::instance::Phase>()?;
        let outcome = match state.outcome.as_str() {
            "" => None,
            outcome => Some(outcome.parse::<harmos_operations::operations::instance::Outcome>()?),
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
