/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

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
use super::envelope::delimited;
use super::{DecodeSettings, Decoder, Event};
use crate::{MemorySize, MessageError, operations::StreamItem};
use harmos_domain::Portable;
use harmos_sdk::grpc::DecodeInput;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Codec<W, D, E> {
    decoder: Decoder<W, D>,
    encode: EncodeSettings,
    response: PhantomData<fn() -> E>,
}

pub struct Encoder<W, E> {
    settings: EncodeSettings,
    scratch: Vec<u8>,
    response: PhantomData<fn() -> (W, E)>,
}

pub struct Transcoded;

/* === Enums === */

/* === Traits === */

pub trait Wire<T>: Send + Sync + 'static {
    fn encode(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError>;

    fn decode_admitted<C>(input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, Status>) -> Result<(T, C), Status>;
}

pub trait Encode<W>: Send + 'static {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError>;
}

/* === Implementations === */

impl EncodeSettings {
    pub(super) fn validate(&self) -> Result<(), Status> {
        if self.message_bytes.get() > u32::MAX as usize || self.buffer_bytes.get() > self.message_bytes.get() {
            return Err(Status::invalid_argument("gRPC encoding limits exceed supported capacity"));
        }
        Ok(())
    }
}

impl<T: Portable> Wire<T> for Transcoded {
    fn encode(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        harmos_sdk::grpc::encode(value, bytes)
    }

    fn decode_admitted<C>(input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, Status>) -> Result<(T, C), Status> {
        let prepared = harmos_sdk::grpc::prepare::<T>(input).map_err(|error| Status::invalid_argument(error.to_string()))?;
        let claim = admit(prepared.memory())?;
        let value = prepared.decode().map_err(|error| Status::invalid_argument(error.to_string()))?;
        Ok((value, claim))
    }
}

impl<W: Wire<T>, T: Send + Sync + 'static> Encode<W> for crate::operations::Response<T> {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        W::encode(self, bytes)
    }
}

impl<W, I, R> Encode<W> for Event<StreamItem<I>, crate::operations::Response<R>>
where
    W: Wire<I> + Wire<R>,
    I: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        match self {
            Self::Item(value) => delimited(1, bytes, |bytes| <W as Wire<I>>::encode(value, bytes)),
            Self::Completion(value) => delimited(2, bytes, |bytes| <W as Wire<R>>::encode(value, bytes)),
        }
    }
}

impl<W: Wire<D>, D: MemorySize + Send + 'static, E: Encode<W>> Codec<W, D, E> {
    pub fn server(self) -> tonic::server::Grpc<Self> {
        let decode = self.decoder.message_limit();
        let encode = self.encode.message_bytes.get();
        tonic::server::Grpc::new(self).max_decoding_message_size(decode).max_encoding_message_size(encode)
    }

    pub fn new(decode: DecodeSettings, encode: EncodeSettings) -> Result<Self, Status> {
        encode.validate()?;
        Ok(Self { decoder: Decoder::new(decode)?, encode, response: PhantomData })
    }
}
impl<W: Wire<D>, D: MemorySize + Send + 'static, E: Encode<W>> tonic::codec::Codec for Codec<W, D, E> {
    type Encode = E;
    type Decode = StreamItem<D>;
    type Encoder = Encoder<W, E>;
    type Decoder = Decoder<W, D>;
    fn encoder(&mut self) -> Self::Encoder {
        Encoder { settings: self.encode, scratch: Vec::new(), response: PhantomData }
    }

    fn decoder(&mut self) -> Self::Decoder {
        self.decoder.fork()
    }
}
impl<W, E: Encode<W>> Encoder<W, E> {
    fn message(&mut self, value: &E) -> Result<&[u8], Status> {
        self.scratch.clear();
        value.write(&mut self.scratch).map_err(|error| Status::internal(error.to_string()))?;
        if self.scratch.len() > self.settings.message_bytes.get() {
            return Err(Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"));
        }
        Ok(&self.scratch)
    }
}
impl<W: Send + 'static, E: Encode<W>> tonic::codec::Encoder for Encoder<W, E> {
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
