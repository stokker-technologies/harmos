/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Client Envelopes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;

// Third Party Imports
use bytes::{Buf, BufMut};
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};
use tokio::sync::OwnedSemaphorePermit;
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Frame {
    pub(super) bytes: Vec<u8>,
    pub(super) _permit: OwnedSemaphorePermit,
}

pub(super) struct Wire<R, O> {
    pub(super) envelope: bool,
    pub(super) limit: usize,
    pub(super) marker: PhantomData<fn() -> (R, O)>,
}

pub(super) struct Encode {
    limit: usize,
}
pub(super) struct Decode<R, O> {
    envelope: bool,
    marker: PhantomData<fn() -> (R, O)>,
}

/* === Enums === */

pub(super) enum Output<R, O> {
    Item(O),
    Response(R),
}

/* === Traits === */

/* === Implementations === */

impl<R: prost::Message + Default + Send + 'static, O: prost::Message + Default + Send + 'static> Codec for Wire<R, O> {
    type Encode = Frame;
    type Decode = Output<R, O>;
    type Encoder = Encode;
    type Decoder = Decode<R, O>;
    fn encoder(&mut self) -> Self::Encoder {
        Encode { limit: self.limit }
    }
    fn decoder(&mut self) -> Self::Decoder {
        Decode {
            envelope: self.envelope,
            marker: PhantomData,
        }
    }
}

impl Encoder for Encode {
    type Item = Frame;
    type Error = Status;
    fn encode(&mut self, item: Frame, buffer: &mut EncodeBuf<'_>) -> Result<(), Status> {
        if item.bytes.len() > self.limit {
            return Err(Status::resource_exhausted("client message exceeds encoded byte limit"));
        }
        buffer.put_slice(&item.bytes);
        Ok(())
    }
}

impl<R: prost::Message + Default, O: prost::Message + Default> Decoder for Decode<R, O> {
    type Item = Output<R, O>;
    type Error = Status;
    fn decode(&mut self, buffer: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        if !self.envelope {
            return R::decode(buffer).map(|value| Some(Output::Response(value))).map_err(invalid);
        }
        let mut output = None;
        while buffer.has_remaining() {
            let (tag, wire) = decode_key(buffer).map_err(invalid)?;
            if tag != 1 && tag != 2 {
                skip_field(wire, tag, buffer, DecodeContext::default()).map_err(invalid)?;
                continue;
            }
            if wire != WireType::LengthDelimited || output.is_some() {
                return Err(Status::data_loss("invalid operation response envelope"));
            }
            let length = usize::try_from(decode_varint(buffer).map_err(invalid)?).map_err(invalid)?;
            if length > buffer.remaining() {
                return Err(Status::data_loss("truncated operation response envelope"));
            }
            let payload = buffer.copy_to_bytes(length);
            output = Some(if tag == 1 {
                Output::Item(O::decode(payload).map_err(invalid)?)
            } else {
                Output::Response(R::decode(payload).map_err(invalid)?)
            });
        }
        output.map(Some).ok_or_else(|| Status::data_loss("empty operation response envelope"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(error: impl std::fmt::Display) -> Status {
    Status::data_loss(error.to_string())
}

/* === Tests === */
