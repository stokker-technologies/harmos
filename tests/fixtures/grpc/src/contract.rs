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
#[harmos::operation(request = Payload, response = Payload, id = "unary", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Unary"))]
pub struct Unary;

#[harmos::operation(request = Payload, response = Payload, input = Payload, id = "upload", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Upload"))]
pub struct Upload;

#[harmos::operation(request = Payload, response = Payload, output = Payload, id = "download", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Download"))]
pub struct Download;

#[harmos::operation(request = Payload, response = Payload, input = Payload, output = Payload, id = "exchange", version = 1, policy = POLICY, grpc(package = "fixture.v1", service = "Operations", method = "Exchange"))]
pub struct Exchange;

#[harmos::catalog]
pub enum Operations {
    Unary(Unary),
    Upload(Upload),
    Download(Download),
    Exchange(Exchange),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
