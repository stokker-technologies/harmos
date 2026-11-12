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
use super::StatusAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Get;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Get {
    type Request = harmos_protocol::v1::StatusGetRequest;
    type Response = harmos_protocol::v1::StatusGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.Status/Get";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Status", "Get"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StatusAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<StatusAccess>().map_err(|_| ServingError::Catalog)?;
        let access = access.0.get().ok_or(ServingError::Uninitialized)?;
        access.get(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
