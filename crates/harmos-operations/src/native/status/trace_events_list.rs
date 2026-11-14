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
use super::StatusAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct TraceEventsList;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for TraceEventsList {
    type Request = harmos_protocol::client::status::TraceEventsListRequest;
    type Response = harmos_protocol::client::status::TraceEventsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.status.TraceEvents/List";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1.status", "TraceEvents", "List"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StatusAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<StatusAccess>().map_err(|_| ServingError::Catalog)?;
        let access = access.0.get().ok_or(ServingError::Uninitialized)?;
        access.trace_events_list(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
