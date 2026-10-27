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

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::codec::{Encode, Transcoded, Wire};
use crate::{MemorySize, MessageError};
use harmos_domain::__private::decoding_add;
use harmos_domain::Portable;
use harmos_sdk::grpc::{DecodeInput, prost};

/* === Constants === */

/* === Macros === */

macro_rules! envelope {
    ($name:ident, $first:ident, $second:ident) => {
        impl<A: MemorySize, B: MemorySize> $name<A, B> {
            pub fn project(
                item: crate::operations::StreamItem<Self>,
            ) -> crate::Result<$name<crate::operations::StreamItem<A>, crate::operations::StreamItem<B>>> {
                Ok(
                    match item.project(|value| match value {
                        Self::$first(value) => Ok(value),
                        Self::$second(value) => Err(value),
                    })? {
                        Ok(value) => $name::$first(value),
                        Err(value) => $name::$second(value),
                    },
                )
            }
        }
        impl<W, A, B> Wire<$name<A, B>> for W
        where
            W: Wire<A> + Wire<B>,
            A: Send + Sync + 'static,
            B: Send + Sync + 'static,
        {
            fn encode(value: &$name<A, B>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
                match value {
                    $name::$first(value) => delimited(1, bytes, |bytes| <W as Wire<A>>::encode(value, bytes)),
                    $name::$second(value) => delimited(2, bytes, |bytes| <W as Wire<B>>::encode(value, bytes)),
                }
            }
            fn encode_admitted(
                value: &$name<A, B>, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), tonic::Status>,
            ) -> Result<(), tonic::Status> {
                match value {
                    $name::$first(value) => delimited_admitted(1, bytes, limit, admit, |bytes, admit| {
                        <W as Wire<A>>::encode_admitted(value, bytes, limit, admit)
                    }),
                    $name::$second(value) => delimited_admitted(2, bytes, limit, admit, |bytes, admit| {
                        <W as Wire<B>>::encode_admitted(value, bytes, limit, admit)
                    }),
                }
            }
            fn decode_admitted<C>(
                input: DecodeInput<'_>, admit: impl FnOnce(usize) -> Result<C, tonic::Status>,
            ) -> Result<($name<A, B>, C), tonic::Status> {
                let (tag, payload) = input
                    .variant(&[1, 2])
                    .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?
                    .ok_or_else(|| tonic::Status::invalid_argument("empty operation envelope"))?;
                let admit = |bound| {
                    let bytes = decoding_add(std::mem::size_of::<$name<A, B>>(), bound)
                        .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?;
                    admit(bytes)
                };
                match tag {
                    1 => <W as Wire<A>>::decode_admitted(payload, admit).map(|(value, claim)| ($name::$first(value), claim)),
                    _ => <W as Wire<B>>::decode_admitted(payload, admit).map(|(value, claim)| ($name::$second(value), claim)),
                }
            }
        }
        impl<A: MemorySize, B: MemorySize> MemorySize for $name<A, B> {
            fn memory_size(&self) -> usize {
                let heap = match self {
                    Self::$first(value) => value.memory_size().saturating_sub(std::mem::size_of::<A>()),
                    Self::$second(value) => value.memory_size().saturating_sub(std::mem::size_of::<B>()),
                };
                std::mem::size_of::<Self>().saturating_add(heap)
            }
        }
    };
}

envelope!(InputEnvelope, Request, Item);

envelope!(OutputEnvelope, Item, Response);

/* === Type Aliases === */

/* === Structs === */

/// An Already Encoded Complete gRPC Message, Retaining Its Actual Byte Owner.
pub struct PreparedFrame {
    bytes: crate::operations::Response<Vec<u8>>,
}

/* === Enums === */

#[derive(Debug, PartialEq)]
pub enum InputEnvelope<R, I> {
    Request(R),
    Item(I),
}

#[derive(Debug, PartialEq)]
pub enum OutputEnvelope<I, R> {
    Item(I),
    Response(R),
}

/* === Traits === */

/* === Implementations === */

impl PreparedFrame {
    pub fn from_owner(bytes: crate::operations::Response<Vec<u8>>) -> Self {
        Self { bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Encode<Transcoded> for PreparedFrame {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        bytes.extend_from_slice(&self.bytes);
        Ok(())
    }

    fn write_admitted(
        &self, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), tonic::Status>,
    ) -> Result<(), tonic::Status> {
        super::codec::encode_bytes(&self.bytes, bytes, limit, admit)
    }
}

/* === Public Functions === */

/// Prepares The Complete Opening Frame Of A Typed Input Stream Once.
pub fn prepare_input_request<O: crate::operations::OperationDeclaration>(
    request: &O, limit: usize,
) -> Result<PreparedFrame, tonic::Status> {
    prepare(1, request, limit)
}

/// Prepares The Complete Item Frame Of A Typed Input Stream Once.
///
/// Standalone Preparation Uses The Frame Limit Without Claiming An Aggregate Budget.
/// Callers With Native Claims Can Encode Through The Admitted Wire Hook And Supply
/// The Retained Byte Owner Through PreparedFrame::from_owner.
pub fn prepare_input_item<O: crate::operations::OperationDeclaration>(
    item: &<O::Input as crate::operations::Direction>::Item, limit: usize,
) -> Result<PreparedFrame, tonic::Status> {
    prepare(2, item, limit)
}

/// Finds A Declared Input Item's Bytes Allowance Including Its Native Envelope.
pub fn input_payload_limit<O: crate::operations::OperationDeclaration>(
    template: &<O::Input as crate::operations::Direction>::Item, path: &[&str], maximum_payload: usize, frame_limit: usize,
) -> Result<usize, MessageError> {
    input_payload_limit_admitted::<O>(template, path, maximum_payload, frame_limit, &mut |_| Ok(()))
}

/// Uses Existing Caller Admission For A Declared Input Item's Allowance Template.
pub fn input_payload_limit_admitted<O: crate::operations::OperationDeclaration>(
    template: &<O::Input as crate::operations::Direction>::Item, path: &[&str], maximum_payload: usize, frame_limit: usize,
    admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<usize, MessageError> {
    harmos_sdk::grpc::bytes_field_limit_admitted(template, path, maximum_payload, payload_limit(2, frame_limit), admit)
}

/// Finds An Output Item's Bytes Allowance Including Its Native Envelope.
pub fn output_payload_limit<T: Portable>(
    template: &T, path: &[&str], maximum_payload: usize, frame_limit: usize,
) -> Result<usize, MessageError> {
    output_payload_limit_admitted(template, path, maximum_payload, frame_limit, &mut |_| Ok(()))
}

/// Uses Existing Caller Admission For The Small Canonical Allowance Template.
pub fn output_payload_limit_admitted<T: Portable>(
    template: &T, path: &[&str], maximum_payload: usize, frame_limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<usize, MessageError> {
    harmos_sdk::grpc::bytes_field_limit_admitted(template, path, maximum_payload, payload_limit(1, frame_limit), admit)
}

/// Counts The Complete Output Item Frame Without Allocating Its Wire Buffer.
pub fn output_item_len<T: Portable>(item: &T) -> Result<usize, MessageError> {
    output_item_len_admitted(item, &mut |_| Ok(()))
}

/// Counts A Complete Output Item Using Existing Caller Workspace Admission.
pub fn output_item_len_admitted<T: Portable>(
    item: &T, admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<usize, MessageError> {
    let payload = harmos_sdk::grpc::encoded_len_admitted(item, admit)?;
    let (_, prefix) = header(1, payload);
    decoding_add(payload, prefix)
}

// Frame One Length-Delimited Field Around A Payload Written Into Scratch.
pub(super) fn delimited(
    tag: u32, bytes: &mut Vec<u8>, write: impl FnOnce(&mut Vec<u8>) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    let start = bytes.len();
    write(bytes)?;
    let (header, length) = header(tag, bytes.len() - start);
    bytes.try_reserve_exact(length).map_err(|_| MessageError::borrowed("gRPC envelope allocation failed"))?;
    insert(bytes, start, &header[..length]);
    Ok(())
}

pub(super) fn delimited_admitted(
    tag: u32, bytes: &mut Vec<u8>, limit: usize, admit: &mut dyn FnMut(usize) -> Result<(), tonic::Status>,
    write: impl FnOnce(&mut Vec<u8>, &mut dyn FnMut(usize) -> Result<(), tonic::Status>) -> Result<(), tonic::Status>,
) -> Result<(), tonic::Status> {
    let start = bytes.len();
    write(bytes, admit)?;
    let (header, length) = header(tag, bytes.len() - start);
    let required = bytes.len().checked_add(length).ok_or_else(|| tonic::Status::resource_exhausted("gRPC envelope size overflows"))?;
    if required > limit {
        return Err(tonic::Status::resource_exhausted("gRPC encoded message exceeds wire ceiling"));
    }
    super::codec::reserve(bytes, required, admit)?;
    insert(bytes, start, &header[..length]);
    Ok(())
}

/* === Private Functions === */

fn prepare<T: Portable>(tag: u32, value: &T, limit: usize) -> Result<PreparedFrame, tonic::Status> {
    let mut bytes = Vec::new();
    delimited_admitted(tag, &mut bytes, limit, &mut |_| Ok(()), |bytes, admit| {
        <Transcoded as Wire<T>>::encode_admitted(value, bytes, limit, admit)
    })?;
    Ok(PreparedFrame::from_owner(crate::operations::Response::new(bytes)))
}

fn payload_limit(tag: u32, limit: usize) -> usize {
    let mut low = 0;
    let mut high = limit;
    while low < high {
        let distance = high - low;
        let middle = low + distance / 2 + distance % 2;
        let (_, prefix) = header(tag, middle);
        if middle.checked_add(prefix).is_some_and(|length| length <= limit) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn header(tag: u32, payload: usize) -> ([u8; 15], usize) {
    // A Protobuf Key Uses At Most Five Bytes; A u64 Length Uses At Most Ten.
    let mut bytes = [0; 15];
    let mut remaining = &mut bytes[..];
    prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, &mut remaining);
    prost::encoding::encode_varint(payload as u64, &mut remaining);
    let length = 15 - remaining.len();
    (bytes, length)
}

fn insert(bytes: &mut Vec<u8>, start: usize, header: &[u8]) {
    let end = bytes.len();
    bytes.resize(end + header.len(), 0);
    bytes.copy_within(start..end, start + header.len());
    bytes[start..start + header.len()].copy_from_slice(header);
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::{borrow::Borrow, sync::Arc};

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Chunk {
        offset: u64,
        data: Vec<u8>,
    }

    #[test]
    fn payload_allowance_includes_complete_output_envelope() {
        let template = Chunk { offset: u64::MAX, data: Vec::new() };
        for maximum in [2, 64, 127, 128, 16383, 16384] {
            let payload = output_payload_limit(&template, &["data"], usize::MAX, maximum).unwrap();
            if payload == 0 {
                continue;
            }
            let frame = Chunk { offset: u64::MAX, data: vec![37; payload] };
            assert!(output_item_len(&frame).unwrap() <= maximum);
            let next = Chunk { offset: u64::MAX, data: vec![37; payload + 1] };
            assert!(output_item_len(&next).unwrap() > maximum);
        }
    }

    #[test]
    fn prepared_input_is_the_complete_frame_and_keeps_its_byte_owner() {
        let value = Chunk { offset: 17, data: vec![37; 128] };
        let frame = prepare(2, &value, 4096).unwrap();
        let mut expected = Vec::new();
        <Transcoded as Wire<InputEnvelope<(), Chunk>>>::encode(&InputEnvelope::Item(value), &mut expected).unwrap();
        assert_eq!(frame.len(), expected.len());
        assert!(prepare(2, &Chunk { offset: 17, data: vec![37; 128] }, frame.len() - 1).is_err());

        struct Owner(Vec<u8>);
        impl Borrow<Vec<u8>> for Owner {
            fn borrow(&self) -> &Vec<u8> {
                &self.0
            }
        }
        let owner = Arc::new(Owner(expected.clone()));
        let weak = Arc::downgrade(&owner);
        let frame = PreparedFrame::from_owner(crate::operations::Response::from_owner(owner));
        let mut bytes = Vec::new();
        frame.write(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
        assert!(weak.upgrade().is_some());
        drop(frame);
        assert!(weak.upgrade().is_none());
    }
}
