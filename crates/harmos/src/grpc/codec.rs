/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod sealed {
    pub trait Encoding {}
}

/* === Re-Exports === */
pub use crate::config::EncodeSettings;

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;

// Third Party Imports
use bytes::BufMut;
use tonic::{
    Status,
    codec::{BufferSettings, EncodeBuf},
};

// Local Imports
use super::envelope::{encode, size};
use super::{DecodeSettings, Decoder, Event};
use crate::{MemorySize, Protobuf, operations::StreamItem};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Codec<D, E> {
    decoder: Decoder<D>,
    encode: EncodeSettings,
    response: PhantomData<fn() -> E>,
}

pub struct Encoder<E> {
    settings: EncodeSettings,
    scratch: Vec<u8>,
    response: PhantomData<fn() -> E>,
}

/* === Enums === */

/* === Traits === */
pub trait Encode: sealed::Encoding + Send + 'static {
    fn wire_len(&self) -> usize;

    fn write(&self, bytes: &mut Vec<u8>);
}

/* === Implementations === */
impl EncodeSettings {
    pub(super) fn validate(&self) -> Result<(), Status> {
        if self.message_bytes.get() > u32::MAX as usize
            || self.buffer_bytes.get() > self.message_bytes.get()
        {
            return Err(Status::invalid_argument(
                "gRPC encoding limits exceed supported capacity",
            ));
        }
        Ok(())
    }
}

impl<T: Protobuf + 'static> sealed::Encoding for crate::operations::Response<T> {}
impl<T: Protobuf + 'static> Encode for crate::operations::Response<T> {
    fn wire_len(&self) -> usize {
        self.encoded_len()
    }

    fn write(&self, bytes: &mut Vec<u8>) {
        self.encode_into(bytes);
    }
}
impl<I: Protobuf + 'static, R: Protobuf + 'static> sealed::Encoding
    for Event<StreamItem<I>, crate::operations::Response<R>>
{
}
impl<I: Protobuf + 'static, R: Protobuf + 'static> Encode
    for Event<StreamItem<I>, crate::operations::Response<R>>
{
    fn wire_len(&self) -> usize {
        let payload = match self {
            Self::Item(value) => value.encoded_len(),
            Self::Completion(value) => value.encoded_len(),
        };
        size(payload)
    }

    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Item(value) => encode(1, &**value, bytes),
            Self::Completion(value) => encode(2, &**value, bytes),
        }
    }
}

impl<D: Protobuf + MemorySize + 'static, E: Encode> Codec<D, E> {
    pub fn server(self) -> tonic::server::Grpc<Self> {
        let decode = self.decoder.message_limit();
        let encode = self.encode.message_bytes.get();
        tonic::server::Grpc::new(self)
            .max_decoding_message_size(decode)
            .max_encoding_message_size(encode)
    }

    pub fn new(decode: DecodeSettings, encode: EncodeSettings) -> Result<Self, Status> {
        encode.validate()?;
        Ok(Self {
            decoder: Decoder::new(decode)?,
            encode,
            response: PhantomData,
        })
    }
}
impl<D: Protobuf + MemorySize + 'static, E: Encode> tonic::codec::Codec for Codec<D, E> {
    type Encode = E;
    type Decode = StreamItem<D>;
    type Encoder = Encoder<E>;
    type Decoder = Decoder<D>;
    fn encoder(&mut self) -> Self::Encoder {
        Encoder {
            settings: self.encode,
            scratch: Vec::new(),
            response: PhantomData,
        }
    }

    fn decoder(&mut self) -> Self::Decoder {
        self.decoder.fork()
    }
}
impl<E: Encode> Encoder<E> {
    fn message(&mut self, value: &E) -> Result<&[u8], Status> {
        let length = value.wire_len();
        if length > self.settings.message_bytes.get() {
            return Err(Status::resource_exhausted(
                "gRPC encoded message exceeds wire ceiling",
            ));
        }
        self.scratch.clear();
        if self.scratch.capacity() < length {
            self.scratch
                .try_reserve_exact(length)
                .map_err(|_| Status::resource_exhausted("gRPC encoding allocation refused"))?;
        }
        value.write(&mut self.scratch);
        if self.scratch.len() != length {
            return Err(Status::internal(
                "gRPC payload codec violated its encoded length",
            ));
        }
        Ok(&self.scratch)
    }
}
impl<E: Encode> tonic::codec::Encoder for Encoder<E> {
    type Item = E;
    type Error = Status;
    fn encode(&mut self, item: E, destination: &mut EncodeBuf<'_>) -> Result<(), Status> {
        destination.put_slice(self.message(&item)?);
        Ok(())
    }

    fn buffer_settings(&self) -> BufferSettings {
        // Flush Each Protobuf Instead Of Accumulating An Unbounded Ready Batch.
        BufferSettings::new(self.settings.buffer_bytes.get(), 1)
    }
}

/* === Public Functions === */

/* === Private Functions === */
/* === Tests === */
