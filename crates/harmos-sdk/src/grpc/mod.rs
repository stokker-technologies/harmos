/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Protobuf Transport Adapter Shared By The Host Runtime And Sidecars.
//
// Domain messages stay codec-free: each carries its canonical Postcard value
// and its described shape. This module projects every such message onto the
// protobuf form packaging publishes, so no per-type protobuf code is generated.

/* === Modules === */

mod input;
mod transcode;
mod validator;

/* === Re-Exports === */

pub use input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
pub use prost;
pub use validator::{PayloadError, Validator};

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use harmos_domain::__private::decoding_add;
use harmos_domain::schema::Described;
use harmos_domain::{Message, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Prepared<T> {
    postcard: Vec<u8>,
    memory: usize,
    value: PhantomData<fn() -> T>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T: Message> Prepared<T> {
    // Includes The Canonical Buffer Retained Until The Admitted Decode Completes.
    pub fn memory(&self) -> usize {
        self.memory
    }

    #[tracing::instrument(target = "harmos::grpc", name = "grpc.postcard.decode", level = "trace", skip_all, fields(bytes = self.postcard.len(), required_bytes = self.memory))]
    pub fn decode(self) -> Result<T, MessageError> {
        T::decode(&self.postcard)
    }

    #[cfg(feature = "streaming")]
    pub(crate) fn into_postcard(self) -> Vec<u8> {
        self.postcard
    }
}

/* === Public Functions === */

// These Read And Write `T`'s Message Bytes As Its Canonical Postcard Value; Host
// Adapters Whose Message Bytes Are Opaque Payloads Forward Them Instead.

#[tracing::instrument(target = "harmos::grpc", name = "grpc.protobuf.encode", level = "trace", skip_all)]
pub fn encode<T: Message + Described>(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    transcode::encode(&T::DEFINITION, &value.encode_to_vec(), bytes)
}

#[tracing::instrument(target = "harmos::grpc", name = "grpc.protobuf.decode", level = "trace", skip_all)]
pub fn decode<T: Message + Described>(input: DecodeInput<'_>) -> Result<T, MessageError> {
    T::decode(&transcode::decode(&T::DEFINITION, input)?)
}

#[tracing::instrument(target = "harmos::grpc", name = "grpc.prepare", level = "trace", skip_all, fields(canonical_bytes = tracing::field::Empty, required_bytes = tracing::field::Empty))]
pub fn prepare<T: Message + Described>(input: DecodeInput<'_>) -> Result<Prepared<T>, MessageError> {
    let postcard = transcode::decode(&T::DEFINITION, input)?;
    let memory = decoding_add(postcard.capacity(), T::decoding_memory(&postcard)?)?;
    tracing::Span::current().record("canonical_bytes", postcard.len());
    tracing::Span::current().record("required_bytes", memory);
    Ok(Prepared { postcard, memory, value: PhantomData })
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use harmos_domain::MemorySize;

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Row {
        bytes: Vec<u8>,
        label: String,
    }

    #[test]
    fn prepared_decode_bound() {
        let value = Row { bytes: vec![37; 8192], label: "prepared".into() };
        let mut encoded = Vec::new();
        encode(&value, &mut encoded).unwrap();
        let prepared = prepare::<Row>(DecodeInput::new(&encoded)).unwrap();
        let memory = prepared.memory();
        let canonical = prepared.postcard.capacity();
        assert_eq!(prepared.postcard, value.encode_to_vec());
        let decoded = prepared.decode().unwrap();
        assert_eq!(decoded.bytes, value.bytes);
        assert_eq!(decoded.label, value.label);
        assert!(decoded.memory_size() <= memory - canonical);
        assert!(prepare::<Row>(DecodeInput::new(&[0xff])).is_err());
    }
}
