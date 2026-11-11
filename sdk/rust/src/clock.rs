/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Operation Contracts

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::v1::{Now, Sleep, SleepResponse, Time};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use harmos_protocol::clock::{time_types, types as clock_types};
use prost_types::FileDescriptorSet;

// Local Imports
use crate::{Message, OperationRequest};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for Sleep {
    fn name() -> String {
        "harmos.v1.Sleep".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Sleep", Some(("duration", ".google.protobuf.Duration")), prost_types::Duration::types())
    }
}

impl Message for SleepResponse {
    fn name() -> String {
        "harmos.v1.SleepResponse".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("SleepResponse", None, FileDescriptorSet::default())
    }
}

impl Message for Now {
    fn name() -> String {
        "harmos.v1.Now".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Now", None, FileDescriptorSet::default())
    }
}

impl Message for Time {
    fn name() -> String {
        "harmos.v1.Time".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Time", Some(("now", ".google.protobuf.Timestamp")), prost_types::Timestamp::types())
    }
}

impl OperationRequest for Sleep {
    type Response = SleepResponse;
    const ID: &'static str = "harmos.clock.sleep";
}
impl OperationRequest for Now {
    type Response = Time;
    const ID: &'static str = "harmos.clock.now";
}

impl Message for prost_types::Duration {
    fn name() -> String {
        "google.protobuf.Duration".into()
    }
    fn types() -> FileDescriptorSet {
        time_types("Duration", "google/protobuf/duration.proto")
    }
}

impl Message for prost_types::Timestamp {
    fn name() -> String {
        "google.protobuf.Timestamp".into()
    }
    fn types() -> FileDescriptorSet {
        time_types("Timestamp", "google/protobuf/timestamp.proto")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
