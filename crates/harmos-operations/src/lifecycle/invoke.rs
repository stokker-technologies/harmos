/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Client Invocation Through the Registry

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::execution::operations::instances::InstanceAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Invoke;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Invoke {
    type Request = harmos_protocol::v1::InvocationsInvokeRequest;
    type Response = harmos_protocol::v1::InvocationsInvokeResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.Invocations/Invoke";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Invocations", "Invoke"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<InstanceAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<InstanceAccess>().map_err(|_| ServingError::Catalog)?;
        Ok(Self::Response { payload: access.0.invoke(request).await? })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
