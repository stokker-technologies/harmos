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
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::Resources;
use harmos::operations::{OperationPolicy, ReplaySafety, RestartPolicy, RetryPolicy, Visibility};
use harmos::sidecar::{Refusal, Supervisor};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(5),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Read {
    resources: Arc<Resources>,
}

#[harmos::stream(
    description = "One measurement this sidecar publishes, declared here rather than shared.\nDeclared in the sidecar's own crate on purpose: an application that shares a row type with its host has the host's crate to put it in, and an application that does not must still be able to declare one.",
    id = "sample",
    window = 16,
    batch(samples = 4, gaps = "report")
)]
#[harmos::message]
#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Sample {
    pub instrument: String,

    #[harmos(measure)]
    pub value: i32,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Request {
    pub gain: u32,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Value {
    pub value: i32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation(
    id = "standalone-bridge.get_reading",
    version = 1,
    policy = POLICY,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown
)]
impl Read {
    pub async fn initialize(resources: Arc<Resources>, _: &Supervisor) -> Self {
        Self { resources }
    }

    async fn run(&mut self, context: &Supervisor, request: &Request) -> Result<Value, Refusal> {
        let value = measurement(request.gain)?;
        context
            .publisher::<crate::StandaloneBridge>()
            .publish_on::<Sample>(&Sample {
                instrument: self.resources.instrument.clone(),
                value,
            })
            .await
            .map_err(Refusal::from)?;
        Ok(Value { value })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn measurement(gain: u32) -> Result<i32, Refusal> {
    i32::try_from(gain)
        .ok()
        .and_then(|gain| gain.checked_mul(1_000))
        .ok_or_else(|| Refusal::of("instrument gain exceeds measurement range"))
}

/* === Tests === */
