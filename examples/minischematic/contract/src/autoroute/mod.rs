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
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::operations::{OperationPolicy, ReplaySafety, RetryPolicy, Visibility};

/* === Constants === */
pub const NET_RENAME: &str = "net.rename";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct EchoRequest {
    pub text: String,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct EchoResponse {
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct NetName {
    pub base: String,
    pub member: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RenameNet {
    pub net: u64,
    pub to: NetName,
}

#[harmos::stream(
    description = "Progress published by an autorouter, encoded as a Harmos protobuf message.",
    id = "progress",
    window = 16
)]
#[harmos::message]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct Progress {
    pub percent: u32,
}

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    #[operation(description = "The Guest's Portable Echo Operation.",
        request = EchoRequest,
        response = EchoResponse,
        id = "autoroute.echo",
        policy = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(5),
            retries: RetryPolicy::Never,
        },
        visibility = Visibility::Shared,
        replay = ReplaySafety::Idempotent,
        version = 1,
    )]
    Echo,
}

/* === Traits === */

/* === Implementations === */
impl NetName {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn plain(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            member: None,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
