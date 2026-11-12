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

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use super::StatusAccess;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct TraceEventsList;


/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for TraceEventsList {
    type Request = harmos_protocol::status::TraceEventsListRequest;
    type Response = harmos_protocol::status::TraceEventsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.status.trace_events_list";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1.status", "TraceEvents", "List"));
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
