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
use harmos_protocol::host::TimeSleepRequest;
use harmos_protocol::host::TimeSleepResponse;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Sleep;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Sleep {
    type Request = TimeSleepRequest;
    type Response = TimeSleepResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.Time/Sleep";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ClockAccess>()];
    fn admitted_wait(request: &Self::Request) -> Option<std::time::Duration> {
        duration(request).ok()
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let clock = context.resource::<ClockAccess>().map_err(|_| ServingError::Catalog)?;
        clock.0.sleep(duration(&request)?).await;
        Ok(TimeSleepResponse {})
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn duration(request: &TimeSleepRequest) -> Result<std::time::Duration, ServingError> {
    let duration = request.duration.unwrap_or_default();
    if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) || duration.seconds > 315_576_000_000 {
        return Err(ServingError::Codec(crate::MessageError::borrowed("invalid sleep duration")));
    }
    Ok(std::time::Duration::new(duration.seconds as u64, duration.nanos as u32))
}

/* === Tests === */
