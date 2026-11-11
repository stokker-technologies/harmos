/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Additional Protobuf Graph Fixture Shared by the Reference and SDK Plugin.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Third Party Imports
use prost::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Message)]
pub struct Graph {
    #[prost(message, repeated, tag = "1")]
    pub items: Vec<Nested>,

    #[prost(message, optional, tag = "2")]
    pub tail: Option<Zebra>,

    #[prost(message, optional, boxed, tag = "3")]
    pub next: Option<Box<Graph>>,

    #[prost(message, optional, tag = "4")]
    pub interval: Option<prost_types::Duration>,

    #[prost(int32, tag = "5")]
    pub status: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct Nested {
    #[prost(int32, optional, tag = "1")]
    pub count: Option<i32>,

    #[prost(message, optional, boxed, tag = "2")]
    pub next: Option<Box<Graph>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Zebra {
    #[prost(message, optional, tag = "1")]
    pub value: Option<Value>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Value {
    #[prost(bytes = "vec", tag = "1")]
    pub value: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> prost_types::FileDescriptorSet {
    prost_types::FileDescriptorSet::decode(include_bytes!("../fixtures/schema.bin").as_slice()).expect("schema fixture decodes")
}

/* === Private Functions === */

/* === Tests === */
