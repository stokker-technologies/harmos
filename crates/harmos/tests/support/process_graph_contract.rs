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
use harmos::operations::{
    OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireOperation,
};

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

pub struct Read;

pub struct Probe;

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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
