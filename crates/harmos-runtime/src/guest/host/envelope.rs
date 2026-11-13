/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_operations::execution::operations::{CallValue, OperationPolicy};
use harmos_protocol::{
    guest,
    v1::{self, guest_child, guest_command, response},
};
use prost::Message as _;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug)]
pub struct WireChildCall {
    pub context: [u8; 32],
    pub generation_context: Option<[u8; 32]>,
    pub values: Vec<CallValue>,
    pub operation: String,
    pub fingerprint: [u8; 32],
    pub generation: u64,
    pub payload: Vec<u8>,
    pub submit: bool,
    pub policy: Option<OperationPolicy>,
}
#[derive(Debug)]
pub struct WireChildControl {
    pub context: [u8; 32],
    pub instance: u64,
    pub action: WireChildAction,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireChildAction {
    Join,
    Status,
    Cancel,
    Release,
    Detach,
}
#[derive(Debug)]
pub enum WireChild {
    Call(Box<WireChildCall>),
    Control(WireChildControl),
}
#[derive(Debug)]
pub enum WireCommand {
    Prepare { invocation: u64, operation: String, fingerprint: [u8; 32], generation: u64, payload: Vec<u8> },
    Initialize { invocation: u64, context: [u8; 32] },
    Attempt { invocation: u64, context: [u8; 32], values: Vec<CallValue> },
    Finalize { invocation: u64 },
    Cancel { invocation: u64 },
}
#[derive(Debug)]
pub enum WireAnswer {
    Ready { payload: Vec<u8> },
    Failed { failure: harmos_operations::Failure },
}
#[derive(Debug, thiserror::Error)]
pub enum WireCodecError {
    #[error(transparent)]
    Protocol(#[from] guest::Error),

    #[error(transparent)]
    Protobuf(#[from] prost::DecodeError),

    #[error(transparent)]
    Admission(#[from] crate::Error),

    #[error("invalid guest message shape")]
    Shape,
}

/* === Traits === */

/* === Implementations === */

impl WireCommand {
    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        let phase = match self {
            Self::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => guest_command::Phase::Prepare(v1::GuestPrepare {
                invocation: *invocation,
                operation: operation.clone(),
                fingerprint: fingerprint.to_vec(),
                generation: *generation,
                payload: payload.clone(),
            }),
            Self::Initialize { invocation, context } => guest_command::Phase::Initialize(v1::GuestInvocation {
                invocation: *invocation,
                context: context.to_vec(),
                values: Vec::new(),
            }),
            Self::Attempt { invocation, context, values } => {
                let values = values
                    .iter()
                    .map(|value| v1::GuestValue {
                        key: value.key.clone(),
                        payload: value.payload.clone(),
                    })
                    .collect::<Vec<_>>();
                guest::validate_values(&values)?;
                guest_command::Phase::Attempt(v1::GuestInvocation {
                    invocation: *invocation,
                    context: context.to_vec(),
                    values,
                })
            }
            Self::Finalize { invocation } => guest_command::Phase::Finalize(*invocation),
            Self::Cancel { invocation } => guest_command::Phase::Cancel(*invocation),
        };
        Ok(guest::encode(&v1::GuestCommand { phase: Some(phase) })?)
    }
}
impl WireAnswer {
    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        if bytes.len() > guest::COMMAND_BYTES {
            return Err(WireCodecError::Shape);
        }
        Ok(match v1::Response::decode(bytes)?.result.ok_or(WireCodecError::Shape)? {
            response::Result::Payload(payload) => Self::Ready { payload },
            response::Result::Failure(failure) => Self::Failed { failure: failure.into() },
        })
    }
}
impl WireChild {
    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        Ok(match guest::decode_child(bytes)?.action.ok_or(WireCodecError::Shape)? {
            guest_child::Action::Call(call) => {
                guest::validate_values(&call.values)?;
                let request = call.request.ok_or(WireCodecError::Shape)?;
                let policy = request
                    .policy
                    .as_ref()
                    .map(|policy| harmos_protocol::policy::OperationPolicy::try_from(Some(policy)))
                    .transpose()
                    .map_err(|_| WireCodecError::Shape)?;
                Self::Call(Box::new(WireChildCall {
                    context: call.context.try_into().map_err(|_| WireCodecError::Shape)?,
                    generation_context: if call.generation_context.is_empty() {
                        None
                    } else {
                        Some(call.generation_context.try_into().map_err(|_| WireCodecError::Shape)?)
                    },
                    values: call.values.into_iter().map(|value| CallValue { key: value.key, payload: value.payload }).collect(),
                    operation: request.operation,
                    fingerprint: request.fingerprint.try_into().map_err(|_| WireCodecError::Shape)?,
                    generation: call.generation,
                    payload: request.payload,
                    submit: call.submit,
                    policy,
                }))
            }
            guest_child::Action::Control(control) => Self::Control(WireChildControl {
                context: control.context.try_into().map_err(|_| WireCodecError::Shape)?,
                instance: control.instance,
                action: match v1::GuestChildAction::try_from(control.action).map_err(|_| WireCodecError::Shape)? {
                    v1::GuestChildAction::Join => WireChildAction::Join,
                    v1::GuestChildAction::Status => WireChildAction::Status,
                    v1::GuestChildAction::Cancel => WireChildAction::Cancel,
                    v1::GuestChildAction::Release => WireChildAction::Release,
                    v1::GuestChildAction::Detach => WireChildAction::Detach,
                    _ => return Err(WireCodecError::Shape),
                },
            }),
        })
    }
    pub fn context(&self) -> &[u8; 32] {
        match self {
            Self::Call(call) => &call.context,
            Self::Control(control) => &control.context,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
