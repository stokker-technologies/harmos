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
use crate::operations::{
    Contract, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility,
    WireOperation,
};
use crate::plugin::Grant;

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(30),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Operations;
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
impl WireOperation for Operations {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new(
        "harmos.host.v1",
        "Host",
        "Operations",
    ));

    #[cfg(feature = "grpc")]
    fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl OperationDeclaration for Operations {
    type Request = OperationsRequest;
    type Response = OperationsResponse;
    const ID: &'static str = "host.operations";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const DESCRIPTION: &'static str =
        "Discover bounded live contracts; invocation authorization remains conditional";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
