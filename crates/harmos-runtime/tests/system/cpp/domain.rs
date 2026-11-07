/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Nominal Rust Contracts For The C++ Bench Authoring Example.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::catalog::{Catalog, Member, Visitor};
use harmos_domain::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(5),
    attempt_timeout: std::time::Duration::from_secs(5),
    cancellation_timeout: std::time::Duration::from_secs(2),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_domain::operation(id = "bench.read_temperature", response = Temperature, policy = POLICY)]
pub struct ReadTemperature {
    pub channel: i32,
}

#[harmos_domain::message]
pub struct Temperature {
    pub temperature_c: f64,
}

#[harmos_domain::operation(id = "bench.capture", response = Captured, policy = POLICY, operations = [ReadTemperature])]
pub struct Capture {
    pub channel: i32,
    pub sample_count: u32,
}

#[harmos_domain::message]
pub struct Captured {
    pub published: u32,
}

pub struct Operations;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<V: Visitor + Member<ReadTemperature> + Member<Capture>> Catalog<V> for Operations {
    type Entries = [V::Entry; 2];
    const ENTRIES: Self::Entries = [<V as Member<ReadTemperature>>::ENTRY, <V as Member<Capture>>::ENTRY];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
