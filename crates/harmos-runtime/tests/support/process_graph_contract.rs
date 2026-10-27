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
use harmos_runtime::operations::{OperationPolicy, RetryPolicy, Visibility};

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

#[harmos_runtime::operation(request = Read, infer_request,
    id = "process_graph_read",

    policy = POLICY,
    response = Step,
    visibility = Visibility::Shared,
)]
pub struct ReadDeclaration;

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub struct Read {
    pub phase: u64,
}

#[harmos_runtime::operation(request = Probe, infer_request,
    id = "process_graph_probe",

    policy = POLICY,
    response = Step,
    visibility = Visibility::Host,
)]
pub struct ProbeDeclaration;

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub struct Probe {
    pub phase: u64,
}

/* === Enums === */

#[harmos_runtime::catalog]
pub enum Readings {
    Read(ReadDeclaration),
}

#[harmos_runtime::catalog]
pub enum Queries {
    Probe(ProbeDeclaration),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
