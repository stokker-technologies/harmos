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

use crate::Error;
use crate::execution::operations::instances::InstanceAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Get;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Get {
    type Request = harmos_protocol::client::InvocationsGetRequest;
    type Response = harmos_protocol::client::InvocationsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.Invocations/Get";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1", "Invocations", "Get"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<InstanceAccess>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<InstanceAccess>().map_err(|_| ServingError::Catalog)?;
        let instance = access
            .0
            .owned(Id::new(request.instance))
            .ok_or_else(|| ServingError::Native(std::sync::Arc::new(Error::UnknownInstance(request.instance))))?;
        let _ = output;
        Ok(harmos_protocol::client::InvocationsGetResponse {
            invocation: Some(super::state(instance.status())),
        })
    }
}

impl Get {
    pub fn of(instance: Id) -> harmos_protocol::client::InvocationsGetRequest {
        harmos_protocol::client::InvocationsGetRequest { instance: instance.value() }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
