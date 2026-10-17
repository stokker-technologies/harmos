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

// Local Imports
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy, Visibility};
use harmos::sidecar::Refusal;

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Step {
    pub phase: u64,
}

#[harmos::message]
#[harmos::operation(
    id = "process.graph.read",
    version = 1,
    policy = POLICY,
    response = Step,
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
pub struct Read {
    pub phase: u64,
}

#[harmos::message]
#[harmos::operation(
    id = "process.graph.probe",
    version = 1,
    policy = POLICY,
    response = Step,
    visibility = Visibility::Host,
)]
#[derive(Clone, PartialEq)]
pub struct Probe {
    pub phase: u64,
}

/* === Enums === */
#[harmos::catalog]
pub enum Readings {
    Read(Read),
}

#[harmos::catalog]
pub enum Queries {
    Probe(Probe),
}

/* === Traits === */

/* === Implementations === */
impl Marks {
    pub fn mark(&self, name: &str) -> Result<(), Refusal> {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(self.root.join(name)).map_err(Refusal::of)?;
        file.write_all(b"1\n").map_err(Refusal::of)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
