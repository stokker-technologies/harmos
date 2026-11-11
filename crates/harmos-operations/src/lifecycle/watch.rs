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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_operations::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = harmos_protocol::host::Watch;
    type Response = super::InstanceState;
    type Endpoints = ProtobufOutput<super::InstanceState>;
    const ID: &'static str = "host.operations.watch";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Operations", "Watch"));
    const POLICY: crate::OperationPolicy = super::WATCH_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<InstanceAccess>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<InstanceAccess>().map_err(|_| ServingError::Catalog)?;
        let instance = access
            .0
            .owned(Id::new(request.instance))
            .ok_or_else(|| ServingError::Native(std::sync::Arc::new(Error::UnknownInstance(request.instance))))?;
        let mut changes = instance.subscribe();
        drop(instance);
        loop {
            let status = changes.current();
            let state = super::state(status);
            output.send(state.clone()).await.map_err(ServingError::Stream)?;
            if status.is_settled() || !changes.changed().await {
                return Ok(state);
            }
        }
    }
}

impl Watch {
    pub fn of(instance: Id) -> harmos_protocol::host::Watch {
        harmos_protocol::host::Watch { instance: instance.value() }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
