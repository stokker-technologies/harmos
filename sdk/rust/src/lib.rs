/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Plugin authoring over the generated `harmos.v1` protocol.
//!
//! ```no_run
//! use harmos_sdk_grpc::{Context, Message, Observer, Operation, Plugin, Result, Stream, Visibility};
//!
//! #[derive(Message)]
//! struct SetTemperature { target: f64 }
//!
//! #[derive(Message)]
//! struct Temperature { celsius: f64 }
//!
//! async fn set_temperature(ctx: &Context, request: SetTemperature) -> Result<()> {
//!     ctx.set(request.target);
//!     Ok(())
//! }
//!
//! async fn read_temperature(_: &Context) -> Result<Temperature> {
//!     Ok(Temperature { celsius: 20.0 })
//! }
//!
//! fn main() -> Result<()> {
//!     let set = Operation::new("thermal.set", set_temperature).visibility(Visibility::Public);
//!     let temperature = Stream::<Temperature>::new("thermal.temperature");
//!     let observer = Observer::new(&temperature, read_temperature).every_ms(1000);
//!     Plugin::new("thermal", "1.0.0").operation(set).stream(temperature).observer(observer).serve()
//! }
//! ```
//!
//! `Message` derives protobuf scalar, bytes, string, optional, repeated and nested
//! named-struct fields. Fields receive consecutive tags; `#[harmos(tag = 3)]`
//! pins a field's tag. `#[harmos(package = "thermal.v1", file = "thermal/v1/data.proto")]`
//! controls descriptor identity; `name` supplies an existing protobuf message name.
//! Enums, maps, tuple structs and generics require an explicit `Message` implementation
//! over their generated protobuf type. Required requests implement `OperationRequest`.
//!
//! `Context` resources live for one invocation or publication call. Observers answer
//! host ticks and cannot make child calls. Device streams use `Stream::publish_on`
//! and `Publisher::publish`, with backpressure and no replay after demand closes.
//! Declare clock requirements with `clock::Sleep` and `clock::Now`; every wait and
//! evidence-time read then travels on the invoking call. Independently spawned
//! background work is invisible to virtual time and must be driven in real time.
//!
//! `serve` defaults to managed lifecycle. `Plugin::remote` keeps the endpoint alive
//! after Stop. Both honor `HARMOS_ADDRESS`; cleanup hooks run before Stop replies.

/* === Modules === */

pub mod clock;
mod context;
mod error;
mod message;
mod operation;
mod plugin;
pub mod prelude;
mod server;
mod stream;

/* === Re-Exports === */

extern crate self as harmos_sdk_grpc;

pub use context::Context;
pub use error::{Error, Result};
pub use harmos_protocol::v1::{Classification, Failure, OperationPolicy, Section, Visibility};
pub use harmos_sdk_derive::Message;
pub use message::{Message, OperationRequest};
pub use operation::Operation;
pub use plugin::Plugin;
pub use stream::{Observer, Publisher, Stream};

// Dependencies Used by the Data Derive and Generated Protobuf Consumers.
#[doc(hidden)]
pub mod encoding {
    pub use crate::message::merge_descriptor_types;
    pub use ::bytes::{Buf, BufMut};
    pub use prost::encoding::{
        DecodeContext, WireType, r#bool, bytes, double, float, int32, int64, message, skip_field, string, uint32, uint64,
    };
    pub use prost::{DecodeError, Message};
    pub use prost_types::{
        DescriptorProto, Duration, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, OneofDescriptorProto, Timestamp,
    };
}

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
