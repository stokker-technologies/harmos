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
    type Request = harmos_protocol::host::Cancel;
    type Response = super::InstanceState;
    type Endpoints = ();
    const ID: &'static str = "host.operations.cancel";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Operations", "Cancel"));
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
        instance.request_cancel();
        Ok(super::state(instance.status()))
    }
}

impl Cancel {
    pub fn of(instance: Id) -> harmos_protocol::host::Cancel {
        harmos_protocol::host::Cancel { instance: instance.value() }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
