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

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use harmos_protocol::clock::declarations_types;
use prost_types::{Duration, FileDescriptorSet, Timestamp};

// Local Imports
use crate::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for Duration {
    fn name() -> String {
        "google.protobuf.Duration".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&declarations_types(), &Self::name()).expect("generated clock message")
    }
}

impl Message for Timestamp {
    fn name() -> String {
        "google.protobuf.Timestamp".into()
    }
    fn types() -> FileDescriptorSet {
        harmos_protocol::schema::message_types(&declarations_types(), &Self::name()).expect("generated clock message")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
