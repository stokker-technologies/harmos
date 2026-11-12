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

pub use harmos_protocol::v1::{TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse};

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

impl Message for TimeSleepRequest {
    fn name() -> String {
        "harmos.v1.TimeSleepRequest".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

impl Message for TimeSleepResponse {
    fn name() -> String {
        "harmos.v1.TimeSleepResponse".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

impl Message for TimeGetRequest {
    fn name() -> String {
        "harmos.v1.TimeGetRequest".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

impl Message for TimeGetResponse {
    fn name() -> String {
        "harmos.v1.TimeGetResponse".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&catalog_types(), &Self::name()).expect("generated clock message")
    }
}

impl OperationRequest for TimeSleepRequest {
    type Response = TimeSleepResponse;
    const ID: &'static str = "harmos.time.sleep";
}
impl OperationRequest for TimeGetRequest {
    type Response = TimeGetResponse;
    const ID: &'static str = "harmos.time.get";
}

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
