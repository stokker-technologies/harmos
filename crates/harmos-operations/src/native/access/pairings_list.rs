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

pub struct PairingsList;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for PairingsList {
    type Request = harmos_protocol::client::access::PairingsListRequest;
    type Response = harmos_protocol::client::access::PairingsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.access.Pairings/List";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1.access", "Pairings", "List"));
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
        access.pairings_list(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
