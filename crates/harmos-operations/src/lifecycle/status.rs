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
use harmos_domain::operations::instance::Id;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Status;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Status {
    type Request = harmos_protocol::host::Status;
    type Response = super::InstanceState;
    type Endpoints = ();
    const ID: &'static str = "host.operations.status";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Operations", "Status"));
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
        Ok(super::state(instance.status()))
    }
}

impl Status {
    pub fn of(instance: Id) -> harmos_protocol::host::Status {
        harmos_protocol::host::Status { instance: instance.value() }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
