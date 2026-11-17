/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Time Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::host::{TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use harmos_operations::host::declarations::clock;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Time<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Time<'_> {
    pub async fn get(&self, request: TimeGetRequest) -> Result<TimeGetResponse> {
        self.call.call::<clock::Get>(request).await
    }

    pub async fn sleep(&self, request: TimeSleepRequest) -> Result<TimeSleepResponse> {
        self.call.call::<clock::Sleep>(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
