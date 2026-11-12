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

// Third Party Imports

// Local Imports
use super::AccessAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct PairingsDelete;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for PairingsDelete {
    type Request = harmos_protocol::access::PairingsDeleteRequest;
    type Response = harmos_protocol::access::PairingsDeleteResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.access.pairings.delete";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1.access", "Pairings", "Delete"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<AccessAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<AccessAccess>().map_err(|_| ServingError::Catalog)?;
        let access = access
            .0
            .read()
            .expect("access port lock is healthy")
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or(ServingError::Uninitialized)?;
        access.pairings_delete(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
