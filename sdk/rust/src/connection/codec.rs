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
            let required = super::admission::memory(&self.response, bytes, std::mem::size_of::<R>(), false)?;
            let permit = claim(&self.retained, required)?;
            let value = R::decode(bytes).map_err(invalid)?;
            return Ok(Output::Response(value, permit));
        }

        // Scan Borrowed Occurrences Before Allocating Any Generated Value; Several May Merge.
        let mut remaining = bytes;
        let mut occurrences = 0usize;
        while occurrence(&mut remaining)?.is_some() {
            occurrences += 1;
        }
        remaining = bytes;
        let mut selected = None;
        let mut retained = 0usize;
        let mut required = std::mem::size_of::<R>().checked_add(std::mem::size_of::<O>()).ok_or_else(exhausted)?;
        while let Some((tag, payload)) = occurrence(&mut remaining)? {
            let descriptor = if tag == 1 {
                self.output.as_ref()
            } else {
                Some(&self.response)
            };
            let root = if tag == 1 {
                std::mem::size_of::<O>()
            } else {
                std::mem::size_of::<R>()
            };
            let memory = if let Some(descriptor) = descriptor {
                super::admission::memory(descriptor, payload, root, occurrences > 1)?
            } else {
                // NoItems Ignores Fields, But Replaced Oneof Occurrences Still Must Be Valid.
                let mut fields = payload;
                while fields.has_remaining() {
                    let (field, wire) = decode_key(&mut fields).map_err(invalid)?;
                    skip_field(wire, field, &mut fields, DecodeContext::default()).map_err(invalid)?;
                }
                root
            };
            required = required.checked_add(memory).ok_or_else(super::admission::overflow)?;
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
        Ok(if selected == 1 {
            Output::Item(item, permit)
        } else {
            Output::Response(response, permit)
        })
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
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    thread_local! {
        static LIVE: Cell<isize> = const { Cell::new(0) };
    }

    // A Pass-Through Allocator Measures Only This Test Thread's Live Bytes
    struct Counting;

    #[allow(unsafe_code, reason = "measuring decoded allocations requires an allocator shim")]
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            account(layout.size().cast_signed());
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            account(-layout.size().cast_signed());
            unsafe { System.dealloc(pointer, layout) }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            account(size.cast_signed() - layout.size().cast_signed());
            unsafe { System.realloc(pointer, layout, size) }
        }
    }

    fn account(bytes: isize) {
        // Thread Teardown May Allocate After Its Slot Is Gone.
        let _ = LIVE.try_with(|live| live.set(live.get() + bytes));
    }

    #[test]
    fn representative_product_replies_fit_the_unchanged_retained_budget() {
        // Exercise the Same Admission and Decode Boundary as a Network Reply.
        fn reply<T: Message + ReflectMessage + Default + PartialEq + std::fmt::Debug>(value: T, budget: usize) {
            let encoded = value.encode_to_vec();
            let retained = Arc::new(Semaphore::new(budget));
            let decoder = Decode::<T, T> {
                retained: retained.clone(),
                response: value.descriptor(),
                output: None,
                envelope: false,
                marker: PhantomData,
            };
            let before = LIVE.get();
            let Output::Response(decoded, permit) = decoder.message(&encoded).unwrap() else {
                panic!("unary reply");
            };
            // The Claim Covers the Decoded Value Within Five Percent.
            let held = usize::try_from(LIVE.get() - before).unwrap() + std::mem::size_of::<T>();
            assert!(held <= permit.num_permits() && permit.num_permits() <= held + held / 20);
            assert_eq!(decoded, value);
            drop(permit);
            assert_eq!(retained.available_permits(), budget);
        }
        fn record(fields: impl IntoIterator<Item = (String, prost_types::Value)>) -> prost_types::Value {
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct { fields: fields.into_iter().collect() })),
            }
        }
        let budget = 4 * 1024 * 1024;

        // A 93 KB Plugin List Carries Each Entry's Operations and Full Descriptor Set.
        let files =
            harmos_protocol::host::PluginEntry::default().descriptor().parent_pool().file_descriptor_protos().cloned().collect::<Vec<_>>();
        let operations = (0..12)
            .map(|index| harmos_protocol::model::OperationContract {
                id: format!("fixture.operation_{index}"),
                request: "fixture.v1.Request".into(),
                response: "fixture.v1.Response".into(),
                visibility: harmos_protocol::model::Visibility::Public as i32,
                policy: Some(Default::default()),
                fingerprint: vec![7; 32],
                description: "Reads One Fixture Value".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let plugins = harmos_protocol::host::PluginsListResponse {
            entries: (0..2)
                .map(|index| harmos_protocol::host::PluginEntry {
                    name: format!("plugin-{index}"),
                    revision: vec![1; 16],
                    state: harmos_protocol::host::PluginState::Ready as i32,
                    description: Some(harmos_protocol::host::PluginDescription {
                        id: format!("fixture.plugin_{index}"),
                        version: "1.0.0".into(),
                        declarations: Some(harmos_protocol::model::Declarations {
                            operations: operations.clone(),
                            types: Some(prost_types::FileDescriptorSet { file: files.clone() }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        reply(plugins, budget);

        // A Document Holds Ordinary Text and Nested Properties.
        let text = prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue("document text".repeat(256))),
        };
        reply(record([("title".into(), text.clone()), ("body".into(), text)]), budget);

        // A Page of Samples Contains Repeated Numeric Rows.
        let samples = prost_types::ListValue {
            values: (0..256)
                .map(|index| {
                    record([
                        (
                            "time".into(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::NumberValue(f64::from(index))),
                            },
                        ),
                        (
                            "reading".into(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::NumberValue(0.5)),
                            },
                        ),
                    ])
                })
                .collect(),
        };
        reply(samples, budget);

        // Plot Snapshots Contain Several Repeated Series.
        let series = prost_types::Value {
            kind: Some(prost_types::value::Kind::ListValue(prost_types::ListValue {
                values: (0..128)
                    .map(|index| prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(f64::from(index))),
                    })
                    .collect(),
            })),
        };
        reply(
            prost_types::ListValue {
                values: (0..8).map(|index| record([(format!("series_{index}"), series.clone())])).collect(),
            },
            budget,
        );

        // Oversized Wire Values Still Refuse Before Decode Under That Same Budget.
        let oversized = prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue("x".repeat(budget))),
        };
        let decoder = Decode::<prost_types::Value, prost_types::Value> {
            retained: Arc::new(Semaphore::new(budget)),
            response: oversized.descriptor(),
            output: None,
            envelope: false,
            marker: PhantomData,
        };
        assert_eq!(decoder.message(&oversized.encode_to_vec()).err().unwrap().code(), tonic::Code::ResourceExhausted);
    }

    #[test]
    fn repeated_singular_messages_claim_their_merged_capacity() {
        // Eight Then One List Element Merge Into One Vec Past Its Power-of-Two Capacity, Whether a
        // Field or the Response Envelope Repeats.
        let list = |count| prost_types::Value {
            kind: Some(prost_types::value::Kind::ListValue(prost_types::ListValue {
                values: vec![prost_types::Value::default(); count],
            })),
        };
        let mut envelope = Vec::new();
        for count in [8, 1] {
            prost::encoding::encode_key(2, WireType::LengthDelimited, &mut envelope);
            list(count).encode_length_delimited(&mut envelope).unwrap();
        }
        for (bytes, envelope) in [([list(8).encode_to_vec(), list(1).encode_to_vec()].concat(), false), (envelope, true)] {
            let decoder = Decode::<prost_types::Value, prost_types::Value> {
                retained: Arc::new(Semaphore::new(1024 * 1024)),
                response: list(0).descriptor(),
                output: None,
                envelope,
                marker: PhantomData,
            };
            let before = LIVE.get();
            let Output::Response(decoded, permit) = decoder.message(&bytes).unwrap() else {
                panic!("unary reply");
            };
            assert_eq!(decoded, list(9));
            assert!(usize::try_from(LIVE.get() - before).unwrap() + std::mem::size_of::<prost_types::Value>() <= permit.num_permits());
        }
    }

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
        let required = super::super::admission::memory(&value.descriptor(), &bytes, 0, false).unwrap();
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
        let required = super::super::admission::memory(&value.descriptor(), &bytes, std::mem::size_of_val(&value), false).unwrap();
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
        let retained = super::super::admission::memory(&descriptor, &first.encode_to_vec(), 0, true).unwrap()
            + super::super::admission::memory(&descriptor, &second.encode_to_vec(), 0, true).unwrap();
        let budget = Arc::new(Semaphore::new(retained * 4));
        let decoder = Decode::<prost_types::ListValue, prost_types::ListValue> {
            retained: budget.clone(),
            response: descriptor.clone(),
            output: Some(descriptor),
            envelope: true,
            marker: PhantomData,
        };
        let Output::Response(value, permit) = decoder.message(&envelope).unwrap() else {
            panic!("final variant must win")
        };
        assert_eq!(value.values.len(), 8);
        assert_eq!(budget.available_permits(), retained * 3);
        drop(permit);
        assert_eq!(budget.available_permits(), retained * 4);
        assert_eq!(value.values.len(), 8);
    }
}
