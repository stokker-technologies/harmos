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
use crate::schema::declaration::{Definition, Field, Kind};
use crate::schema::{Cardinality, Described};
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

pub(crate) const TIMESTAMP_DEFINITION: Definition = Timestamp::DEFINITION;
pub(crate) const VALUE_FIELDS: &[Field] = Value4::FIELDS;
pub(crate) const ENTRY_DEFINITION: Definition = Entry4::DEFINITION;

/* === Macros === */
macro_rules! shape {
    ($name:ident, $fields:expr) => {
        pub(crate) struct $name;
        impl Described for $name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($name));
            const FIELDS: &'static [Field] = $fields;
            const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
                { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
    };
}

macro_rules! level {
    ($level:ident, $entry:ident, $array:ident, $object:ident, $inner:ident) => {
        shape!(
            $array,
            &[field(
                "items",
                1,
                message($inner::DEFINITION),
                Cardinality::Repeated
            )]
        );
        shape!(
            $entry,
            &[
                field("key", 1, Kind::String, Cardinality::Required),
                field(
                    "value",
                    2,
                    message($inner::DEFINITION),
                    Cardinality::Required
                ),
            ]
        );
        shape!(
            $object,
            &[field(
                "entries",
                1,
                message($entry::DEFINITION),
                Cardinality::Repeated
            )]
        );
        shape!(
            $level,
            &[
                field("null", 1, message(<()>::DEFINITION), Cardinality::Variant),
                field("bool", 2, message(<bool>::DEFINITION), Cardinality::Variant),
                field(
                    "integer",
                    3,
                    message(<i64>::DEFINITION),
                    Cardinality::Variant
                ),
                field(
                    "unsigned",
                    4,
                    message(<u64>::DEFINITION),
                    Cardinality::Variant
                ),
                field(
                    "number",
                    5,
                    message(<f64>::DEFINITION),
                    Cardinality::Variant
                ),
                field("text", 6, message(String::DEFINITION), Cardinality::Variant),
                field(
                    "datetime",
                    7,
                    message(Timestamp::DEFINITION),
                    Cardinality::Variant
                ),
                field(
                    "array",
                    8,
                    message($array::DEFINITION),
                    Cardinality::Variant
                ),
                field(
                    "object",
                    9,
                    message($object::DEFINITION),
                    Cardinality::Variant
                ),
            ]
        );
    };
}

shape!(
    Timestamp,
    &[
        field("seconds", 1, Kind::I64, Cardinality::Required),
        field("nanos", 2, Kind::U32, Cardinality::Required),
    ]
);
shape!(
    Value0,
    &[
        field("null", 1, message(<()>::DEFINITION), Cardinality::Variant),
        field(
            "bool",
            2,
            message(<bool as Described>::DEFINITION),
            Cardinality::Variant,
        ),
        field(
            "integer",
            3,
            message(<i64 as Described>::DEFINITION),
            Cardinality::Variant,
        ),
        field(
            "unsigned",
            4,
            message(<u64 as Described>::DEFINITION),
            Cardinality::Variant,
        ),
        field(
            "number",
            5,
            message(<f64 as Described>::DEFINITION),
            Cardinality::Variant,
        ),
        field(
            "text",
            6,
            message(<String as Described>::DEFINITION),
            Cardinality::Variant,
        ),
        field(
            "datetime",
            7,
            message(Timestamp::DEFINITION),
            Cardinality::Variant,
        ),
    ]
);
level!(Value1, Entry1, Array1, Object1, Value0);
level!(Value2, Entry2, Array2, Object2, Value1);
level!(Value3, Entry3, Array3, Object3, Value2);
level!(Value4, Entry4, Array4, Object4, Value3);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Described for Value {
    const NAME: &'static str = "harmos::Value";
    const FIELDS: &'static [Field] = VALUE_FIELDS;
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
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

const fn message(definition: Definition) -> Kind {
    Kind::Message { definition }
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
