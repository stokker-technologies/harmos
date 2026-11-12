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

pub struct Cancel;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Cancel {
    type Request = harmos_protocol::v1::InvocationsCancelRequest;
    type Response = harmos_protocol::v1::InvocationsCancelResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.Invocations/Cancel";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Invocations", "Cancel"));
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
        if instance.status().phase != harmos_operations::operations::instance::Phase::Running {
            return Err(ServingError::Native(std::sync::Arc::new(Error::InvalidInput {
                field: "Cancel",
                reason: "only running invocations can be cancelled",
            })));
        }
        instance.request_cancel();
        Ok(harmos_protocol::v1::InvocationsCancelResponse {
            invocation: Some(super::state(instance.status())),
        })
    }
}

impl Cancel {
    pub fn of(instance: Id) -> harmos_protocol::v1::InvocationsCancelRequest {
        harmos_protocol::v1::InvocationsCancelRequest {
            instance: instance.value(),
            reason: String::new(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
