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
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
pub struct Payload {
    pub value: String,
}

/* === Enums === */
#[harmos::message]
#[harmos::operation(
    id = "unary",
    version = 1,
    policy = POLICY,
    response = Payload,
    grpc(package = "fixture.v1", service = "Operations", method = "Unary"),
)]
pub struct Unary {
    pub value: String,
}

#[harmos::message]
#[harmos::operation(
    id = "upload",
    version = 1,
    policy = POLICY,
    response = Payload,
    input = Payload,
    grpc(package = "fixture.v1", service = "Operations", method = "Upload"),
)]
pub struct Upload {
    pub value: String,
}

#[harmos::message]
#[harmos::operation(
    id = "download",
    version = 1,
    policy = POLICY,
    response = Payload,
    output = Payload,
    grpc(package = "fixture.v1", service = "Operations", method = "Download"),
)]
pub struct Download {
    pub value: String,
}

#[harmos::message]
#[harmos::operation(
    id = "exchange",
    version = 1,
    policy = POLICY,
    response = Payload,
    input = Payload,
    output = Payload,
    grpc(package = "fixture.v1", service = "Operations", method = "Exchange"),
)]
pub struct Exchange {
    pub value: String,
}

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
