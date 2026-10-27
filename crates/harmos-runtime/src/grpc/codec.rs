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
use std::{marker::PhantomData, sync::Arc};

// Third Party Imports
use bytes::BufMut;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tonic::{
    Status,
    codec::{BufferSettings, EncodeBuf},
};

// Local Imports
use super::envelope::{delimited, delimited_admitted};
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
    preparation: Option<Arc<Semaphore>>,
    response: PhantomData<fn() -> E>,
}

pub struct Encoder<W, E> {
    settings: EncodeSettings,
    scratch: Vec<u8>,
    claim: Option<OwnedSemaphorePermit>,
    preparation: Option<Arc<Semaphore>>,
    response: PhantomData<fn() -> (W, E)>,
}

pub struct Transcoded;

/* === Enums === */

/* === Traits === */

pub trait Wire<T>: Send + Sync + 'static {
    fn encode(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError>;

    /// Admits Encoding Workspace Through The Caller's Existing Native Owner.
    ///
    /// The Default Supports Cooperative Handwritten Codecs. Override It To Admit
    /// Their Actual Workspace Before Construction; Internal User Allocations Are Not Governed.
    fn encode_admitted(
        value: &T, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
    ) -> Result<(), Status> {
        cooperative(bytes, limit, admit, |bytes| Self::encode(value, bytes))
    }

    fn decode_admitted<C>(input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, Status>) -> Result<(T, C), Status>;
}

pub trait Encode<W>: Send + 'static {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError>;

    /// Uses The Same Native Admission Owner As The Containing Encoder.
    fn write_admitted(&self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
        cooperative(bytes, limit, admit, |bytes| self.write(bytes))
    }
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

    fn encode_admitted(
        value: &T, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
    ) -> Result<(), Status> {
        let mut refusal = None;
        let result = harmos_sdk::grpc::encode_admitted(value, bytes, limit, &mut |required| {
            admit(required).map_err(|error| {
                refusal = Some(error);
                MessageError::borrowed("gRPC encoding admission refused")
            })
        });
        if let Some(error) = refusal {
            return Err(error);
        }
        result.map_err(|error| match error {
            harmos_sdk::grpc::EncodeError::Message(error) => Status::internal(error.to_string()),
            harmos_sdk::grpc::EncodeError::Limit { .. } => Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"),
        })
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

    fn write_admitted(&self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
        W::encode_admitted(self, bytes, limit, admit)
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

    fn write_admitted(&self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
        match self {
            Self::Item(value) => {
                delimited_admitted(1, bytes, limit, admit, |bytes, admit| <W as Wire<I>>::encode_admitted(value, bytes, limit, admit))
            }
            Self::Completion(value) => {
                delimited_admitted(2, bytes, limit, admit, |bytes, admit| <W as Wire<R>>::encode_admitted(value, bytes, limit, admit))
            }
        }
    }
}

impl<W: Wire<D>, D: MemorySize + Send + 'static, E: Encode<W>> Codec<W, D, E> {
    pub fn server(self) -> tonic::server::Grpc<Self> {
        let decode = self.decoder.message_limit();
        let encode = self.encode.message_bytes.get();
        tonic::server::Grpc::new(self).max_decoding_message_size(decode).max_encoding_message_size(encode)
    }

    pub(super) fn with_additional_memory(mut self, memory: super::decoding::AdditionalMemory) -> Self {
        self.decoder.additional = Some(memory);
        self
    }

    /// Creates A Standalone Codec With Per-Message Limits.
    ///
    /// Runtime Server Routes Also Supply Their Existing Optional Operation Preparation Owner.
    /// An Absent Owner Preserves Unbounded Aggregate Encoding, Rather Than Claiming Admission.
    pub fn new(decode: DecodeSettings, encode: EncodeSettings) -> Result<Self, Status> {
        encode.validate()?;
        Ok(Self {
            decoder: Decoder::new(decode)?,
            encode,
            preparation: None,
            response: PhantomData,
        })
    }

    pub(crate) fn with_preparation(mut self, preparation: Option<Arc<Semaphore>>) -> Self {
        self.preparation = preparation;
        self
    }
}
impl<W: Wire<D>, D: MemorySize + Send + 'static, E: Encode<W>> tonic::codec::Codec for Codec<W, D, E> {
    type Encode = E;
    type Decode = StreamItem<D>;
    type Encoder = Encoder<W, E>;
    type Decoder = Decoder<W, D>;
    fn encoder(&mut self) -> Self::Encoder {
        Encoder {
            settings: self.encode,
            scratch: Vec::new(),
            claim: None,
            preparation: self.preparation.clone(),
            response: PhantomData,
        }
    }

    fn decoder(&mut self) -> Self::Decoder {
        self.decoder.fork()
    }
}
impl<W, E: Encode<W>> Encoder<W, E> {
    #[tracing::instrument(target = "harmos::grpc", name = "grpc.encode", level = "trace", skip_all)]
    fn message(&mut self, value: &E) -> Result<&[u8], Status> {
        self.scratch.clear();
        let result = value.write_admitted(&mut self.scratch, self.settings.message_bytes.get(), &mut |required| {
            retain(&self.preparation, &mut self.claim, required)
        });
        let result = if result.is_ok() && self.scratch.len() > self.settings.message_bytes.get() {
            Err(Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"))
        } else {
            result
        };
        if let Err(error) = result {
            let claimed = self.claim.as_ref().map_or(0, OwnedSemaphorePermit::num_permits);
            if self.preparation.is_some() && self.scratch.capacity() > claimed {
                self.scratch = Vec::new();
            }
            retain(&self.preparation, &mut self.claim, self.scratch.capacity())?;
            return Err(error);
        }
        retain(&self.preparation, &mut self.claim, self.scratch.capacity())?;
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

fn retain(preparation: &Option<Arc<Semaphore>>, claim: &mut Option<OwnedSemaphorePermit>, required: usize) -> Result<(), Status> {
    let Some(preparation) = preparation else {
        return Ok(());
    };
    let retained = claim.as_ref().map_or(0, OwnedSemaphorePermit::num_permits);
    if required == retained {
        return Ok(());
    }
    if required > retained {
        let delta = u32::try_from(required - retained).map_err(|_| Status::resource_exhausted("gRPC encoding size overflows"))?;
        let additional = preparation
            .clone()
            .try_acquire_many_owned(delta)
            .map_err(|_| Status::resource_exhausted("operation preparation bytes are retained"))?;
        match claim {
            Some(claim) => claim.merge(additional),
            None => *claim = Some(additional),
        }
        return Ok(());
    }
    if required == 0 {
        *claim = None;
    } else if let Some(claim) = claim {
        drop(claim.split(retained - required));
    }
    Ok(())
}

fn cooperative(
    bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
    write: impl FnOnce(&mut Vec<u8>) -> Result<(), MessageError>,
) -> Result<(), Status> {
    let peak = bytes.capacity().checked_add(limit).ok_or_else(|| Status::resource_exhausted("gRPC encoding size overflows"))?;
    admit(peak)?;
    write(bytes).map_err(|error| Status::internal(error.to_string()))?;
    if bytes.len() > limit {
        return Err(Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"));
    }
    admit(bytes.capacity())
}

pub(crate) fn encode_bytes(
    payload: &[u8], bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>,
) -> Result<(), Status> {
    let length = bytes.len().checked_add(payload.len()).ok_or_else(|| Status::resource_exhausted("gRPC encoding size overflows"))?;
    if length > limit {
        return Err(Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"));
    }
    reserve(bytes, length, admit)?;
    bytes.extend_from_slice(payload);
    Ok(())
}

pub(super) fn reserve(bytes: &mut Vec<u8>, length: usize, admit: &mut dyn FnMut(usize) -> Result<(), Status>) -> Result<(), Status> {
    let mut peak = bytes.capacity();
    if length > bytes.capacity() {
        peak = peak.checked_add(length).ok_or_else(|| Status::resource_exhausted("gRPC encoding size overflows"))?;
    }
    admit(peak)?;
    bytes
        .try_reserve_exact(length.saturating_sub(bytes.len()))
        .map_err(|_| Status::resource_exhausted("gRPC encoding allocation failed"))?;
    admit(bytes.capacity())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::Response;

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Row {
        bytes: Vec<u8>,
    }

    fn encoder(preparation: Option<Arc<Semaphore>>) -> Encoder<Transcoded, Response<Row>> {
        Encoder {
            settings: EncodeSettings {
                message_bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
                buffer_bytes: std::num::NonZeroUsize::new(1024).unwrap(),
            },
            scratch: Vec::new(),
            claim: None,
            preparation,
            response: PhantomData,
        }
    }

    #[test]
    fn encoding_retains_capacity_and_failed_growth_keeps_existing_claim() {
        let budget = Arc::new(Semaphore::new(48 * 1024));
        let mut encoder = encoder(Some(budget.clone()));
        encoder.message(&Response::new(Row { bytes: vec![37; 8192] })).unwrap();
        let retained = encoder.scratch.capacity();
        assert_eq!(budget.available_permits(), 48 * 1024 - retained);
        assert_eq!(encoder.claim.as_ref().unwrap().num_permits(), retained);

        let free = budget.available_permits();
        let blocker = budget.clone().try_acquire_many_owned(free as u32).unwrap();
        let refusal = encoder.message(&Response::new(Row { bytes: vec![37; 16384] }));
        assert_eq!(refusal.unwrap_err().code(), tonic::Code::ResourceExhausted);
        assert_eq!(encoder.scratch.capacity(), retained);
        assert_eq!(encoder.claim.as_ref().unwrap().num_permits(), retained);
        assert_eq!(budget.available_permits(), 0);

        drop(blocker);
        encoder.message(&Response::new(Row { bytes: vec![37; 16384] })).unwrap();
        let retained = encoder.scratch.capacity();
        assert_eq!(budget.available_permits(), 48 * 1024 - retained);
        encoder.message(&Response::new(Row { bytes: vec![] })).unwrap();
        assert_eq!(encoder.scratch.capacity(), retained);
        assert_eq!(budget.available_permits(), 48 * 1024 - retained);
        drop(encoder);
        assert_eq!(budget.available_permits(), 48 * 1024);
    }

    #[test]
    fn absent_preparation_budget_preserves_native_encoding() {
        let mut encoder = encoder(None);
        encoder.message(&Response::new(Row { bytes: vec![37; 8192] })).unwrap();
        assert!(encoder.claim.is_none());
        assert!(!encoder.scratch.is_empty());
    }

    #[test]
    fn empty_retained_workspace_needs_no_permit() {
        let budget = Arc::new(Semaphore::new(0));
        let mut claim = None;
        retain(&Some(budget.clone()), &mut claim, 0).unwrap();
        assert!(claim.is_none());
        assert_eq!(budget.available_permits(), 0);
    }
}
