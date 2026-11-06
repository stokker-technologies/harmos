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

/* === Imports === */

// Standard Library Imports
// Third Party Imports
use prost::Message;
use tonic::Status;
use tonic::codec::{BufferSettings, DecodeBuf, EncodeBuf};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Message)]
pub struct Packet {
    #[prost(uint32, tag = "1")]
    pub version: u32,

    #[prost(uint64, tag = "2")]
    pub generation: u64,

    #[prost(uint32, tag = "3")]
    pub phase: u32,

    #[prost(uint64, tag = "4")]
    pub invocation: u64,

    #[prost(string, tag = "5")]
    pub operation: String,

    #[prost(bytes = "vec", tag = "6")]
    pub fingerprint: Vec<u8>,

    #[prost(bytes = "vec", tag = "7")]
    pub context: Vec<u8>,

    #[prost(bytes = "vec", tag = "8")]
    pub payload: Vec<u8>,

    #[prost(uint64, tag = "9")]
    pub count: u64,
}

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
        target.reserve(item.encoded_len());
        item.encode(target).map_err(|error| Status::internal(error.to_string()))
    }
    fn buffer_settings(&self) -> BufferSettings {
        BufferSettings::new(8192, 1)
    }
}

impl tonic::codec::Decoder for Decoder {
    type Item = Packet;
    type Error = Status;
    fn decode(&mut self, source: &mut DecodeBuf<'_>) -> Result<Option<Packet>, Status> {
        Packet::decode(source).map(Some).map_err(|error| Status::invalid_argument(error.to_string()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
