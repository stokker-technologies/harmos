/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod codec;

/* === Re-Exports === */

pub use codec::WireCodecError;

/* === Imports === */
use harmos_operations::execution::operations::{CallValue, RetryPolicy};
use serde::{Deserialize, Serialize};

/* === Structs === */

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireChildCall {
    pub context: [u8; 32],

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_context: Option<[u8; 32]>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<CallValue>,

    pub operation: String,
    pub fingerprint: [u8; 32],
    pub generation: u64,
    pub payload: Vec<u8>,

    // Submitted children answer their instance id at admission and remain
    // owned by the calling attempt until joined, released or detached.
    #[serde(default)]
    pub submit: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
}

// Drives one child the same attempt submitted; the host resolves the instance
// only inside the attempt context that submitted it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireChildControl {
    pub context: [u8; 32],
    pub instance: u64,
    pub action: WireChildAction,
}

/* === Enums === */
// Answers: `Join` the child's response payload, `Status` and `Cancel` an
// encoded instance state, `Release` and `Detach` an empty payload.
// Releasing cancels the child; detaching leaves it running without a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireChildAction {
    Join,
    Status,
    Cancel,
    Release,
    Detach,
}

#[derive(Debug)]
pub enum WireChild {
    Call(WireChildCall),
    Control(WireChildControl),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireCommand {
    Prepare { invocation: u64, operation: String, fingerprint: [u8; 32], generation: u64, payload: Vec<u8> },
    Initialize { invocation: u64, context: [u8; 32] },
    Attempt { invocation: u64, context: [u8; 32], values: Vec<CallValue> },
    Finalize { invocation: u64 },
    StreamAttempt { invocation: u64, epoch: u64, context: [u8; 32], values: Vec<CallValue> },
    StreamSend { invocation: u64, epoch: u64, payload: Vec<u8> },
    StreamFinish { invocation: u64, epoch: u64 },
    StreamReceive { invocation: u64, epoch: u64 },
    Cancel { invocation: u64 },
    PrepareGrpc { invocation: u64, operation: String, fingerprint: [u8; 32], generation: u64, payload: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireAnswer {
    Stream { epoch: u64, transferred: bool, payload: Option<Vec<u8>> },
    Ready { payload: Vec<u8> },
    Failed { failure: harmos_domain::Failure },
}

/* === Traits === */

/* === Implementations === */
impl WireCommand {
    pub const LIMIT: usize = codec::LIMIT;
}

/* === Public Functions === */

pub fn decode_prepared_grant(payload: &[u8]) -> Result<Option<harmos_domain::plugin::Grant>, serde_json::Error> {
    if payload.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(payload).map(Some)
}

/* === Private Functions === */

/* === Tests === */
