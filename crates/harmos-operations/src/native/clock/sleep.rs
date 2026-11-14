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
    type Request = harmos_protocol::client::TimeSleepRequest;
    type Response = harmos_protocol::client::TimeSleepResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.Time/Sleep";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1", "Time", "Sleep"));
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
        Ok(harmos_protocol::client::TimeSleepResponse {})
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
