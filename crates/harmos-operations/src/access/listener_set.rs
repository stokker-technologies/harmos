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

pub struct ListenerSet;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for ListenerSet {
    type Request = harmos_protocol::v1::access::ListenerSetRequest;
    type Response = harmos_protocol::v1::access::ListenerSetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.access.Listener/Set";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1.access", "Listener", "Set"));
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
        access.listener_set(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
