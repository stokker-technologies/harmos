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
use crate::{MemorySize, MessageError, Protobuf};
use harmos_contract::__private::{DecodeInput, decoding_add, prost};

/* === Constants === */

/* === Macros === */
macro_rules! envelope {
    ($name:ident, $first:ident, $second:ident) => {
        #[cfg(feature = "runtime")]
        impl<A: MemorySize, B: MemorySize> $name<A, B> {
            pub fn project(
                item: crate::operations::StreamItem<Self>,
            ) -> crate::Result<
                $name<crate::operations::StreamItem<A>, crate::operations::StreamItem<B>>,
            > {
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
        impl<A: Protobuf, B: Protobuf> Protobuf for $name<A, B> {
            fn encode_to_vec(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(self.encoded_len());
                self.encode_into(&mut bytes);
                bytes
            }
            fn encode_into(&self, bytes: &mut Vec<u8>) {
                match self {
                    Self::$first(value) => encode(1, value, bytes),
                    Self::$second(value) => encode(2, value, bytes),
                }
            }
            fn encoded_len(&self) -> usize {
                let length = match self {
                    Self::$first(value) => value.encoded_len(),
                    Self::$second(value) => value.encoded_len(),
                };
                size(length)
            }
            fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
                let input = DecodeInput::new(bytes);
                let (tag, payload) = input
                    .variant(&[1, 2])?
                    .ok_or_else(|| MessageError::borrowed("empty operation envelope"))?;
                match tag {
                    1 => A::decode_input(payload).map(Self::$first),
                    _ => B::decode_input(payload).map(Self::$second),
                }
            }
            fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
                let input = DecodeInput::new(bytes);
                let (tag, payload) = input
                    .variant(&[1, 2])?
                    .ok_or_else(|| MessageError::borrowed("empty operation envelope"))?;
                let bound = match tag {
                    1 => A::decoding_memory_input(payload)?,
                    _ => B::decoding_memory_input(payload)?,
                };
                decoding_add(std::mem::size_of::<Self>(), bound)
            }
        }
        impl<A: MemorySize, B: MemorySize> MemorySize for $name<A, B> {
            fn memory_size(&self) -> usize {
                let heap = match self {
                    Self::$first(value) => {
                        value.memory_size().saturating_sub(std::mem::size_of::<A>())
                    }
                    Self::$second(value) => {
                        value.memory_size().saturating_sub(std::mem::size_of::<B>())
                    }
                };
                std::mem::size_of::<Self>().saturating_add(heap)
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

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
envelope!(InputEnvelope, Request, Item);
envelope!(OutputEnvelope, Item, Response);

/* === Public Functions === */

/* === Private Functions === */
pub(super) fn size(payload: usize) -> usize {
    1usize
        .saturating_add(prost::encoding::encoded_len_varint(payload as u64))
        .saturating_add(payload)
}

pub(super) fn encode<T: Protobuf>(tag: u32, value: &T, bytes: &mut Vec<u8>) {
    prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, bytes);
    prost::encoding::encode_varint(value.encoded_len() as u64, bytes);
    value.encode_into(bytes);
}

/* === Tests === */
