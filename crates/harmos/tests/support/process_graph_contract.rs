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
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::operations::{
    OperationDeclaration, OperationPolicy, RetryPolicy, Visibility, WireOperation,
};
use harmos::plugin::sidecar::Refusal;

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Step {
    pub phase: u64,
}

pub struct Read;

pub struct Probe;

pub struct Marks {
    pub root: PathBuf,
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
impl OperationDeclaration for Read {
    type Request = Step;
    type Response = Step;
    const ID: &'static str = "process.graph.read";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl WireOperation for Read {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Probe {
    type Request = Step;
    type Response = Step;
    const ID: &'static str = "process.graph.probe";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Probe {
    const VERSION: u32 = 1;
}

impl Marks {
    pub fn mark(&self, name: &str) -> Result<(), Refusal> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join(name))
            .map_err(Refusal::of)?;
        file.write_all(b"1\n").map_err(Refusal::of)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
