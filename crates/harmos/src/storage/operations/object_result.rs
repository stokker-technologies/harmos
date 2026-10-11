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
use std::borrow::Cow;

// Third Party Imports

// Local Imports
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
use crate::__private::decoding_add;

use crate::schema::{Cardinality, Described, Field, Kind};
use crate::storage::{ObjectEntry, ObjectMetadata};
#[cfg(feature = "grpc")]
use crate::storage_codec::{Nested, Value};
use crate::{MemorySize, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Result<T> = std::result::Result<T, MessageError>;

/* === Structs === */

/* === Enums === */
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectResult {
    Value(ObjectEntry),
    Fields(ObjectMetadata),
}

/* === Traits === */

/* === Implementations === */
impl ObjectResult {
    #[cfg(feature = "runtime")]
    pub(super) fn wrapping_memory<T: MemorySize>(value: &T) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(value.memory_size().saturating_sub(std::mem::size_of::<T>()))
    }
}
impl MemorySize for ObjectResult {
    fn memory_size(&self) -> usize {
        let heap = match self {
            Self::Value(value) => value
                .memory_size()
                .saturating_sub(std::mem::size_of::<ObjectEntry>()),
            Self::Fields(value) => value
                .memory_size()
                .saturating_sub(std::mem::size_of::<ObjectMetadata>()),
        };
        std::mem::size_of::<Self>().saturating_add(heap)
    }
}
impl Described for ObjectResult {
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("value"),
            tag: 1,
            cardinality: Cardinality::Variant,
            kind: Kind::Message {
                fields: Cow::Borrowed(ObjectEntry::FIELDS),
            },
        },
        Field {
            name: Cow::Borrowed("fields"),
            tag: 2,
            cardinality: Cardinality::Variant,
            kind: Kind::Message {
                fields: Cow::Borrowed(ObjectMetadata::FIELDS),
            },
        },
    ];
}
#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for ObjectResult {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
        bytes
    }
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Value(value) => Nested::<ObjectEntry>::write(value, 1, bytes),
            Self::Fields(value) => Nested::<ObjectMetadata>::write(value, 2, bytes),
        }
    }
    fn encoded_len(&self) -> usize {
        match self {
            Self::Value(value) => Nested::<ObjectEntry>::size(value, 1),
            Self::Fields(value) => Nested::<ObjectMetadata>::size(value, 2),
        }
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }
    fn decoding_memory_input(input: DecodeInput<'_>) -> Result<usize> {
        let (tag, child) = input
            .variant(&[1, 2])?
            .ok_or_else(|| MessageError::borrowed("missing object result variant"))?;
        let heap = match tag {
            1 => ObjectEntry::decoding_memory_input(child)?
                .saturating_sub(std::mem::size_of::<ObjectEntry>()),
            _ => ObjectMetadata::decoding_memory_input(child)?
                .saturating_sub(std::mem::size_of::<ObjectMetadata>()),
        };
        decoding_add(std::mem::size_of::<Self>(), heap)
    }
    fn decode_input(input: DecodeInput<'_>) -> Result<Self> {
        let (tag, child) = input
            .variant(&[1, 2])?
            .ok_or_else(|| MessageError::borrowed("missing object result variant"))?;
        match tag {
            1 => ObjectEntry::decode_input(child).map(Self::Value),
            _ => ObjectMetadata::decode_input(child).map(Self::Fields),
        }
    }
}

impl crate::__private::PostcardValue for ObjectResult {
    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Value(value) => {
                0u32.write(bytes);
                value.write(bytes);
            }
            Self::Fields(value) => {
                1u32.write(bytes);
                value.write(bytes);
            }
        }
    }
    fn size(&self) -> usize {
        1usize.saturating_add(match self {
            Self::Value(value) => value.size(),
            Self::Fields(value) => value.size(),
        })
    }
    fn memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        input.nested(|input| {
            let heap = match input.value::<u32>()? {
                0 => <ObjectEntry as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<ObjectEntry>()),
                1 => <ObjectMetadata as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<ObjectMetadata>()),
                _ => return Err(MessageError::borrowed("unknown object result variant")),
            };
            decoding_add(std::mem::size_of::<Self>(), heap)
        })
    }
    fn read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Self> {
        input.nested(|input| match input.value::<u32>()? {
            0 => <ObjectEntry as crate::__private::PostcardValue>::read(input).map(Self::Value),
            1 => <ObjectMetadata as crate::__private::PostcardValue>::read(input).map(Self::Fields),
            _ => Err(MessageError::borrowed("unknown object result variant")),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
