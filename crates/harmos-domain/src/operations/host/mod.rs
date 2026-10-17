/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod payloads;

/* === Re-Exports === */
pub use payloads::*;

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::operations::{Contract, OperationPolicy, RestartPolicy, RetryPolicy, Visibility};
use crate::plugin::Grant;

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(30),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */
crate::declare!(Operations {
    id: "host.operations",
    version: 1,
    policy: POLICY,
    response: OperationsResponse,
    description: "Discover bounded live contracts; invocation authorization remains conditional",
    visibility: Visibility::Shared,
    grpc: ("harmos.host.v1", "Host", "Operations"),
});

/* === Type Aliases === */

/* === Structs === */
// A Live Provider Contract. Invocation Still Performs Request-Specific Authorization.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredOperation {
    pub owner: crate::Identifier,
    pub generation: u64,
    pub contract: Contract,
    pub grants: Vec<Grant>,
    pub conditional_access: bool,
    pub grpc_available: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
