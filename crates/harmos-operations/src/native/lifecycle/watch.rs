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
use crate::Error;
use crate::execution::operations::instances::InstanceAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_operations::operations::instance::Id;
use harmos_protocol::host::InvocationsWatchRequest;
use harmos_protocol::host::InvocationsWatchResponse;

use crate::host::declarations::Events;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = InvocationsWatchRequest;
    type Response = InvocationsWatchResponse;
    type Endpoints = ProtobufOutput<InvocationsWatchResponse>;
    const ID: &'static str = "harmos.host.v1.Invocations/Watch";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::WATCH_POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<InstanceAccess>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        Self::run_with(context, request, output).await
    }
}

impl Watch {
    pub fn of(instance: Id) -> InvocationsWatchRequest {
        InvocationsWatchRequest { instance: instance.value() }
    }
}

impl Watch {
    pub(crate) async fn run_with(
        context: ExecutionContext, request: InvocationsWatchRequest, output: &impl Events<InvocationsWatchResponse>,
    ) -> Result<InvocationsWatchResponse, ServingError> {
        let access = context.resource::<InstanceAccess>().map_err(|_| ServingError::Catalog)?;
        let instance = access
            .0
            .owned(Id::new(request.instance))
            .ok_or_else(|| ServingError::Native(std::sync::Arc::new(Error::UnknownInstance(request.instance))))?;
        let mut changes = instance.subscribe();
        drop(instance);
        loop {
            let status = changes.current();
            let state = InvocationsWatchResponse { invocation: Some(super::state(status)) };
            output.send(state.clone()).await.map_err(ServingError::Stream)?;
            if status.is_settled() || !changes.changed().await {
                return Ok(state);
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
