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
use prost_reflect::{DynamicMessage, Kind as ProtoKind, MessageDescriptor, ReflectMessage, Value};

// Local Imports
use crate::plugin::grpc::Error;
use harmos_domain::{
    message::postcard::Value as _,
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
    pub(crate) fn new(descriptor: MessageDescriptor) -> Result<Self, Error> {
        let mut messages = vec![descriptor.clone()];
        let mut indices = BTreeMap::from([(descriptor.full_name().to_owned(), 0)]);
        let mut definitions = Vec::new();
        while definitions.len() < messages.len() {
            let message = messages[definitions.len()].clone();
            let mut fields = Vec::new();
            for field in message.fields() {
                let kind = match field.kind() {
                    ProtoKind::Bool => Kind::Bool,
                    ProtoKind::Uint32 | ProtoKind::Fixed32 => Kind::U32,
                    ProtoKind::Uint64 | ProtoKind::Fixed64 => Kind::U64,
                    ProtoKind::Int32 | ProtoKind::Sint32 | ProtoKind::Sfixed32 | ProtoKind::Enum(_) => Kind::I32,
                    ProtoKind::Int64 | ProtoKind::Sint64 | ProtoKind::Sfixed64 => Kind::I64,
                    ProtoKind::Float => Kind::F32,
                    ProtoKind::Double => Kind::F64,
                    ProtoKind::String => Kind::String,
                    ProtoKind::Bytes => Kind::Bytes,
                    ProtoKind::Message(message) => {
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
        let Value::Message(columns) = columns.as_ref() else {
            return Err(invalid());
        };
        let fields = columns.descriptor().fields().collect::<Vec<_>>();
        let mut lists = Vec::with_capacity(fields.len());
        for field in &fields {
            let values = match columns.get_field(field).into_owned() {
                Value::List(values) => values,
                _ => Vec::new(),
            };
            if values.len() as u64 != count {
                return Err(Error::Invalid(format!(
                    "Answer Sample Window: column `{}` holds {} values for {count} samples; send one value per sample in every column",
                    field.name(),
                    values.len()
                )));
            }
            lists.push(values);
        }
        // Each Sample Claims Its Values in Every Column; the Header and List Lengths Claim the Rest.
        let per_sample = fields.iter().map(|field| 1 + cost(&field.kind(), 0)).sum::<usize>().max(1);
        let chunk = ((harmos_domain::message::postcard::MAX_VALUES - 64) / per_sample).max(1) as u64;
        let mut rows = Vec::new();
        let mut offset = 0_u64;
        while offset < count {
            let length = chunk.min(count - offset);
            let first = i64::try_from(i128::from(start) + i128::from(offset) * i128::from(period_nanos))
                .map_err(|_| Error::Invalid("Grant Sample Window: sample times must fall before 2262".into()))?;
            let mut part = DynamicMessage::new(columns.descriptor());
            for (field, values) in fields.iter().zip(&lists) {
                part.set_field(field, Value::List(values[offset as usize..(offset + length) as usize].to_vec()));
            }
            let mut stored = message.clone();
            stored.set_field_by_number(batch::START_UNIX_NANOS, Value::I64(first));
            stored.set_field_by_number(batch::COUNT, Value::U64(length));
            stored.set_field_by_number(batch::COLUMNS, Value::Message(part));
            let mut bytes = Vec::new();
            encode(&stored, &mut bytes, 0)?;
            rows.push((first, length, bytes));
            offset += length;
        }
        Ok(rows)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// Structural Values One Element of `kind` Claims When Stored: Presence, Nested Fields and List Lengths Included.
fn cost(kind: &ProtoKind, depth: usize) -> usize {
    let ProtoKind::Message(message) = kind else {
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
            let Value::List(values) = value.as_ref() else { unreachable!() };
            values.len().write(bytes);
            for value in values {
                encode_value(value, bytes, depth)?;
            }
        } else if field.is_map() {
            let Value::Map(values) = value.as_ref() else { unreachable!() };
            values.len().write(bytes);
            let mut entries: Vec<_> = values.iter().collect();
            entries.sort_by_key(|(key, _)| format!("{key:?}"));
            let ProtoKind::Message(entry) = field.kind() else { unreachable!() };
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

fn encode_value(value: &Value, bytes: &mut Vec<u8>, depth: usize) -> Result<(), Error> {
    match value {
        Value::Bool(value) => value.write(bytes),
        Value::I32(value) | Value::EnumNumber(value) => value.write(bytes),
        Value::I64(value) => value.write(bytes),
        Value::U32(value) => value.write(bytes),
        Value::U64(value) => value.write(bytes),
        Value::F32(value) => value.write(bytes),
        Value::F64(value) => value.write(bytes),
        Value::String(value) => value.write(bytes),
        Value::Bytes(value) => value.to_vec().write(bytes),
        Value::Message(message) => encode(message, bytes, depth + 1)?,
        Value::List(_) | Value::Map(_) => unreachable!("collection fields are encoded by their descriptor"),
    }
    Ok(())
}

/* === Tests === */
