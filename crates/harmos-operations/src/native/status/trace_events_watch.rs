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
use futures_util::StreamExt;

// Local Imports
use super::StatusAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_protocol::host::status::TraceEventsWatchRequest;
use harmos_protocol::host::status::TraceEventsWatchResponse;

use crate::host::declarations::Events;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct TraceEventsWatch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for TraceEventsWatch {
    type Request = TraceEventsWatchRequest;
    type Response = TraceEventsWatchResponse;
    type Endpoints = ProtobufOutput<TraceEventsWatchResponse>;
    const ID: &'static str = "harmos.host.v1.status.TraceEvents/Watch";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StatusAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        Self::run_with(context, request, output).await
    }
}

impl TraceEventsWatch {
    pub(crate) async fn run_with(
        context: ExecutionContext, request: TraceEventsWatchRequest, output: &impl Events<TraceEventsWatchResponse>,
    ) -> Result<TraceEventsWatchResponse, ServingError> {
        let access = context.resource::<StatusAccess>().map_err(|_| ServingError::Catalog)?;
        let access = access.0.get().ok_or(ServingError::Uninitialized)?;
        let mut events = access.trace_events_watch(request).await?;
        let mut last = TraceEventsWatchResponse::default();
        while let Some(event) = events.next().await {
            last = event?;
            output.send(last.clone()).await.map_err(ServingError::Stream)?;
        }
        Ok(last)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
