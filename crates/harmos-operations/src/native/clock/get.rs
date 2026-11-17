/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Host Clock Operation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::ClockAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_protocol::host::TimeGetRequest;
use harmos_protocol::host::TimeGetResponse;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Get;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Get {
    type Request = TimeGetRequest;
    type Response = TimeGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.Time/Get";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ClockAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let clock = context.resource::<ClockAccess>().map_err(|_| ServingError::Catalog)?;
        let _ = request;
        Ok(TimeGetResponse { now: Some(clock.0.now()) })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
