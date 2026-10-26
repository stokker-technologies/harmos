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
use crate::operations::{Contract, OperationPolicy, RetryPolicy, Visibility};
use crate::plugin::Grant;

/* === Constants === */

pub const POLICY: OperationPolicy =
    OperationPolicy { timeout: Duration::from_secs(30), attempt_timeout: Duration::from_secs(30), retries: RetryPolicy::Never };

/* === Macros === */

pub struct Catalog;

impl crate::operations::OperationDeclaration for Catalog {
    type Request = CatalogRequest;
    type Response = CatalogPage;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "host_catalog";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const DESCRIPTION: &'static str = "Page live operation contracts or the schemas, streams and objects storage holds durably";
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.host.v1", "Host", "Catalog"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        (CatalogRequest::required_grant)(request)
    }
}

impl crate::operations::RequestFor for CatalogRequest {
    type Operation = Catalog;
}

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
