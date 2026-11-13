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
use std::{marker::PhantomData, sync::Arc};

// Third Party Imports
use bytes::{Buf, BufMut};
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};
use prost_reflect::MessageDescriptor;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
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
    pub(super) retained: Arc<Semaphore>,
    pub(super) response: MessageDescriptor,
    pub(super) output: Option<MessageDescriptor>,
    pub(super) envelope: bool,
    pub(super) limit: usize,
    pub(super) marker: PhantomData<fn() -> (R, O)>,
}

pub(super) struct Encode {
    limit: usize,
}
pub(super) struct Decode<R, O> {
    retained: Arc<Semaphore>,
    response: MessageDescriptor,
    output: Option<MessageDescriptor>,
    envelope: bool,
    marker: PhantomData<fn() -> (R, O)>,
}

/* === Enums === */

pub(super) enum Output<R, O> {
    Item(O, OwnedSemaphorePermit),
    Response(R, OwnedSemaphorePermit),
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
            retained: self.retained.clone(),
            response: self.response.clone(),
            output: self.output.clone(),
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
        // Tonic Presents One Contiguous, Already Wire-Bounded Message.
        let bytes = buffer.chunk();
        if bytes.len() != buffer.remaining() {
            return Err(Status::internal("noncontiguous client decode buffer"));
        }
        let value = self.message(bytes)?;
        buffer.advance(buffer.remaining());
        Ok(Some(value))
    }
}

impl<R: prost::Message + Default, O: prost::Message + Default> Decode<R, O> {
    fn message(&self, bytes: &[u8]) -> Result<Output<R, O>, Status> {
        if !self.envelope {
            let required = super::admission::memory(&self.response, bytes, std::mem::size_of::<R>())?;
            let permit = claim(&self.retained, required)?;
            let value = R::decode(bytes).map_err(invalid)?;
            return Ok(Output::Response(value, permit));
        }

        // Scan Borrowed Occurrences Before Allocating Any Generated Value.
        let mut remaining = bytes;
        let mut selected = None;
        let mut retained = 0usize;
        let mut required = std::mem::size_of::<R>().checked_add(std::mem::size_of::<O>()).ok_or_else(exhausted)?;
        while let Some((tag, payload)) = occurrence(&mut remaining)? {
            let descriptor = if tag == 1 { self.output.as_ref() } else { Some(&self.response) };
            let root = if tag == 1 { std::mem::size_of::<O>() } else { std::mem::size_of::<R>() };
            let memory = if let Some(descriptor) = descriptor {
                super::admission::memory(descriptor, payload, root)?
            } else {
                // NoItems Ignores Fields, But Replaced Oneof Occurrences Still Must Be Valid.
                let mut fields = payload;
                while fields.has_remaining() {
                    let (field, wire) = decode_key(&mut fields).map_err(invalid)?;
                    skip_field(wire, field, &mut fields, DecodeContext::default()).map_err(invalid)?;
                }
                root
            };
            required = required.checked_add(memory).ok_or_else(|| {
                Status::resource_exhausted("client retained_bytes overflow; raise retained_bytes or request smaller pages")
            })?;
            if selected != Some(tag) {
                retained = 0;
            }
            retained = retained.checked_add(memory).ok_or_else(exhausted)?;
            selected = Some(tag);
        }
        let selected = selected.ok_or_else(|| Status::data_loss("empty operation response envelope"))?;
        let mut scratch = claim(&self.retained, required)?;

        // Merge In Place; Oneof Replacement Discards The Earlier Variant.
        let mut response = R::default();
        let mut item = O::default();
        let mut previous = None;
        remaining = bytes;
        while let Some((tag, payload)) = occurrence(&mut remaining)? {
            if previous != Some(tag) {
                response = R::default();
                item = O::default();
                previous = Some(tag);
            }
            if tag == 1 {
                item.merge(payload).map_err(invalid)?;
            } else {
                response.merge(payload).map_err(invalid)?;
            }
        }
        let permit = scratch.split(retained).ok_or_else(exhausted)?;
        drop(scratch);
        Ok(if selected == 1 { Output::Item(item, permit) } else { Output::Response(response, permit) })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn claim(budget: &Arc<Semaphore>, required: usize) -> Result<OwnedSemaphorePermit, Status> {
    let required = u32::try_from(required).map_err(|_| exhausted())?;
    budget.clone().try_acquire_many_owned(required).map_err(|_| exhausted())
}

fn exhausted() -> Status {
    Status::resource_exhausted("client retained_bytes bound exceeded; raise retained_bytes or request smaller pages")
}

fn occurrence<'a>(remaining: &mut &'a [u8]) -> Result<Option<(u32, &'a [u8])>, Status> {
    while remaining.has_remaining() {
        let (tag, wire) = decode_key(remaining).map_err(invalid)?;
        if tag != 1 && tag != 2 {
            if wire == WireType::LengthDelimited {
                return Err(Status::data_loss("unknown operation response variant"));
            }
            skip_field(wire, tag, remaining, DecodeContext::default()).map_err(invalid)?;
            continue;
        }
        if wire != WireType::LengthDelimited {
            return Err(Status::data_loss("invalid operation response envelope"));
        }
        let length = usize::try_from(decode_varint(remaining).map_err(invalid)?).map_err(invalid)?;
        if length > remaining.len() {
            return Err(Status::data_loss("truncated operation response envelope"));
        }
        let (payload, rest) = remaining.split_at(length);
        *remaining = rest;
        return Ok(Some((tag, payload)));
    }
    Ok(None)
}

fn invalid(error: impl std::fmt::Display) -> Status {
    Status::data_loss(error.to_string())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use prost_reflect::ReflectMessage;

    #[derive(Default)]
    struct Undecodable;

    impl Message for Undecodable {
        fn encode_raw(&self, _: &mut impl BufMut) {
            unreachable!()
        }
        fn merge_field(&mut self, _: u32, _: WireType, _: &mut impl Buf, _: DecodeContext) -> Result<(), prost::DecodeError> {
            panic!("admission must refuse before decoding")
        }
        fn encoded_len(&self) -> usize {
            0
        }
        fn clear(&mut self) {}
    }

    #[test]
    fn compact_messages_are_refused_before_decode() {
        let value = prost_types::ListValue {
            values: vec![prost_types::Value::default(); 128],
        };
        let bytes = value.encode_to_vec();
        let required = super::super::admission::memory(&value.descriptor(), &bytes, 0).unwrap();
        let budget = Arc::new(Semaphore::new(required - 1));
        let decoder = Decode::<Undecodable, Undecodable> {
            retained: budget.clone(),
            response: value.descriptor(),
            output: Some(value.descriptor()),
            envelope: false,
            marker: PhantomData,
        };
        assert!(bytes.len() < required - 1);
        assert_eq!(decoder.message(&bytes).err().unwrap().code(), tonic::Code::ResourceExhausted);
        assert_eq!(budget.available_permits(), required - 1);
        let decoder = Decode { envelope: true, ..decoder };
        let mut envelope = Vec::new();
        prost::encoding::encode_key(1, WireType::LengthDelimited, &mut envelope);
        prost::encoding::encode_varint(bytes.len() as u64, &mut envelope);
        envelope.extend_from_slice(&bytes);
        assert_eq!(decoder.message(&envelope).err().unwrap().code(), tonic::Code::ResourceExhausted);
        assert_eq!(budget.available_permits(), required - 1);
    }

    #[test]
    fn queued_values_retain_claims_and_failed_decodes_return_them() {
        let value = prost_types::ListValue {
            values: vec![prost_types::Value::default(); 8],
        };
        let bytes = value.encode_to_vec();
        let required = super::super::admission::memory(&value.descriptor(), &bytes, std::mem::size_of_val(&value)).unwrap();
        let budget = Arc::new(Semaphore::new(required * 2));
        let decoder = Decode::<prost_types::ListValue, prost_types::ListValue> {
            retained: budget.clone(),
            response: value.descriptor(),
            output: Some(value.descriptor()),
            envelope: false,
            marker: PhantomData,
        };
        let first = decoder.message(&bytes).unwrap();
        let second = decoder.message(&bytes).unwrap();
        assert_eq!(budget.available_permits(), 0);
        assert_eq!(decoder.message(&bytes).err().unwrap().code(), tonic::Code::ResourceExhausted);
        drop(first);
        assert_eq!(budget.available_permits(), required);
        drop(second);
        assert_eq!(budget.available_permits(), required * 2);
        // Invalid UTF-8 Is Detected By Prost After Admission, Returning Its Claim.
        let decoder = Decode::<String, String> {
            retained: budget.clone(),
            response: String::default().descriptor(),
            output: None,
            envelope: false,
            marker: PhantomData,
        };
        assert_eq!(decoder.message(&[10, 1, 255]).err().unwrap().code(), tonic::Code::DataLoss);
        assert_eq!(budget.available_permits(), required * 2);
    }

    #[test]
    fn envelope_merges_repeated_values_and_releases_scratch() {
        let descriptor = prost_types::ListValue::default().descriptor();
        let first = prost_types::ListValue {
            values: vec![prost_types::Value::default(); 3],
        };
        let second = prost_types::ListValue {
            values: vec![prost_types::Value::default(); 5],
        };
        let mut envelope = Vec::new();
        for (tag, value) in [(1, &second), (2, &first), (2, &second)] {
            prost::encoding::encode_key(tag, WireType::LengthDelimited, &mut envelope);
            value.encode_length_delimited(&mut envelope).unwrap();
        }
        let retained = super::super::admission::memory(&descriptor, &first.encode_to_vec(), 0).unwrap()
            + super::super::admission::memory(&descriptor, &second.encode_to_vec(), 0).unwrap();
        let budget = Arc::new(Semaphore::new(retained * 4));
        let decoder = Decode::<prost_types::ListValue, prost_types::ListValue> {
            retained: budget.clone(),
            response: descriptor.clone(),
            output: Some(descriptor),
            envelope: true,
            marker: PhantomData,
        };
        let Output::Response(value, permit) = decoder.message(&envelope).unwrap() else { panic!("final variant must win") };
        assert_eq!(value.values.len(), 8);
        assert_eq!(budget.available_permits(), retained * 3);
        drop(permit);
        assert_eq!(budget.available_permits(), retained * 4);
        assert_eq!(value.values.len(), 8);
    }
}
