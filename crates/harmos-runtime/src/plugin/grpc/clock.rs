/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Operations and Their Protocol Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports
use prost::Message;

// Local Imports
use super::{invoke::CallError, v1};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn contracts() -> Vec<v1::OperationContract> {
    [("sleep", "Sleep", "SleepResponse"), ("now", "Now", "Time")]
        .into_iter()
        .map(|(id, request, response)| {
            let descriptors = harmos_protocol::clock::catalog_types();
            let id = format!("harmos.clock.{id}");
            let request = format!("harmos.v1.{request}");
            let response = format!("harmos.v1.{response}");
            let fingerprint = harmos_protocol::schema::fingerprint(&id, &descriptors, &request, &response).unwrap();
            v1::OperationContract {
                id,
                request,
                response,
                fingerprint,
                visibility: v1::Visibility::Public as i32,
                ..Default::default()
            }
        })
        .collect()
}

pub(crate) async fn run(clock: &crate::clock::Clock, operation: &str, payload: &[u8]) -> Result<Vec<u8>, CallError> {
    let invalid = |error: String| CallError::Transport(tonic::Status::invalid_argument(error));
    if operation == "harmos.clock.now" {
        v1::Now::decode(payload).map_err(|error| invalid(error.to_string()))?;
        return Ok(v1::Time { now: Some(clock.date().into()) }.encode_to_vec());
    }
    let request = v1::Sleep::decode(payload).map_err(|error| invalid(error.to_string()))?;
    let duration = request.duration.unwrap_or_default();
    if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) || duration.seconds > 315_576_000_000 {
        return Err(invalid("Invalid Sleep Duration".into()));
    }
    crate::clock::waiting(clock.sleep(Duration::new(duration.seconds as u64, duration.nanos as u32))).await;
    Ok(v1::SleepResponse {}.encode_to_vec())
}

/* === Private Functions === */

/* === Tests === */
