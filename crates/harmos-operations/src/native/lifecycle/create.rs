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

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::execution::operations::instances::InstanceAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_protocol::host::{InvocationsCreateRequest, InvocationsCreateResponse};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Create;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Create {
    type Request = InvocationsCreateRequest;
    type Response = InvocationsCreateResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.Invocations/Create";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<InstanceAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<InstanceAccess>().map_err(|_| ServingError::Catalog)?;
        Ok(Self::Response { payload: access.0.invoke(request).await? })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
