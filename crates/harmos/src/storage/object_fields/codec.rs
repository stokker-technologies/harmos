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
use std::marker::PhantomData;

// Third Party Imports

// Local Imports
use super::*;
use crate::__private::decoding_add;
#[cfg(feature = "grpc")]
use crate::__private::{DecodeInput, DecodeScalar, DecodeValue, decoding_vector};

use crate::MessageError;
use crate::schema::{Cardinality, Described, Field, Kind};
#[cfg(feature = "grpc")]
use crate::storage::codec::{Id, Unsigned};
use crate::storage::codec::{Nested, Repeated, Value, product};
use crate::storage::object_values::Key;
#[cfg(feature = "grpc")]
use crate::storage::position::codec::{
    field_len, identifier, text_field, uint_field, write_bytes, write_header,
};

/* === Constants === */

/* === Macros === */
product!(ObjectFields { projection: Names => 1, filters: Bounded<ObjectFilter> => 2, order: Bounded<ObjectSort> => 3 });
product!(ObjectFilter { field: FieldName => 1, comparison: Comparison => 2, value: FilterScalar => 3 });
product!(ObjectSort { field: FieldName => 1, descending: Boolean => 2, nulls: NullOrder => 3 });
product!(ObjectMetadata {
    schema: Nested<SchemaKey> => 1, item: Key => 2,
    position: Nested<StorageCursor> => 3, version: Nested<ObjectVersionToken> => 4,
    values: Bounded<ObjectScalar> => 5,
});
macro_rules! enumeration {
    ($codec:ident, $ty:ident, {$($variant:ident => $number:literal),+}) => {
        impl Value for $codec {
            fn postcard_write(value: &$ty, bytes: &mut Vec<u8>) {
                let value: u64 = match value { $($ty::$variant => $number),+ };
                crate::__private::PostcardValue::write(&value, bytes);
            }
            fn postcard_size(value: &$ty) -> usize {
                let value: u64 = match value { $($ty::$variant => $number),+ };
                crate::__private::PostcardValue::size(&value)
            }
            fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
                Self::postcard_read(input)?; Ok(0)
            }
            fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<$ty> {
                match input.value::<u64>()? { $($number => Ok($ty::$variant),)+ _ => Err(MessageError::borrowed("invalid object scalar selector")) }
            }

            type Native = $ty;
            const KIND: Kind = Kind::U64;
    #[cfg(feature = "grpc")]
            fn write(value: &$ty, tag: u32, bytes: &mut Vec<u8>) { Unsigned::write(&match value { $($ty::$variant => $number),+ }, tag, bytes); }
    #[cfg(feature = "grpc")]
            fn size(value: &$ty, tag: u32) -> usize { Unsigned::size(&match value { $($ty::$variant => $number),+ }, tag) }
    #[cfg(feature = "grpc")]
            fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> { Self::read(input, tag).map(|_| 0) }
    #[cfg(feature = "grpc")]
            fn read(input: DecodeInput<'_>, tag: u32) -> Result<$ty> {
                match Unsigned::read(input, tag)? { $($number => Ok($ty::$variant),)+ _ => Err(MessageError::borrowed("invalid object scalar selector")) }
            }
        }
    };
}
enumeration!(Comparison, ObjectComparison, {Equal => 0, NotEqual => 1, Less => 2, LessOrEqual => 3, Greater => 4, GreaterOrEqual => 5});
enumeration!(NullOrder, ObjectNullOrder, {First => 0, Last => 1});

/* === Type Aliases === */
type Result<T> = std::result::Result<T, MessageError>;

/* === Structs === */
struct Names;
struct FieldName;
struct Boolean;
struct FilterScalar;
struct Comparison;
struct NullOrder;
struct Bounded<T>(PhantomData<T>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Value for FieldName {
    fn postcard_write(value: &Identifier, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(value, bytes);
    }

    fn postcard_size(value: &Identifier) -> usize {
        crate::__private::PostcardValue::size(value)
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        let heap = <Identifier as crate::__private::PostcardValue>::memory(input)?
            .saturating_sub(std::mem::size_of::<Identifier>());
        if heap > 256 {
            return Err(MessageError::borrowed("object query field exceeds limit"));
        }
        Ok(heap)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Identifier> {
        Self::postcard_memory(&mut input.clone())?;
        <Identifier as crate::__private::PostcardValue>::read(input)
    }

    type Native = Identifier;
    const KIND: Kind = Kind::String;
    #[cfg(feature = "grpc")]
    fn write(value: &Identifier, tag: u32, bytes: &mut Vec<u8>) {
        Id::write(value, tag, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &Identifier, tag: u32) -> usize {
        Id::size(value, tag)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let memory = Id::memory(input, tag)?;
        if memory > 256 {
            return Err(MessageError::borrowed("object field name exceeds limit"));
        }
        Ok(memory)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Identifier> {
        Id::read(input, tag)
    }
}
impl Value for FilterScalar {
    fn postcard_write(value: &ObjectScalar, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(value, bytes);
    }

    fn postcard_size(value: &ObjectScalar) -> usize {
        crate::__private::PostcardValue::size(value)
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        let heap = <ObjectScalar as crate::__private::PostcardValue>::memory(input)?
            .saturating_sub(std::mem::size_of::<ObjectScalar>());
        if heap > MAX_SELECTOR_BYTES {
            return Err(MessageError::borrowed("object query field exceeds limit"));
        }
        Ok(heap)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<ObjectScalar> {
        Self::postcard_memory(&mut input.clone())?;
        <ObjectScalar as crate::__private::PostcardValue>::read(input)
    }

    type Native = ObjectScalar;
    const KIND: Kind = Nested::<ObjectScalar>::KIND;
    #[cfg(feature = "grpc")]
    fn write(value: &ObjectScalar, tag: u32, bytes: &mut Vec<u8>) {
        Nested::<ObjectScalar>::write(value, tag, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &ObjectScalar, tag: u32) -> usize {
        Nested::<ObjectScalar>::size(value, tag)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let memory =
            <ObjectScalar as crate::__private::Protobuf>::decoding_memory_input(input.field(tag)?)?;
        let text = memory.saturating_sub(std::mem::size_of::<ObjectScalar>());
        if text > MAX_SELECTOR_BYTES {
            return Err(MessageError::borrowed(
                "predicate text exceeds query bounds",
            ));
        }
        Ok(text)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<ObjectScalar> {
        Nested::<ObjectScalar>::read(input, tag)
    }
}
impl Value for Boolean {
    crate::storage::codec::postcard_field!();
    type Native = bool;
    const KIND: Kind = Kind::Bool;
    #[cfg(feature = "grpc")]
    fn write(value: &bool, tag: u32, bytes: &mut Vec<u8>) {
        crate::__private::prost::encoding::bool::encode(tag, value, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &bool, tag: u32) -> usize {
        crate::__private::prost::encoding::bool::encoded_len(tag, value)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        Unsigned::memory(input, tag)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<bool> {
        Ok(Unsigned::read(input, tag)? != 0)
    }
}
impl<T: crate::storage::codec::Native> Value for Bounded<T> {
    fn postcard_write(value: &Vec<T>, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(value, bytes);
    }

    fn postcard_size(value: &Vec<T>) -> usize {
        crate::__private::PostcardValue::size(value)
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        if input.clone().value::<usize>()? > MAX_COLUMNS {
            return Err(MessageError::borrowed("too many object query fields"));
        }
        Ok(<Vec<T> as crate::__private::PostcardValue>::memory(input)?
            .saturating_sub(std::mem::size_of::<Vec<T>>()))
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Vec<T>> {
        if input.clone().value::<usize>()? > MAX_COLUMNS {
            return Err(MessageError::borrowed("too many object query fields"));
        }
        <Vec<T> as crate::__private::PostcardValue>::read(input)
    }

    type Native = Vec<T>;
    const KIND: Kind = Repeated::<T>::KIND;
    const CARDINALITY: Cardinality = Cardinality::Repeated;
    #[cfg(feature = "grpc")]
    fn write(value: &Vec<T>, tag: u32, bytes: &mut Vec<u8>) {
        Repeated::<T>::write(value, tag, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &Vec<T>, tag: u32) -> usize {
        Repeated::<T>::size(value, tag)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        bounded_count(input, tag)?;
        Repeated::<T>::memory(input, tag)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<T>> {
        Repeated::<T>::read(input, tag)
    }
}
impl Value for Names {
    fn postcard_write(value: &Vec<Identifier>, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(value, bytes);
    }

    fn postcard_size(value: &Vec<Identifier>) -> usize {
        crate::__private::PostcardValue::size(value)
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        input.nested(|input| {
            let count: usize = input.value()?;
            if count > MAX_COLUMNS {
                return Err(MessageError::borrowed("too many object query fields"));
            }
            input.claim(count)?;
            let mut heap = count * std::mem::size_of::<Identifier>();
            for _ in 0..count {
                heap = decoding_add(heap, FieldName::postcard_memory(input)?)?;
            }
            Ok(heap)
        })
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Vec<Identifier>> {
        Self::postcard_memory(&mut input.clone())?;
        <Vec<Identifier> as crate::__private::PostcardValue>::read(input)
    }

    type Native = Vec<Identifier>;
    const KIND: Kind = Kind::String;
    const CARDINALITY: Cardinality = Cardinality::Repeated;
    #[cfg(feature = "grpc")]
    fn write(names: &Vec<Identifier>, tag: u32, bytes: &mut Vec<u8>) {
        for name in names {
            Id::write(name, tag, bytes);
        }
    }

    #[cfg(feature = "grpc")]
    fn size(names: &Vec<Identifier>, tag: u32) -> usize {
        names.iter().fold(0usize, |size, name| {
            size.saturating_add(Id::size(name, tag))
        })
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let count = bounded_count(input, tag)?;
        let mut memory = count * std::mem::size_of::<Identifier>();
        input.fields(&mut |field| {
            if field.tag == tag {
                let text = field.text()?;
                if text.len() > 256 {
                    return Err(MessageError::borrowed("object field name exceeds limit"));
                }
                memory = decoding_add(memory, text.len())?;
            }
            Ok(())
        })?;
        Ok(memory)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<Vec<Identifier>> {
        let mut values = decoding_vector(bounded_count(input, tag)?)?;
        input.fields(&mut |field| {
            if field.tag == tag {
                values.push(identifier(field.text()?)?);
            }
            Ok(())
        })?;
        Ok(values)
    }
}
impl Described for ObjectScalar {
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("null"),
            tag: 1,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[]),
            },
            cardinality: Cardinality::Variant,
        },
        Field {
            name: Cow::Borrowed("boolean"),
            tag: 2,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[Field {
                    name: Cow::Borrowed("value"),
                    tag: 1,
                    kind: Kind::Bool,
                    cardinality: Cardinality::Required,
                }]),
            },
            cardinality: Cardinality::Variant,
        },
        Field {
            name: Cow::Borrowed("signed"),
            tag: 3,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[Field {
                    name: Cow::Borrowed("value"),
                    tag: 1,
                    kind: Kind::I64,
                    cardinality: Cardinality::Required,
                }]),
            },
            cardinality: Cardinality::Variant,
        },
        Field {
            name: Cow::Borrowed("unsigned"),
            tag: 4,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[Field {
                    name: Cow::Borrowed("value"),
                    tag: 1,
                    kind: Kind::U64,
                    cardinality: Cardinality::Required,
                }]),
            },
            cardinality: Cardinality::Variant,
        },
        Field {
            name: Cow::Borrowed("float"),
            tag: 5,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[Field {
                    name: Cow::Borrowed("value"),
                    tag: 1,
                    kind: Kind::F64,
                    cardinality: Cardinality::Required,
                }]),
            },
            cardinality: Cardinality::Variant,
        },
        Field {
            name: Cow::Borrowed("text"),
            tag: 6,
            kind: Kind::Message {
                fields: Cow::Borrowed(&[Field {
                    name: Cow::Borrowed("value"),
                    tag: 1,
                    kind: Kind::String,
                    cardinality: Cardinality::Required,
                }]),
            },
            cardinality: Cardinality::Variant,
        },
    ];
}
#[cfg(feature = "grpc")]
impl ObjectScalar {
    fn payload_size(&self) -> (u32, usize) {
        use crate::__private::prost::encoding::{bool, double, int64, uint64};
        match self {
            Self::Null => (1, 0),
            Self::Bool(value) => (2, bool::encoded_len(1, value)),
            Self::Signed(value) => (3, int64::encoded_len(1, value)),
            Self::Unsigned(value) => (4, uint64::encoded_len(1, value)),
            Self::Float(value) => (5, double::encoded_len(1, value)),
            Self::Text(value) => (6, field_len(value.len())),
        }
    }
}
#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for ObjectScalar {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
        bytes
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        use crate::__private::prost::encoding::{bool, double, int64, uint64};
        let (tag, length) = self.payload_size();
        write_header(tag, length, bytes);
        match self {
            Self::Null => (),
            Self::Bool(value) => bool::encode(1, value, bytes),
            Self::Signed(value) => int64::encode(1, value, bytes),
            Self::Unsigned(value) => uint64::encode(1, value, bytes),
            Self::Float(value) => double::encode(1, value, bytes),
            Self::Text(value) => write_bytes(1, value.as_bytes(), bytes),
        }
    }

    fn encoded_len(&self) -> usize {
        field_len(self.payload_size().1)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }

    fn decoding_memory(bytes: &[u8]) -> Result<usize> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> Result<usize> {
        let (tag, child) = input
            .variant(&[1, 2, 3, 4, 5, 6])?
            .ok_or_else(|| MessageError::borrowed("missing object scalar variant"))?;
        let text = match scalar(tag, child)? {
            DecodeValue::Bytes(bytes) => bytes.len(),
            _ => 0,
        };
        decoding_add(std::mem::size_of::<Self>(), text)
    }

    fn decode_input(input: DecodeInput<'_>) -> Result<Self> {
        let (tag, child) = input
            .variant(&[1, 2, 3, 4, 5, 6])?
            .ok_or_else(|| MessageError::borrowed("missing object scalar variant"))?;
        Ok(match (tag, scalar(tag, child)?) {
            (1, _) => Self::Null,
            (2, DecodeValue::Varint(value)) => Self::Bool(value != 0),
            (3, DecodeValue::Varint(value)) => Self::Signed(value as i64),
            (4, DecodeValue::Varint(value)) => Self::Unsigned(value),
            (5, DecodeValue::Bits64(value)) => Self::Float(f64::from_bits(value)),
            (6, DecodeValue::Bytes(value)) => Self::Text(
                std::str::from_utf8(value)
                    .map_err(|_| MessageError::borrowed("invalid object scalar text"))?
                    .into(),
            ),
            _ => unreachable!("validated scalar"),
        })
    }
}

impl crate::__private::PostcardValue for ObjectScalar {
    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Null => 0u32.write(bytes),
            Self::Bool(value) => {
                1u32.write(bytes);
                value.write(bytes);
            }
            Self::Signed(value) => {
                2u32.write(bytes);
                value.write(bytes);
            }
            Self::Unsigned(value) => {
                3u32.write(bytes);
                value.write(bytes);
            }
            Self::Float(value) => {
                4u32.write(bytes);
                value.write(bytes);
            }
            Self::Text(value) => {
                5u32.write(bytes);
                value.write(bytes);
            }
        }
    }

    fn size(&self) -> usize {
        1usize.saturating_add(match self {
            Self::Null => 0,
            Self::Bool(value) => value.size(),
            Self::Signed(value) => value.size(),
            Self::Unsigned(value) => value.size(),
            Self::Float(value) => value.size(),
            Self::Text(value) => value.size(),
        })
    }

    fn memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        input.nested(|input| {
            let heap = match input.value::<u32>()? {
                0 => 0,
                1 => {
                    input.value::<bool>()?;
                    0
                }
                2 => {
                    input.value::<i64>()?;
                    0
                }
                3 => {
                    input.value::<u64>()?;
                    0
                }
                4 => {
                    finite(input.value()?)?;
                    0
                }
                5 => input.value::<&str>()?.len(),
                _ => return Err(MessageError::borrowed("unknown object scalar variant")),
            };
            decoding_add(std::mem::size_of::<Self>(), heap)
        })
    }

    fn read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Self> {
        input.nested(|input| {
            Ok(match input.value::<u32>()? {
                0 => Self::Null,
                1 => Self::Bool(input.value()?),
                2 => Self::Signed(input.value()?),
                3 => Self::Unsigned(input.value()?),
                4 => Self::Float(finite(input.value()?)?),
                5 => Self::Text(<String as crate::__private::PostcardValue>::read(input)?),
                _ => return Err(MessageError::borrowed("unknown object scalar variant")),
            })
        })
    }
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(MessageError::borrowed("object scalar float must be finite"))
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn bounded_count(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
    let mut count = 0;
    input.fields(&mut |field| {
        if field.tag == tag {
            count += 1;
            if count > MAX_COLUMNS {
                return Err(MessageError::borrowed("too many object query fields"));
            }
        }
        Ok(())
    })?;
    Ok(count)
}
#[cfg(feature = "grpc")]
fn scalar(tag: u32, input: DecodeInput<'_>) -> Result<DecodeValue<'_>> {
    match tag {
        1 => Ok(DecodeValue::Varint(0)),
        2..=4 => Ok(DecodeValue::Varint(uint_field(input, 1)?.unwrap_or(0))),
        5 => {
            let mut value = DecodeValue::Bits64(0);
            input.fields(&mut |field| {
                if field.tag == 1 {
                    value = field.scalar(DecodeScalar::Bits64)?;
                }
                Ok(())
            })?;
            if matches!(value, DecodeValue::Bits64(bits) if !f64::from_bits(bits).is_finite()) {
                return Err(MessageError::borrowed("object scalar float must be finite"));
            }
            Ok(value)
        }
        6 => Ok(DecodeValue::Bytes(
            text_field(input, 1)?.unwrap_or("").as_bytes(),
        )),
        _ => unreachable!("known variant"),
    }
}

/* === Tests === */
