/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Plugin Authoring And Generated-Contract Clients Over The Harmos Protocol.
//!
//! ```no_run
//! use harmos::{Message, Result, Visibility};
//! use harmos::plugin::{Context, Observer, Operation, Plugin, Stream};
//! use prost_types::Duration;
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
//!     let observer = Observer::new(&temperature, read_temperature).every(Duration { seconds: 1, nanos: 0 });
//!     Plugin::new("thermal", "1.0.0").operation(set).stream(temperature).observer(observer).serve()
//! }
//! ```
//!
//! `Message` derives protobuf scalar, bytes, string, optional, repeated and nested
//! named-struct fields. Fields receive consecutive tags; `#[harmos(tag = 3)]`
//! pins a field's tag; `#[harmos(name = "Temperature")]` supplies an existing
//! protobuf message name. Enums, maps, tuple structs and generics require an explicit `Message` implementation
//! over their generated protobuf type. Required requests implement `OperationRequest`.
//!
//! `Context` resources live for one invocation or publication call. Observers answer
//! host ticks and may call declared host operations, clock, settings and Watches until answering the tick. Device streams use `Stream::publish_on`
//! and `Publisher::publish`, with backpressure and no replay after demand closes.
//! Declare clock requirements with `HostOperation::TimeSleep` and `HostOperation::TimeGet`; every wait and
//! evidence-time read then travels on the invoking call. Independently spawned
//! background work is invisible to virtual time and must be driven in real time.
//!
//! `serve` defaults to managed lifecycle. `Plugin::remote` keeps the endpoint alive
//! after Stop. Both honor `HARMOS_ADDRESS`; cleanup hooks run before Stop replies.

//!
//! Enable `wasm` for WebAssembly components. Authors return `wasm::Registration`
//! from one exported `harmos_register` function using the same `Plugin` and
//! `Operation` builders. The SDK owns the component ABI; the host owns fuel,
//! memory, lifecycle authority and child admission. Plugin operations are unary.

/* === Modules === */

#[cfg(not(target_arch = "wasm32"))]
pub mod client;
mod clock;
#[cfg(not(target_arch = "wasm32"))]
mod connection;
mod context;
mod error;
mod message;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod operation;
pub mod plugin;
#[cfg(not(target_arch = "wasm32"))]
mod server;
mod wire;

mod stream;
#[cfg(feature = "wasm")]
pub mod wasm;

/* === Re-Exports === */

extern crate self as harmos;

#[cfg(not(target_arch = "wasm32"))]
pub use connection::{Client, ClientSettings};
use context::Context;
pub use error::{Error, Result};
include!("exports.rs");
pub use harmos_sdk_derive::Message;
pub use message::{Message, OperationRequest};
use operation::Operation;
use plugin::Plugin;

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
