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

mod input;
mod transcode;
mod validator;

pub use input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
pub use prost;
pub use validator::{PayloadError, Validator};

use std::marker::PhantomData;

use harmos_domain::__private::decoding_add;
use harmos_domain::schema::Described;
use harmos_domain::{Message, MessageError};

pub struct Prepared<T> {
    postcard: Vec<u8>,
    memory: usize,
    value: PhantomData<fn() -> T>,
}

struct Encoding {
    postcard: Vec<u8>,
    definition: harmos_domain::schema::declaration::Definition,
    length: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error(transparent)]
    Message(#[from] MessageError),

    #[error("protobuf encoding requires {required} wire bytes, limit is {limit}")]
    Limit { required: usize, limit: usize },
}

impl Encoding {
    fn new<T: Message + Described>(
        value: &T, retained: usize, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
    ) -> Result<Self, MessageError> {
        let declared = value.encoded_size();
        admit(decoding_add(retained, declared)?)?;
        let postcard = value.encode();
        // Handwritten Encoders Remain Cooperative; Retain Their Actual Returned Capacity.
        admit(decoding_add(retained, postcard.capacity())?)?;
        let length = transcode::encoded_len(&T::DEFINITION, &postcard)?;
        Ok(Self {
            postcard,
            definition: T::DEFINITION,
            length,
        })
    }

    fn write(self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>) -> Result<(), EncodeError> {
        let length = decoding_add(bytes.len(), self.length)?;
        if length > limit {
            return Err(EncodeError::Limit { required: length, limit });
        }
        let mut peak = decoding_add(self.postcard.capacity(), bytes.capacity())?;
        if length > bytes.capacity() {
            // Existing And Replacement Allocations May Coexist During Growth.
            peak = decoding_add(peak, length)?;
        }
        admit(peak)?;
        bytes.try_reserve_exact(self.length).map_err(|_| MessageError::borrowed("protobuf encoding allocation failed"))?;
        admit(decoding_add(self.postcard.capacity(), bytes.capacity())?)?;
        let retained = decoding_add(self.postcard.capacity(), bytes.capacity())?;
        transcode::encode_admitted(&self.definition, &self.postcard, bytes, retained, admit)?;
        drop(self);
        admit(bytes.capacity())?;
        Ok(())
    }
}

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

// These Read And Write `T`'s Message Bytes As Its Canonical Postcard Value; Host
// Adapters Whose Message Bytes Are Opaque Payloads Forward Them Instead.

#[tracing::instrument(target = "harmos::grpc", name = "grpc.protobuf.encode", level = "trace", skip_all)]
pub fn encode<T: Message + Described>(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    encode_bounded(value, bytes, usize::MAX)
}

pub(crate) fn encode_bounded<T: Message + Described>(value: &T, bytes: &mut Vec<u8>, limit: usize) -> Result<(), MessageError> {
    encode_admitted(value, bytes, limit, &mut |_| Ok(())).map_err(|error| match error {
        EncodeError::Message(error) => error,
        EncodeError::Limit { .. } => MessageError::borrowed("protobuf encoding exceeds wire ceiling"),
    })
}

/// Encodes Once Through Canonical Workspace, Admitting Native Buffer Peaks Before Allocation.
///
/// The Callback Owns Admission And Must Retain It Through The Returned Output's Lifetime.
/// Allocations Inside Handwritten Message Methods Remain The Encoder's Responsibility.
pub fn encode_admitted<T: Message + Described>(
    value: &T, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<(), EncodeError> {
    let encoding = Encoding::new(value, bytes.capacity(), admit)?;
    encoding.write(bytes, limit, admit)
}

/// Counts The Protobuf Projection Without Allocating Its Final Wire Buffer.
pub fn encoded_len<T: Message + Described>(value: &T) -> Result<usize, MessageError> {
    encoded_len_admitted(value, &mut |_| Ok(()))
}

/// Counts With The Caller's Existing Admission Until Canonical Workspace Drops.
pub fn encoded_len_admitted<T: Message + Described>(
    value: &T, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<usize, MessageError> {
    let encoding = Encoding::new(value, 0, admit)?;
    let length = encoding.length;
    drop(encoding);
    admit(0)?;
    Ok(length)
}

/// Finds The Largest Bytes-Field Payload Fitting A Message, Without Probe Payloads.
///
/// The Template Selects The Actual Variant And Metadata. The Named Path Must Select
/// Exactly One Present Bytes Field Outside Repeated Containers.
pub fn bytes_field_limit<T: Message + Described>(
    template: &T, path: &[&str], maximum_payload: usize, message_limit: usize,
) -> Result<usize, MessageError> {
    bytes_field_limit_admitted(template, path, maximum_payload, message_limit, &mut |_| Ok(()))
}

/// Uses The Caller's Existing Workspace Admission While Counting A Payload Allowance.
pub fn bytes_field_limit_admitted<T: Message + Described>(
    template: &T, path: &[&str], maximum_payload: usize, message_limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<usize, MessageError> {
    let encoding = Encoding::new(template, 0, admit)?;
    let minimum = transcode::parameterized_len(&encoding.definition, &encoding.postcard, path, 0)?;
    let mut low = 0;
    let mut high = maximum_payload.min(message_limit);
    if minimum > message_limit {
        high = 0;
    }
    while low < high {
        let distance = high - low;
        let middle = low + distance / 2 + distance % 2;
        let length = transcode::parameterized_len(&encoding.definition, &encoding.postcard, path, middle);
        if length.is_ok_and(|length| length <= message_limit) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    drop(encoding);
    admit(0)?;
    Ok(low)
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

#[cfg(test)]
mod tests {
    use super::*;
    use harmos_domain::MemorySize;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Row {
        bytes: Vec<u8>,
        label: String,
    }

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    enum Frame {
        Empty,
        Bytes(Row),
    }

    struct Handwritten {
        row: Row,
        encoded: AtomicUsize,
    }

    impl Message for Handwritten {
        const CODEC: &'static str = Row::CODEC;
        fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
            Row::decoding_memory(bytes)
        }
        fn encode(&self) -> Vec<u8> {
            self.encoded.fetch_add(1, Ordering::SeqCst);
            self.row.encode()
        }

        fn encoded_size(&self) -> usize {
            self.row.encoded_size()
        }

        fn decode(_: &[u8]) -> Result<Self, MessageError> {
            unreachable!("encoding fixture")
        }
    }

    impl Described for Handwritten {
        const NAME: &'static str = Row::NAME;
        const FIELDS: &'static [harmos_domain::schema::declaration::Field] = Row::FIELDS;
        const MARKER: &'static [u8] = Row::MARKER;
    }

    #[test]
    fn prepared_decode_bound() {
        let value = Row {
            bytes: vec![37; 8192],
            label: "prepared".into(),
        };
        let mut encoded = Vec::new();
        encode(&value, &mut encoded).unwrap();
        let prepared = prepare::<Row>(DecodeInput::new(&encoded)).unwrap();
        let memory = prepared.memory();
        let canonical = prepared.postcard.capacity();
        assert_eq!(prepared.postcard, value.encode());
        let decoded = prepared.decode().unwrap();
        assert_eq!(decoded.bytes, value.bytes);
        assert_eq!(decoded.label, value.label);
        assert!(decoded.memory_size() <= memory - canonical);
        assert!(prepare::<Row>(DecodeInput::new(&[0xff])).is_err());
    }

    #[test]
    fn encoding_limit_precedes_final_allocation() {
        let value = Row {
            bytes: vec![37; 8192],
            label: "prepared".into(),
        };
        let encoding = Encoding::new(&value, 0, &mut |_| Ok(())).unwrap();
        let length = encoding.length;
        let mut bytes = Vec::new();
        assert!(encoding.write(&mut bytes, length - 1, &mut |_| Ok(())).is_err());
        assert_eq!(bytes.capacity(), 0);

        let encoding = Encoding::new(&value, 0, &mut |_| Ok(())).unwrap();
        encoding.write(&mut bytes, length, &mut |_| Ok(())).unwrap();
        assert_eq!(bytes.len(), length);
        assert!(bytes.capacity() >= length);
    }

    #[test]
    fn admitted_encoding_retains_one_canonical_value() {
        let value = Handwritten {
            row: Row {
                bytes: vec![37; 8192],
                label: "prepared".into(),
            },
            encoded: AtomicUsize::new(0),
        };
        let mut bytes = Vec::new();
        let refused = encode_admitted(&value, &mut bytes, usize::MAX, &mut |_| Err(MessageError::borrowed("fixture refusal")));
        assert!(refused.is_err());
        assert_eq!(value.encoded.load(Ordering::SeqCst), 0);
        assert_eq!(bytes.capacity(), 0);

        let canonical = value.encoded_size();
        let refused = encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
            if required > canonical { Err(MessageError::borrowed("fixture refusal")) } else { Ok(()) }
        });
        assert!(refused.is_err());
        assert_eq!(value.encoded.load(Ordering::SeqCst), 1);
        assert_eq!(bytes.capacity(), 0);

        let mut claims = Vec::new();
        encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
            claims.push(required);
            Ok(())
        })
        .unwrap();
        assert_eq!(value.encoded.load(Ordering::SeqCst), 2);
        assert_eq!(claims.last(), Some(&bytes.capacity()));
        assert!(claims.iter().any(|claimed| *claimed >= canonical + bytes.len()));
    }

    #[test]
    fn parameterized_bytes_allowance_matches_actual_nested_wire_lengths() {
        let frame = |length| {
            Frame::Bytes(Row {
                bytes: vec![37; length],
                label: "metadata".into(),
            })
        };
        let template = frame(0);
        let path = ["Bytes", "field_0", "bytes"];
        for length in [0, 1, 127, 128, 16383, 16384] {
            let actual = frame(length);
            let limit = encoded_len(&actual).unwrap();
            assert_eq!(bytes_field_limit(&template, &path, length + 1, limit).unwrap(), length);
            if length > 0 {
                assert!(bytes_field_limit(&template, &path, length, limit - 1).unwrap() < length);
            }
        }
        let maximum = bytes_field_limit(&template, &path, usize::MAX, 4096).unwrap();
        assert!(maximum < 4096);
        assert!(encoded_len(&frame(maximum)).unwrap() <= 4096);
        assert!(encoded_len(&frame(maximum + 1)).unwrap() > 4096);
        assert!(bytes_field_limit(&Frame::Empty, &path, 1, 4096).is_err());
        assert!(bytes_field_limit(&template, &["Bytes", "field_0", "label"], 1, 4096).is_err());
        assert!(bytes_field_limit(&template, &["missing"], 1, 4096).is_err());
    }
}
