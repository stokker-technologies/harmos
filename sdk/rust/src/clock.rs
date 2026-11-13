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

pub use harmos_protocol::context::{TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use harmos_protocol::clock::catalog_types;
use prost_types::FileDescriptorSet;

// Local Imports
use crate::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for prost_types::Duration {
    fn name() -> String {
        "google.protobuf.Duration".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

impl Message for prost_types::Timestamp {
    fn name() -> String {
        "google.protobuf.Timestamp".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
