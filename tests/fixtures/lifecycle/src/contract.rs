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

/* === Constants === */
pub const PAYLOAD_BYTES: usize = 256 * 1024;
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(5),
    attempt_timeout: Duration::from_secs(5),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct Payload {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

/* === Enums === */
#[harmos::operation(request = Payload, response = Payload, id = "schematic-sidecar.exchange",
        policy = POLICY, visibility = Visibility::Shared, version = 1)]
pub struct Exchange;

#[harmos::catalog]
pub enum Operations {
    Exchange(Exchange),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
