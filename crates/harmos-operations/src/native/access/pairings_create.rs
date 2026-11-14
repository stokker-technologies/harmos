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

use harmos_protocol::host::access::PairingsCreateRequest;
use harmos_protocol::host::access::PairingsCreateResponse;

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::AccessAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct PairingsCreate;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for PairingsCreate {
    type Request = PairingsCreateRequest;
    type Response = PairingsCreateResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.access.Pairings/Create";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1.access", "Pairings", "Create"));
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
        access.pairings_create(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
