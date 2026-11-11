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

pub struct Sleep;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Sleep {
    type Request = harmos_protocol::v1::Sleep;
    type Response = harmos_protocol::v1::SleepResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.clock.sleep";
    const GRPC: Grpc = Grpc::new("harmos.host.v1", "Clock", "Sleep");
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ClockAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let clock = context.resource::<ClockAccess>().map_err(|_| ServingError::Catalog)?;
        let duration = request.duration.unwrap_or_default();
        if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) || duration.seconds > 315_576_000_000 {
            return Err(ServingError::Codec(crate::MessageError::borrowed("invalid sleep duration")));
        }
        clock.0.sleep(std::time::Duration::new(duration.seconds as u64, duration.nanos as u32)).await;
        Ok(harmos_protocol::v1::SleepResponse {})
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
