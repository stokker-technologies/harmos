/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod relative {
    #[facade::message(harmos_path = ::facade::__harmos)]
    pub struct WireReply {
        pub value: String,
    }
    pub type Reply = WireReply;
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports
use facade::__harmos::operations::{OperationPolicy, RetryPolicy};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[facade::operation(
    harmos_path = ::facade::__harmos,
    response = relative::Reply,
    id = "test.echo",
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)]
pub struct Echo {
    pub value: String,
}

/* === Enums === */

#[facade::catalog(harmos_path = ::facade::__harmos)]
pub enum Operations {
    Echo(Echo),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
