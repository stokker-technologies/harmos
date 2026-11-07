/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Experimental Protobuf Envelopes; No Postcard Bytes Cross This Carrier.

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::prototype::Packet;
pub(super) use harmos_protocol::prototype::{Assignment, Initialization};

/* === Imports === */

// Standard Library Imports
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// Third Party Imports
use prost::Message;
use tonic::Status;
use tonic::codec::{BufferSettings, DecodeBuf, EncodeBuf};

// Local Imports

/* === Constants === */

static PROFILING: LazyLock<bool> = LazyLock::new(|| std::env::var_os("HARMOS_PROFILE_TRANSPORT").is_some());
static ENCODE_NS: AtomicU64 = AtomicU64::new(0);
static DECODE_NS: AtomicU64 = AtomicU64::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub(super) struct Codec;

pub(super) struct Encoder;

pub(super) struct Decoder;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl tonic::codec::Codec for Codec {
    type Encode = Packet;
    type Decode = Packet;
    type Encoder = Encoder;
    type Decoder = Decoder;
    fn encoder(&mut self) -> Encoder {
        Encoder
    }
    fn decoder(&mut self) -> Decoder {
        Decoder
    }
}

impl tonic::codec::Encoder for Encoder {
    type Item = Packet;
    type Error = Status;
    fn encode(&mut self, item: Packet, target: &mut EncodeBuf<'_>) -> Result<(), Status> {
        let started = (*PROFILING).then(Instant::now);
        target.reserve(item.encoded_len());
        let result = item.encode(target).map_err(|error| Status::internal(error.to_string()));
        if let Some(started) = started {
            ENCODE_NS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        result
    }
    fn buffer_settings(&self) -> BufferSettings {
        BufferSettings::new(64 * 1024, 1)
    }
}

impl tonic::codec::Decoder for Decoder {
    type Item = Packet;
    type Error = Status;
    fn decode(&mut self, source: &mut DecodeBuf<'_>) -> Result<Option<Packet>, Status> {
        let started = (*PROFILING).then(Instant::now);
        let result = Packet::decode(source).map(Some).map_err(|error| Status::invalid_argument(error.to_string()));
        if let Some(started) = started {
            DECODE_NS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        result
    }
}

/* === Public Functions === */

pub(super) fn profile() {
    if *PROFILING {
        eprintln!(
            "PROFILE_CODEC pid={} encode_ms={:.3} decode_ms={:.3}",
            std::process::id(),
            ENCODE_NS.swap(0, Ordering::Relaxed) as f64 / 1e6,
            DECODE_NS.swap(0, Ordering::Relaxed) as f64 / 1e6
        );
    }
}

/* === Private Functions === */

/* === Tests === */
