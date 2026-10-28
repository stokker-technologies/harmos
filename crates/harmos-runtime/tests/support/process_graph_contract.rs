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
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

pub const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub struct Step {
    pub phase: u64,
}

#[harmos_runtime::operation(id = "process.graph.read",
    policy = POLICY,
    response = Step,
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
pub struct ReadDeclaration {
    pub phase: u64,
}

#[harmos_runtime::operation(id = "process.graph.probe",
    policy = POLICY,
    response = Step,
    visibility = Host,
)]
#[derive(Clone, PartialEq)]
pub struct ProbeDeclaration {
    pub phase: u64,
}

/* === Enums === */

#[harmos_domain::catalog]
pub enum Readings {
    ReadDeclaration(ReadDeclaration),
}

#[harmos_domain::catalog]
pub enum Queries {
    ProbeDeclaration(ProbeDeclaration),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
