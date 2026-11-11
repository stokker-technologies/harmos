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

use super::ClockAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Now;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Now {
    type Request = harmos_protocol::v1::Now;
    type Response = harmos_protocol::v1::Time;
    type Endpoints = ();
    const ID: &'static str = "harmos.clock.now";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Clock", "Now"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ClockAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let clock = context.resource::<ClockAccess>().map_err(|_| ServingError::Catalog)?;
        let _ = request;
        Ok(harmos_protocol::v1::Time { now: Some(clock.0.now()) })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
