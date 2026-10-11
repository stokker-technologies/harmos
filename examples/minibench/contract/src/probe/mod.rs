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
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::operations::{
    OperationDeclaration, OperationPolicy, ReplaySafety, RetryPolicy, Visibility,
};

/* === Constants === */
pub const READING: (&str, u32) = ("probe-bridge.reading", 1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ReadSample;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Settings {
    pub device: String,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Sample {
    pub channel: u32,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Sampled {
    pub millivolts: i32,
}

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    ReadSample(ReadSample),
}

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ReadSample {
    type Request = Sample;
    type Response = Sampled;
    const ID: &'static str = "probe-bridge.get_sample";
    const DESCRIPTION: &'static str = "Samples and publishes one probe channel";
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
    };
}

impl harmos::operations::WireOperation for ReadSample {
    const VERSION: u32 = 1;
}

impl Settings {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            device: device.into(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
