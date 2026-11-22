/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protobuf Rows Projected Into Canonical Stream Storage

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports
use prost_reflect::{DynamicMessage, MessageDescriptor, ReflectMessage};

// Local Imports
use crate::plugin::grpc::Error;
use harmos_domain::{
    message::postcard::{Reader, Value},
    schema::{
        Cardinality,
        graph::{Field, Kind, Schema},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Row {
    pub(crate) descriptor: MessageDescriptor,
    pub(crate) schema: Schema,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Row {
    pub(crate) fn encode(&self, payload: &[u8]) -> Result<DynamicMessage, Error> {
        let mut reader = Reader::new(payload);
        let message = read_message(self.descriptor.clone(), &mut reader)?;
        reader.finish().map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(message)
    }

    pub(crate) fn new(descriptor: MessageDescriptor) -> Result<Self, Error> {
        let mut messages = vec![descriptor.clone()];
        let mut indices = BTreeMap::from([(descriptor.full_name().to_owned(), 0)]);
        let mut definitions = Vec::new();
        while definitions.len() < messages.len() {
            let message = messages[definitions.len()].clone();
            let mut fields = Vec::new();
            for field in message.fields() {
                let kind = match field.kind() {
                    prost_reflect::Kind::Bool => Kind::Bool,
                    prost_reflect::Kind::Uint32 | prost_reflect::Kind::Fixed32 => Kind::U32,
                    prost_reflect::Kind::Uint64 | prost_reflect::Kind::Fixed64 => Kind::U64,
                    prost_reflect::Kind::Int32
                    | prost_reflect::Kind::Sint32
                    | prost_reflect::Kind::Sfixed32
                    | prost_reflect::Kind::Enum(_) => Kind::I32,
                    prost_reflect::Kind::Int64 | prost_reflect::Kind::Sint64 | prost_reflect::Kind::Sfixed64 => Kind::I64,
                    prost_reflect::Kind::Float => Kind::F32,
                    prost_reflect::Kind::Double => Kind::F64,
                    prost_reflect::Kind::String => Kind::String,
                    prost_reflect::Kind::Bytes => Kind::Bytes,
                    prost_reflect::Kind::Message(message) => {
                        let next = messages.len();
                        let definition = *indices.entry(message.full_name().to_owned()).or_insert_with(|| {
                            messages.push(message);
                            next
                        });
                        Kind::Message { definition }
                    }
                };
                let cardinality = if field.is_list() || field.is_map() {
                    Cardinality::Repeated
                } else if field.supports_presence() {
                    Cardinality::Optional
                } else {
                    Cardinality::Required
                };
                fields.push(Field {
                    name: field.name().to_owned().into(),
                    tag: field.number(),
                    kind,
                    cardinality,
                });
            }
            definitions.push(fields);
        }
        let schema = Schema {
            root: 0,
            definitions,
            representations: BTreeMap::new(),
        };
        schema.prepare(&Default::default()).map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Self { descriptor, schema })
    }

    pub(crate) fn decode(&self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let message = DynamicMessage::decode(self.descriptor.clone(), payload).map_err(|error| Error::Invalid(error.to_string()))?;
        let mut bytes = Vec::new();
        encode(&message, &mut bytes, 0)?;
        Ok(bytes)
    }

    /// Convert One Batch Part Whose Every Column Holds Exactly `count` Values Into Stored Rows.
    ///
    /// A part larger than one stored row may hold is stored as consecutive rows, each with
    /// its own first-sample time and count; the host slices columns, never samples.
    pub(crate) fn decode_batch(&self, payload: &[u8], count: u64, period_nanos: u64) -> Result<Vec<(i64, u64, Vec<u8>)>, Error> {
        use harmos_protocol::columns::batch;
        let invalid = || Error::Invalid("Answer Sample Window: the stored row is not a batch".into());
        let message = DynamicMessage::decode(self.descriptor.clone(), payload).map_err(|error| Error::Invalid(error.to_string()))?;
        let start = message.get_field_by_number(batch::START_UNIX_NANOS).and_then(|value| value.as_i64()).ok_or_else(invalid)?;
        let columns = message.get_field_by_number(batch::COLUMNS).ok_or_else(invalid)?;
        let prost_reflect::Value::Message(columns) = columns.as_ref() else {
            return Err(invalid());
        };
        let fields = columns.descriptor().fields().collect::<Vec<_>>();
        for field in &fields {
            let length = match columns.get_field(field).as_ref() {
                prost_reflect::Value::List(values) => values.len(),
                _ => 0,
            };
            if length as u64 != count {
                return Err(Error::Invalid(format!(
                    "Answer Sample Window: column `{}` holds {length} values for {count} samples; send one value per sample in every column",
                    field.name()
                )));
            }
        }
        let chunk = chunk(&fields);
        // A Part Within One Row's Limit Is Stored as Decoded, Without Copying Its Columns.
        if count <= chunk {
            let mut bytes = Vec::new();
            encode(&message, &mut bytes, 0)?;
            return Ok(vec![(start, count, bytes)]);
        }
        let mut rows = Vec::new();
        let mut offset = 0_u64;
        while offset < count {
            let length = chunk.min(count - offset);
            let first = i64::try_from(i128::from(start) + i128::from(offset) * i128::from(period_nanos))
                .map_err(|_| Error::Invalid("Grant Sample Window: sample times must fall before 2262".into()))?;
            let range = offset as usize..(offset + length) as usize;
            let mut part = DynamicMessage::new(columns.descriptor());
            for field in &fields {
                if let prost_reflect::Value::List(values) = columns.get_field(field).as_ref() {
                    part.set_field(field, prost_reflect::Value::List(values[range.clone()].to_vec()));
                }
            }
            let mut stored = DynamicMessage::new(self.descriptor.clone());
            stored.set_field_by_number(batch::START_UNIX_NANOS, prost_reflect::Value::I64(first));
            stored.set_field_by_number(batch::PERIOD_NANOS, prost_reflect::Value::U64(period_nanos));
            stored.set_field_by_number(batch::COUNT, prost_reflect::Value::U64(length));
            stored.set_field_by_number(batch::COLUMNS, prost_reflect::Value::Message(part));
            let mut bytes = Vec::new();
            encode(&stored, &mut bytes, 0)?;
            rows.push((first, length, bytes));
            offset += length;
        }
        Ok(rows)
    }

    /// The Fewest Stored Rows and Payload Bytes One Window of `count` Samples Takes, Every Value at Its Smallest.
    pub(crate) fn least(&self, count: u64) -> (u64, u128) {
        use harmos_protocol::columns::batch;
        let Some(prost_reflect::Kind::Message(columns)) = self.descriptor.get_field(batch::COLUMNS).map(|field| field.kind()) else {
            return (count, u128::from(count));
        };
        let fields = columns.fields().collect::<Vec<_>>();
        let sample = fields.iter().map(|field| smallest(&field.kind(), 0)).sum::<usize>().max(1);
        (count.div_ceil(chunk(&fields)), u128::from(count) * sample as u128)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// Samples One Stored Row Holds: Each Claims Its Values in Every Column; the Header and List Lengths Claim the Rest.
fn chunk(fields: &[prost_reflect::FieldDescriptor]) -> u64 {
    let per_sample = fields.iter().map(|field| 1 + cost(&field.kind(), 0)).sum::<usize>().max(1);
    ((harmos_domain::message::postcard::MAX_VALUES - 64) / per_sample).max(1) as u64
}

/// The Fewest Stored Bytes One Value of `kind` Encodes To: Floats Are Fixed, Everything Else at Least One Byte.
fn smallest(kind: &prost_reflect::Kind, depth: usize) -> usize {
    match kind {
        prost_reflect::Kind::Double => 8,
        prost_reflect::Kind::Float => 4,
        prost_reflect::Kind::Message(message) if depth < 8 => {
            message.fields().map(|field| smallest(&field.kind(), depth + 1)).sum::<usize>()
        }
        _ => 1,
    }
}

/// Structural Values One Element of `kind` Claims When Stored: Presence, Nested Fields and List Lengths Included.
fn cost(kind: &prost_reflect::Kind, depth: usize) -> usize {
    let prost_reflect::Kind::Message(message) = kind else {
        return 1;
    };
    if depth >= 8 {
        return 64;
    }
    message.fields().map(|field| 2 + cost(&field.kind(), depth + 1)).sum::<usize>() + 1
}

fn encode(message: &DynamicMessage, bytes: &mut Vec<u8>, depth: usize) -> Result<(), Error> {
    if depth >= harmos_domain::schema::MAX_DEPTH {
        return Err(Error::Invalid("Stream Row Nesting Exceeds the Structural Limit".into()));
    }
    for field in message.descriptor().fields() {
        let value = message.get_field(&field);
        if field.is_list() {
            let prost_reflect::Value::List(values) = value.as_ref() else { unreachable!() };
            values.len().write(bytes);
            for value in values {
                encode_value(value, bytes, depth)?;
            }
        } else if field.is_map() {
            let prost_reflect::Value::Map(values) = value.as_ref() else { unreachable!() };
            values.len().write(bytes);
            let mut entries: Vec<_> = values.iter().collect();
            entries.sort_by_key(|(key, _)| format!("{key:?}"));
            let prost_reflect::Kind::Message(entry) = field.kind() else { unreachable!() };
            let optional = entry.get_field_by_name("value").unwrap().supports_presence();
            for (key, value) in entries {
                match key {
                    prost_reflect::MapKey::Bool(value) => value.write(bytes),
                    prost_reflect::MapKey::I32(value) => value.write(bytes),
                    prost_reflect::MapKey::I64(value) => value.write(bytes),
                    prost_reflect::MapKey::U32(value) => value.write(bytes),
                    prost_reflect::MapKey::U64(value) => value.write(bytes),
                    prost_reflect::MapKey::String(value) => value.write(bytes),
                }
                if optional {
                    1_u8.write(bytes);
                }
                encode_value(value, bytes, depth + 1)?;
            }
        } else {
            if field.supports_presence() {
                let present = message.has_field(&field);
                u8::from(present).write(bytes);
                if !present {
                    continue;
                }
            }
            encode_value(value.as_ref(), bytes, depth)?;
        }
    }
    Ok(())
}

fn encode_value(value: &prost_reflect::Value, bytes: &mut Vec<u8>, depth: usize) -> Result<(), Error> {
    match value {
        prost_reflect::Value::Bool(value) => value.write(bytes),
        prost_reflect::Value::I32(value) | prost_reflect::Value::EnumNumber(value) => value.write(bytes),
        prost_reflect::Value::I64(value) => value.write(bytes),
        prost_reflect::Value::U32(value) => value.write(bytes),
        prost_reflect::Value::U64(value) => value.write(bytes),
        prost_reflect::Value::F32(value) => value.write(bytes),
        prost_reflect::Value::F64(value) => value.write(bytes),
        prost_reflect::Value::String(value) => value.write(bytes),
        prost_reflect::Value::Bytes(value) => value.to_vec().write(bytes),
        prost_reflect::Value::Message(message) => encode(message, bytes, depth + 1)?,
        prost_reflect::Value::List(_) | prost_reflect::Value::Map(_) => unreachable!("collection fields are encoded by their descriptor"),
    }
    Ok(())
}

fn read_message(descriptor: MessageDescriptor, reader: &mut Reader<'_>) -> Result<DynamicMessage, Error> {
    let mut message = DynamicMessage::new(descriptor.clone());
    for field in descriptor.fields() {
        if field.is_map() {
            let prost_reflect::Kind::Message(entry) = field.kind() else { unreachable!() };
            let key = entry.get_field(1).expect("map key");
            let value = entry.get_field(2).expect("map value");
            let count = usize::read(reader).map_err(|error| Error::Invalid(error.to_string()))?;
            let mut entries = std::collections::HashMap::new();
            for _ in 0..count {
                let key = match read_value(key.kind(), reader)? {
                    prost_reflect::Value::Bool(value) => prost_reflect::MapKey::Bool(value),
                    prost_reflect::Value::I32(value) => prost_reflect::MapKey::I32(value),
                    prost_reflect::Value::I64(value) => prost_reflect::MapKey::I64(value),
                    prost_reflect::Value::U32(value) => prost_reflect::MapKey::U32(value),
                    prost_reflect::Value::U64(value) => prost_reflect::MapKey::U64(value),
                    prost_reflect::Value::String(value) => prost_reflect::MapKey::String(value),
                    _ => unreachable!("protobuf map key"),
                };
                if value.supports_presence() {
                    let present = u8::read(reader).map_err(|error| Error::Invalid(error.to_string()))?;
                    if present != 1 {
                        return Err(Error::Invalid("Read Stream Row: map values must be present".into()));
                    }
                }
                entries.insert(key, read_value(value.kind(), reader)?);
            }
            message.set_field(&field, prost_reflect::Value::Map(entries));
            continue;
        }
        if field.is_list() {
            let count = usize::read(reader).map_err(|error| Error::Invalid(error.to_string()))?;
            let values = (0..count).map(|_| read_value(field.kind(), reader)).collect::<Result<Vec<_>, _>>()?;
            message.set_field(&field, prost_reflect::Value::List(values));
            continue;
        }
        if field.supports_presence() {
            let present = u8::read(reader).map_err(|error| Error::Invalid(error.to_string()))?;
            if present == 0 {
                continue;
            }
            if present != 1 {
                return Err(Error::Invalid("Read Stream Row: invalid presence tag".into()));
            }
        }
        message.set_field(&field, read_value(field.kind(), reader)?);
    }
    Ok(message)
}

fn read_value(kind: prost_reflect::Kind, reader: &mut Reader<'_>) -> Result<prost_reflect::Value, Error> {
    use prost_reflect::Kind;
    let value = match kind {
        Kind::Bool => bool::read(reader).map(prost_reflect::Value::Bool),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => i32::read(reader).map(prost_reflect::Value::I32),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => i64::read(reader).map(prost_reflect::Value::I64),
        Kind::Uint32 | Kind::Fixed32 => u32::read(reader).map(prost_reflect::Value::U32),
        Kind::Uint64 | Kind::Fixed64 => u64::read(reader).map(prost_reflect::Value::U64),
        Kind::Float => f32::read(reader).map(prost_reflect::Value::F32),
        Kind::Double => f64::read(reader).map(prost_reflect::Value::F64),
        Kind::String => String::read(reader).map(prost_reflect::Value::String),
        Kind::Bytes => Vec::<u8>::read(reader).map(|value| prost_reflect::Value::Bytes(value.into())),
        Kind::Enum(_) => i32::read(reader).map(prost_reflect::Value::EnumNumber),
        Kind::Message(descriptor) => return read_message(descriptor, reader).map(prost_reflect::Value::Message),
    };
    value.map_err(|error| Error::Invalid(error.to_string()))
}

/* === Tests === */
