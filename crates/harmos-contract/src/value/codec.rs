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
use std::collections::BTreeMap;
// Third Party Imports
use time::OffsetDateTime;
// Local Imports
use super::Value;
use crate::message::{PostcardReader as Reader, PostcardValue, decoding_add, decoding_multiply};
use crate::schema::{Cardinality, Described, Field, Kind};
use crate::{MessageError, btree_map_memory};

/* === Constants === */
pub const VALUE_DEPTH: usize = 4;

// Postcard Variant Ordinals Are Part Of The Value Contract
const NULL: u32 = 0;
const BOOL: u32 = 1;
const INTEGER: u32 = 2;
const UNSIGNED: u32 = 3;
const NUMBER: u32 = 4;
const TEXT: u32 = 5;
const DATETIME: u32 = 6;
const ARRAY: u32 = 7;
const OBJECT: u32 = 8;

pub(crate) const TIMESTAMP_FIELDS: &[Field] = &[
    field("seconds", 1, Kind::I64, Cardinality::Required),
    field("nanos", 2, Kind::U32, Cardinality::Required),
];

pub(crate) const VALUE_FIELDS: &[Field] = LEVEL_4;
pub(crate) const ENTRY_FIELDS: &[Field] = ENTRY_4;

/* === Macros === */
macro_rules! level {
    ($level:ident, $entry:ident, $array:ident, $object:ident, $inner:ident) => {
        const $array: &[Field] = &[field("items", 1, message($inner), Cardinality::Repeated)];
        const $entry: &[Field] = &[
            field("key", 1, Kind::String, Cardinality::Required),
            field("value", 2, message($inner), Cardinality::Required),
        ];
        const $object: &[Field] = &[field("entries", 1, message($entry), Cardinality::Repeated)];
        const $level: &[Field] = &[
            field("null", 1, message(&[]), Cardinality::Variant),
            field(
                "bool",
                2,
                message(<bool as Described>::FIELDS),
                Cardinality::Variant,
            ),
            field(
                "integer",
                3,
                message(<i64 as Described>::FIELDS),
                Cardinality::Variant,
            ),
            field(
                "unsigned",
                4,
                message(<u64 as Described>::FIELDS),
                Cardinality::Variant,
            ),
            field(
                "number",
                5,
                message(<f64 as Described>::FIELDS),
                Cardinality::Variant,
            ),
            field(
                "text",
                6,
                message(<String as Described>::FIELDS),
                Cardinality::Variant,
            ),
            field(
                "datetime",
                7,
                message(TIMESTAMP_FIELDS),
                Cardinality::Variant,
            ),
            field("array", 8, message($array), Cardinality::Variant),
            field("object", 9, message($object), Cardinality::Variant),
        ];
    };
}

const LEVEL_0: &[Field] = &[
    field("null", 1, message(&[]), Cardinality::Variant),
    field(
        "bool",
        2,
        message(<bool as Described>::FIELDS),
        Cardinality::Variant,
    ),
    field(
        "integer",
        3,
        message(<i64 as Described>::FIELDS),
        Cardinality::Variant,
    ),
    field(
        "unsigned",
        4,
        message(<u64 as Described>::FIELDS),
        Cardinality::Variant,
    ),
    field(
        "number",
        5,
        message(<f64 as Described>::FIELDS),
        Cardinality::Variant,
    ),
    field(
        "text",
        6,
        message(<String as Described>::FIELDS),
        Cardinality::Variant,
    ),
    field(
        "datetime",
        7,
        message(TIMESTAMP_FIELDS),
        Cardinality::Variant,
    ),
];
level!(LEVEL_1, ENTRY_1, ARRAY_1, OBJECT_1, LEVEL_0);
level!(LEVEL_2, ENTRY_2, ARRAY_2, OBJECT_2, LEVEL_1);
level!(LEVEL_3, ENTRY_3, ARRAY_3, OBJECT_3, LEVEL_2);
level!(LEVEL_4, ENTRY_4, ARRAY_4, OBJECT_4, LEVEL_3);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Described for Value {
    const FIELDS: &'static [Field] = VALUE_FIELDS;
}

impl PostcardValue for Value {
    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Null => NULL.write(bytes),
            Self::Bool(value) => {
                BOOL.write(bytes);
                value.write(bytes);
            }
            Self::Integer(value) => {
                INTEGER.write(bytes);
                value.write(bytes);
            }
            Self::Unsigned(value) => {
                UNSIGNED.write(bytes);
                value.write(bytes);
            }
            Self::Number(value) => {
                NUMBER.write(bytes);
                value.write(bytes);
            }
            Self::Text(value) => {
                TEXT.write(bytes);
                value.write(bytes);
            }
            Self::Datetime(value) => {
                DATETIME.write(bytes);
                write_timestamp(*value, bytes);
            }
            Self::Array(values) => {
                ARRAY.write(bytes);
                values.len().write(bytes);
                for value in values {
                    value.write(bytes);
                }
            }
            Self::Object(entries) => {
                OBJECT.write(bytes);
                write_entries(entries, bytes);
            }
        }
    }

    fn size(&self) -> usize {
        let payload = match self {
            Self::Null => 0,
            Self::Bool(value) => value.size(),
            Self::Integer(value) => value.size(),
            Self::Unsigned(value) => value.size(),
            Self::Number(value) => value.size(),
            Self::Text(value) => value.size(),
            Self::Datetime(value) => timestamp_size(*value),
            Self::Array(values) => values.iter().fold(values.len().size(), |size, value| {
                size.saturating_add(value.size())
            }),
            Self::Object(entries) => entries_size(entries),
        };
        variant(self).size().saturating_add(payload)
    }

    fn memory(input: &mut Reader<'_>) -> Result<usize, MessageError> {
        memory_at(input, VALUE_DEPTH)
    }

    fn read(input: &mut Reader<'_>) -> Result<Self, MessageError> {
        read_at(input, VALUE_DEPTH)
    }
}

crate::with_protobuf! {
    impl crate::Protobuf for Value {
        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(protobuf::body_len(self));
            protobuf::encode(self, &mut bytes);
            bytes
        }
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            protobuf::encode(self, bytes);
        }
        fn encoded_len(&self) -> usize {
            protobuf::body_len(self)
        }
        fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
            protobuf::measure(crate::message::DecodeInput::new(bytes), VALUE_DEPTH)
        }
        fn decoding_memory_input(
            input: crate::message::DecodeInput<'_>,
        ) -> Result<usize, MessageError> {
            protobuf::measure(input, VALUE_DEPTH)
        }
        fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
            Self::decode_input(crate::message::DecodeInput::new(bytes))
        }
        fn decode_input(input: crate::message::DecodeInput<'_>) -> Result<Self, MessageError> {
            protobuf::measure(input, VALUE_DEPTH)?;
            protobuf::read(input, VALUE_DEPTH)
        }
    }

        pub(crate) mod protobuf {
        use std::collections::BTreeMap;

        use prost::encoding::{self, WireType};

        use super::{Value, depth_error, timestamp_parts, from_parts};
        use crate::btree_map_memory;
        use crate::message::{DecodeField, DecodeInput, DecodeScalar, DecodeValue, decoding_add};
        use crate::MessageError;

        pub(crate) fn encode(value: &Value, bytes: &mut Vec<u8>) {
            match value {
                Value::Null => message(1, 0, bytes),
                Value::Bool(value) => {
                    message(2, encoding::bool::encoded_len(1, value), bytes);
                    encoding::bool::encode(1, value, bytes);
                }
                Value::Integer(value) => {
                    message(3, encoding::int64::encoded_len(1, value), bytes);
                    encoding::int64::encode(1, value, bytes);
                }
                Value::Unsigned(value) => {
                    message(4, encoding::uint64::encoded_len(1, value), bytes);
                    encoding::uint64::encode(1, value, bytes);
                }
                Value::Number(value) => {
                    message(5, encoding::double::encoded_len(1, value), bytes);
                    encoding::double::encode(1, value, bytes);
                }
                Value::Text(value) => {
                    message(6, encoding::string::encoded_len(1, value), bytes);
                    encoding::string::encode(1, value, bytes);
                }
                Value::Datetime(value) => {
                    message(7, timestamp_len(*value), bytes);
                    encode_timestamp(*value, bytes);
                }
                Value::Array(values) => {
                    message(8, items_len(values), bytes);
                    for item in values {
                        message(1, body_len(item), bytes);
                        encode(item, bytes);
                    }
                }
                Value::Object(entries) => {
                    message(9, entries_len(entries), bytes);
                    encode_entries(entries, bytes);
                }
            }
        }

        pub(crate) fn body_len(value: &Value) -> usize {
            match value {
                Value::Null => framed(1, 0),
                Value::Bool(value) => framed(2, encoding::bool::encoded_len(1, value)),
                Value::Integer(value) => framed(3, encoding::int64::encoded_len(1, value)),
                Value::Unsigned(value) => framed(4, encoding::uint64::encoded_len(1, value)),
                Value::Number(value) => framed(5, encoding::double::encoded_len(1, value)),
                Value::Text(value) => framed(6, encoding::string::encoded_len(1, value)),
                Value::Datetime(value) => framed(7, timestamp_len(*value)),
                Value::Array(values) => framed(8, items_len(values)),
                Value::Object(entries) => framed(9, entries_len(entries)),
            }
        }

                pub(crate) fn encode_entries(entries: &BTreeMap<String, Value>, bytes: &mut Vec<u8>) {
            encode_entries_at(1, entries, bytes);
        }

        pub(crate) fn encode_entries_at(
            tag: u32,
            entries: &BTreeMap<String, Value>,
            bytes: &mut Vec<u8>,
        ) {
            for (key, value) in entries {
                message(tag, entry_len(key, value), bytes);
                encoding::string::encode(1, key, bytes);
                message(2, body_len(value), bytes);
                encode(value, bytes);
            }
        }

        pub(crate) fn entries_len(entries: &BTreeMap<String, Value>) -> usize {
            entries_len_at(1, entries)
        }

        pub(crate) fn entries_len_at(tag: u32, entries: &BTreeMap<String, Value>) -> usize {
            entries.iter().fold(0usize, |total, (key, value)| {
                total.saturating_add(framed(tag, entry_len(key, value)))
            })
        }

        pub(crate) fn encode_timestamp(value: time::OffsetDateTime, bytes: &mut Vec<u8>) {
            let (seconds, nanos) = timestamp_parts(value);
            if seconds != 0 {
                encoding::int64::encode(1, &seconds, bytes);
            }
            if nanos != 0 {
                encoding::uint32::encode(2, &nanos, bytes);
            }
        }

        pub(crate) fn timestamp_len(value: time::OffsetDateTime) -> usize {
            let (seconds, nanos) = timestamp_parts(value);
            let seconds = if seconds == 0 { 0 } else { encoding::int64::encoded_len(1, &seconds) };
            let nanos = if nanos == 0 { 0 } else { encoding::uint32::encoded_len(2, &nanos) };
            seconds + nanos
        }

        pub(crate) fn message(tag: u32, length: usize, bytes: &mut Vec<u8>) {
            encoding::encode_key(tag, WireType::LengthDelimited, bytes);
            encoding::encode_varint(length as u64, bytes);
        }

        pub(crate) fn framed(tag: u32, length: usize) -> usize {
            encoding::key_len(tag)
                .saturating_add(encoding::encoded_len_varint(length as u64))
                .saturating_add(length)
        }

        fn entry_len(key: &str, value: &Value) -> usize {
            encoding::key_len(1)
                .saturating_add(encoding::encoded_len_varint(key.len() as u64))
                .saturating_add(key.len())
                .saturating_add(framed(2, body_len(value)))
        }

        fn items_len(values: &[Value]) -> usize {
            values
                .iter()
                .fold(0usize, |total, item| total.saturating_add(framed(1, body_len(item))))
        }

                fn selected<'a>(input: DecodeInput<'a>) -> Result<Option<DecodeField<'a>>, MessageError> {
            let mut selected = None;
            input.fields(&mut |field| {
                if (1..=9).contains(&field.tag) {
                    selected = Some(field);
                }
                Ok(())
            })?;
            Ok(selected)
        }

                pub(crate) fn measure(input: DecodeInput<'_>, depth: usize) -> Result<usize, MessageError> {
            let owned = std::mem::size_of::<Value>();
            let Some(field) = selected(input)? else {
                return Ok(owned);
            };
            let nested = match field.tag {
                1 => {
                    field.bytes()?;
                    0
                }
                2..=5 => {
                    wrapped(input.child(field.bytes()?)?, field.tag)?;
                    0
                }
                6 => wrapped(input.child(field.bytes()?)?, field.tag)?.map_or(0, |text| text.len()),
                7 => {
                    timestamp(input.child(field.bytes()?)?)?;
                    0
                }
                8 => {
                    let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                    let array = input.child(field.bytes()?)?;
                    // Each Item Charges Its Inline Vector Slot And Its Allocations
                    let mut items = 0usize;
                    array.fields(&mut |item| {
                        if item.tag == 1 {
                            items = decoding_add(items, measure(array.child(item.bytes()?)?, depth)?)?;
                        }
                        Ok(())
                    })?;
                    items
                }
                9 => {
                    let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                    let object = input.child(field.bytes()?)?;
                    measure_entries(object, depth)?
                }
                _ => unreachable!("oneof selection is bounded"),
            };
            decoding_add(owned, nested)
        }

                pub(crate) fn measure_entries(input: DecodeInput<'_>, depth: usize) -> Result<usize, MessageError> {
            measure_entries_at(input, 1, depth)
        }

        pub(crate) fn measure_entries_at(
            input: DecodeInput<'_>,
            tag: u32,
            depth: usize,
        ) -> Result<usize, MessageError> {
            let mut count = 0usize;
            let mut owned = 0usize;
            input.fields(&mut |field| {
                if field.tag != tag {
                    return Ok(());
                }
                count = decoding_add(count, 1)?;
                let entry = input.child(field.bytes()?)?;
                let mut key = 0usize;
                let mut value = None;
                entry.fields(&mut |part| {
                    match part.tag {
                        1 => key = part.text()?.len(),
                        2 => value = Some(part.bytes()?),
                        _ => {}
                    }
                    Ok(())
                })?;
                let value = match value {
                    Some(bytes) => measure(entry.child(bytes)?, depth)?,
                    None => std::mem::size_of::<Value>(),
                };
                owned = decoding_add(owned, decoding_add(key, value)?)?;
                Ok(())
            })?;
            decoding_add(owned, btree_map_memory::<String, Value>(count))
        }

        pub(crate) fn read(input: DecodeInput<'_>, depth: usize) -> Result<Value, MessageError> {
            let Some(field) = selected(input)? else {
                return Ok(Value::Null);
            };
            Ok(match (field.tag, field.value) {
                (1, _) => {
                    field.bytes()?;
                    Value::Null
                }
                (2..=6, _) => scalar(input.child(field.bytes()?)?, field.tag)?,
                (7, _) => Value::Datetime(timestamp(input.child(field.bytes()?)?)?),
                (8, _) => {
                    let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                    let array = input.child(field.bytes()?)?;
                    let mut items = Vec::new();
                    array.fields(&mut |item| {
                        if item.tag == 1 {
                            items.push(read(array.child(item.bytes()?)?, depth)?);
                        }
                        Ok(())
                    })?;
                    // Keep The Owned Allocation Within The Measured Item Count
                    items.shrink_to_fit();
                    Value::Array(items)
                }
                (9, _) => {
                    let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                    Value::Object(read_entries(input.child(field.bytes()?)?, depth)?)
                }
                _ => unreachable!("oneof selection is bounded"),
            })
        }

        pub(crate) fn read_entries(
            input: DecodeInput<'_>,
            depth: usize,
        ) -> Result<BTreeMap<String, Value>, MessageError> {
            read_entries_at(input, 1, depth)
        }

        pub(crate) fn read_entries_at(
            input: DecodeInput<'_>,
            tag: u32,
            depth: usize,
        ) -> Result<BTreeMap<String, Value>, MessageError> {
            let mut entries = BTreeMap::new();
            input.fields(&mut |field| {
                if field.tag != tag {
                    return Ok(());
                }
                let entry = input.child(field.bytes()?)?;
                let mut key = String::new();
                let mut value = Value::Null;
                entry.fields(&mut |part| {
                    match part.tag {
                        1 => key = part.text()?.to_owned(),
                        2 => value = read(entry.child(part.bytes()?)?, depth)?,
                        _ => {}
                    }
                    Ok(())
                })?;
                entries.insert(key, value);
                Ok(())
            })?;
            Ok(entries)
        }

        pub(crate) fn timestamp(input: DecodeInput<'_>) -> Result<time::OffsetDateTime, MessageError> {
            let mut seconds = 0i64;
            let mut nanos = 0u64;
            input.fields(&mut |field| {
                match field.tag {
                    1 => seconds = varint(field)? as i64,
                    2 => nanos = varint(field)?,
                    _ => {}
                }
                Ok(())
            })?;
            from_parts(seconds, nanos)
        }

                fn wrapped<'a>(input: DecodeInput<'a>, tag: u32) -> Result<Option<&'a str>, MessageError> {
            let mut text = None;
            input.fields(&mut |field| {
                if field.tag != 1 {
                    return Ok(());
                }
                match tag {
                    2..=4 => {
                        varint(field)?;
                    }
                    5 => {
                        field.scalar(DecodeScalar::Bits64)?;
                    }
                    _ => text = Some(field.text()?),
                }
                Ok(())
            })?;
            Ok(text)
        }

        fn scalar(input: DecodeInput<'_>, tag: u32) -> Result<Value, MessageError> {
            let mut value = match tag {
                2 => Value::Bool(false),
                3 => Value::Integer(0),
                4 => Value::Unsigned(0),
                5 => Value::Number(0.0),
                _ => Value::Text(String::new()),
            };
            input.fields(&mut |field| {
                if field.tag != 1 {
                    return Ok(());
                }
                value = match tag {
                    2 => Value::Bool(varint(field)? != 0),
                    3 => Value::Integer(varint(field)? as i64),
                    4 => Value::Unsigned(varint(field)?),
                    5 => match field.scalar(DecodeScalar::Bits64)? {
                        DecodeValue::Bits64(bits) => Value::Number(f64::from_bits(bits)),
                        _ => unreachable!("validated fixed-width scalar"),
                    },
                    _ => Value::Text(field.text()?.to_owned()),
                };
                Ok(())
            })?;
            Ok(value)
        }

        fn varint(field: DecodeField<'_>) -> Result<u64, MessageError> {
            match field.scalar(DecodeScalar::Varint)? {
                DecodeValue::Varint(value) => Ok(value),
                _ => unreachable!("validated varint scalar"),
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
const fn field(name: &'static str, tag: u32, kind: Kind, cardinality: Cardinality) -> Field {
    Field {
        name: Cow::Borrowed(name),
        tag,
        kind,
        cardinality,
    }
}

const fn message(fields: &'static [Field]) -> Kind {
    Kind::Message {
        fields: Cow::Borrowed(fields),
    }
}

fn variant(value: &Value) -> u32 {
    match value {
        Value::Null => NULL,
        Value::Bool(_) => BOOL,
        Value::Integer(_) => INTEGER,
        Value::Unsigned(_) => UNSIGNED,
        Value::Number(_) => NUMBER,
        Value::Text(_) => TEXT,
        Value::Datetime(_) => DATETIME,
        Value::Array(_) => ARRAY,
        Value::Object(_) => OBJECT,
    }
}

fn depth_error() -> MessageError {
    MessageError::borrowed("value nests deeper than its portable depth")
}

pub(crate) fn timestamp_parts(value: OffsetDateTime) -> (i64, u32) {
    (value.unix_timestamp(), value.nanosecond())
}

pub(crate) fn from_parts(seconds: i64, nanos: u64) -> Result<OffsetDateTime, MessageError> {
    let nanos = u32::try_from(nanos)
        .ok()
        .filter(|nanos| *nanos < 1_000_000_000)
        .ok_or_else(|| MessageError::borrowed("timestamp nanoseconds out of range"))?;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|value| value.replace_nanosecond(nanos).ok())
        .ok_or_else(|| MessageError::borrowed("timestamp out of range"))
}

pub(crate) fn write_timestamp(value: OffsetDateTime, bytes: &mut Vec<u8>) {
    let (seconds, nanos) = timestamp_parts(value);
    seconds.write(bytes);
    nanos.write(bytes);
}

pub(crate) fn timestamp_size(value: OffsetDateTime) -> usize {
    let (seconds, nanos) = timestamp_parts(value);
    seconds.size().saturating_add(nanos.size())
}

pub(crate) fn read_timestamp(input: &mut Reader<'_>) -> Result<OffsetDateTime, MessageError> {
    let seconds = i64::read(input)?;
    let nanos = u32::read(input)?;
    from_parts(seconds, u64::from(nanos))
}

pub(crate) fn write_entries(entries: &BTreeMap<String, Value>, bytes: &mut Vec<u8>) {
    entries.len().write(bytes);
    for (key, value) in entries {
        key.write(bytes);
        value.write(bytes);
    }
}

pub(crate) fn entries_size(entries: &BTreeMap<String, Value>) -> usize {
    entries
        .iter()
        .fold(entries.len().size(), |size, (key, value)| {
            size.saturating_add(key.size()).saturating_add(value.size())
        })
}

pub(crate) fn entries_memory(input: &mut Reader<'_>, depth: usize) -> Result<usize, MessageError> {
    input.nested(|input| {
        let count: usize = input.value()?;
        input.claim(count)?;
        let mut owned = btree_map_memory::<String, Value>(count);
        for _ in 0..count {
            let key = String::memory(input)?.saturating_sub(std::mem::size_of::<String>());
            let value = memory_at(input, depth)?.saturating_sub(std::mem::size_of::<Value>());
            owned = decoding_add(owned, decoding_add(key, value)?)?;
        }
        Ok(owned)
    })
}

pub(crate) fn read_entries(
    input: &mut Reader<'_>,
    depth: usize,
) -> Result<BTreeMap<String, Value>, MessageError> {
    input.nested(|input| {
        let count: usize = input.value()?;
        input.claim(count)?;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let key = String::read(input)?;
            let value = read_at(input, depth)?;
            if entries.insert(key, value).is_some() {
                return Err(MessageError::borrowed("duplicate value object key"));
            }
        }
        Ok(entries)
    })
}

fn memory_at(input: &mut Reader<'_>, depth: usize) -> Result<usize, MessageError> {
    input.nested(|input| {
        let owned = std::mem::size_of::<Value>();
        let allocation = match u32::read(input)? {
            NULL => 0,
            BOOL => {
                bool::read(input)?;
                0
            }
            INTEGER => {
                i64::read(input)?;
                0
            }
            UNSIGNED => {
                u64::read(input)?;
                0
            }
            NUMBER => {
                f64::read(input)?;
                0
            }
            TEXT => String::memory(input)?.saturating_sub(std::mem::size_of::<String>()),
            DATETIME => {
                read_timestamp(input)?;
                0
            }
            ARRAY => {
                let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                let count: usize = input.value()?;
                input.claim(count)?;
                let mut items = decoding_multiply(count, std::mem::size_of::<Value>())?;
                for _ in 0..count {
                    let item = memory_at(input, depth)?;
                    items = decoding_add(items, item.saturating_sub(std::mem::size_of::<Value>()))?;
                }
                items
            }
            OBJECT => {
                let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                entries_memory(input, depth)?
            }
            _ => return Err(MessageError::borrowed("unknown value variant")),
        };
        decoding_add(owned, allocation)
    })
}

fn read_at(input: &mut Reader<'_>, depth: usize) -> Result<Value, MessageError> {
    input.nested(|input| {
        Ok(match u32::read(input)? {
            NULL => Value::Null,
            BOOL => Value::Bool(bool::read(input)?),
            INTEGER => Value::Integer(i64::read(input)?),
            UNSIGNED => Value::Unsigned(u64::read(input)?),
            NUMBER => Value::Number(f64::read(input)?),
            TEXT => Value::Text(String::read(input)?),
            DATETIME => Value::Datetime(read_timestamp(input)?),
            ARRAY => {
                let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                let count: usize = input.value()?;
                input.claim(count)?;
                let mut items = crate::message::decoding_vector(count)?;
                for _ in 0..count {
                    items.push(read_at(input, depth)?);
                }
                Value::Array(items)
            }
            OBJECT => {
                let depth = depth.checked_sub(1).ok_or_else(depth_error)?;
                Value::Object(read_entries(input, depth)?)
            }
            _ => return Err(MessageError::borrowed("unknown value variant")),
        })
    })
}

/* === Tests === */
