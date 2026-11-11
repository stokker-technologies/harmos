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
use harmos_protocol::clock::catalog_types;
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
        catalog_types()
    }
}

impl Message for SleepResponse {
    fn name() -> String {
        "harmos.v1.SleepResponse".into()
    }
    fn types() -> FileDescriptorSet {
        catalog_types()
    }
}

impl Message for Now {
    fn name() -> String {
        "harmos.v1.Now".into()
    }
    fn types() -> FileDescriptorSet {
        catalog_types()
    }
}

impl Message for Time {
    fn name() -> String {
        "harmos.v1.Time".into()
    }
    fn types() -> FileDescriptorSet {
        catalog_types()
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
        catalog_types()
    }
}

impl Message for prost_types::Timestamp {
    fn name() -> String {
        "google.protobuf.Timestamp".into()
    }
    fn types() -> FileDescriptorSet {
        catalog_types()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
