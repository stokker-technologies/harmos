/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

#![allow(dead_code, reason = "Each Guest Story Selects Its Own Fixture Messages")]

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct Echoed {
    pub said: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct Value {
    pub value: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct Stage {
    pub id: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct RetryChoice {
    pub retries: u32,
    pub delay_millis: u64,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct EchoOperation {
    pub say: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct ProviderEchoOperation {
    pub say: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.local", file = "harmos/sandbox/local.proto")]
pub struct LocalEcho {
    pub say: String,
}

impl harmos::OperationRequest for LocalEcho {
    type Response = Echoed;
    const ID: &'static str = "probe.local";
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct ReadOperation {
    pub value: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct CountsOperation {
    pub value: String,
}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct DeclineOperation {}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct FailOperation {}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct SpinOperation {}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct WaitOperation {}

#[derive(harmos::Message, Debug, Clone, PartialEq)]
#[harmos(package = "harmos.sandbox.v1", file = "harmos/sandbox/v1/probes.proto")]
pub struct FinishOperation {
    pub mode: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::OperationRequest for EchoOperation {
    type Response = Echoed;
    const ID: &'static str = "probe.echo";
}
impl harmos::OperationRequest for ProviderEchoOperation {
    type Response = Echoed;
    const ID: &'static str = "provider.echo";
}
impl harmos::OperationRequest for ReadOperation {
    type Response = Value;
    const ID: &'static str = "fixture.read";
}
impl harmos::OperationRequest for CountsOperation {
    type Response = Value;
    const ID: &'static str = "fixture.counts";
}
impl harmos::OperationRequest for DeclineOperation {
    type Response = Echoed;
    const ID: &'static str = "probe.decline";
}
impl harmos::OperationRequest for FailOperation {
    type Response = Echoed;
    const ID: &'static str = "probe.fail";
}
impl harmos::OperationRequest for SpinOperation {
    type Response = Echoed;
    const ID: &'static str = "probe.spin";
}
impl harmos::OperationRequest for WaitOperation {
    type Response = Echoed;
    const ID: &'static str = "probe.wait";
}
impl harmos::OperationRequest for FinishOperation {
    type Response = Value;
    const ID: &'static str = "fixture.finish";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
