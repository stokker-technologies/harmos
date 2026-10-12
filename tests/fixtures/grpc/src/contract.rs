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
use harmos::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
pub struct Payload {
    pub value: String,
}

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    #[operation(request = Payload, response = Payload, id = "unary", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Unary"))]
    Unary,
    #[operation(request = Payload, response = Payload, input = Payload, id = "upload", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Upload"))]
    Upload,
    #[operation(request = Payload, response = Payload, output = Payload, id = "download", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Download"))]
    Download,
    #[operation(request = Payload, response = Payload, input = Payload, output = Payload, id = "exchange", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Exchange"))]
    Exchange,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
