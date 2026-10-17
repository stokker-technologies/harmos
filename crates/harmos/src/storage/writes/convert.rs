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
use std::num::NonZeroUsize;
use std::time::{Duration, UNIX_EPOCH};
// Third Party Imports
use arrow_array::{
    Array, BinaryArray, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array,
    ListArray, StringArray, StructArray, TimestampNanosecondArray, UInt32Array, UInt64Array,
};
// Local Imports
use super::DecodedBatch;
use crate::schema::{
    Cardinality,
    graph::{Field, Kind, Schema},
};
use crate::storage::{ObjectKey, ObjectValue, PayloadEncoding, RecordInput, StreamInput};
use crate::{Error, Identifier, Result};

/* === Constants === */
const MAX_DEPTH: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
/// Encode typed payload rows using the registered Harmos message schema. The
/// allowance covers the returned row-vector allocation and all encoded bytes;
/// validation and exact sizing finish before output allocation begins.
pub fn encode_payloads(
    schema: &Schema,
    payload: &StructArray,
    encoding: PayloadEncoding,
    max_bytes: NonZeroUsize,
) -> Result<Vec<Vec<u8>>> {
    let metadata = payload
        .len()
        .checked_mul(std::mem::size_of::<Vec<u8>>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<Vec<u8>>>()))
        .ok_or_else(exhausted)?;
    let mut count = Counter {
        bytes: metadata,
        limit: max_bytes.get(),
    };
    if metadata > max_bytes.get() {
        return Err(exhausted());
    }
    for row in 0..payload.len() {
        encode_message(schema, schema.root, payload, row, encoding, 0, &mut count)?;
    }
    let mut rows = Vec::with_capacity(payload.len());
    for row in 0..payload.len() {
        let mut size = Counter {
            bytes: 0,
            limit: max_bytes.get(),
        };
        encode_message(schema, schema.root, payload, row, encoding, 0, &mut size)?;
        let mut bytes = Vec::with_capacity(size.bytes);
        encode_message(schema, schema.root, payload, row, encoding, 0, &mut bytes)?;
        rows.push(bytes);
    }
    Ok(rows)
}

pub(crate) fn records(
    scope: Option<&Identifier>,
    batch: &DecodedBatch,
    schema: &Schema,
) -> Result<Vec<RecordInput>> {
    let data = batch.data.as_record_batch();
    let metadata = struct_column(
        data.column_by_name("_storage")
            .ok_or_else(invalid)?
            .as_ref(),
    )?;
    let payload = struct_column(data.column_by_name("payload").ok_or_else(invalid)?.as_ref())?;
    (0..data.num_rows())
        .map(|row| {
            if metadata.is_null(row) {
                return Err(invalid());
            }
            let authored_scope = optional_id(metadata, "scope", row)?;
            if scope.is_some_and(|expected| {
                authored_scope
                    .as_ref()
                    .is_some_and(|actual| actual != expected)
            }) {
                return Err(invalid());
            }
            let source_time = metadata
                .column_by_name("source_time")
                .filter(|column| !column.is_null(row))
                .map(|column| {
                    let nanos = typed::<TimestampNanosecondArray>(column.as_ref())?.value(row);
                    if nanos >= 0 {
                        UNIX_EPOCH.checked_add(Duration::from_nanos(nanos as u64))
                    } else {
                        UNIX_EPOCH.checked_sub(Duration::from_nanos(nanos.unsigned_abs()))
                    }
                    .ok_or_else(invalid)
                })
                .transpose()?;
            let mut bytes = Vec::new();
            encode_message(
                schema,
                schema.root,
                payload,
                row,
                batch.schema.payload_encoding,
                0,
                &mut bytes,
            )?;
            Ok(RecordInput {
                schema: batch.schema.clone(),
                scope: scope.cloned().or(authored_scope),
                item: required_id(metadata, "item", row)?,
                source: optional_id(metadata, "source", row)?,
                source_time,
                payload: bytes,
            })
        })
        .collect()
}

pub(crate) fn streams(
    scope: Option<&Identifier>,
    batch: &DecodedBatch,
    schema: &Schema,
) -> Result<Vec<StreamInput>> {
    let records = records(scope, batch, schema)?;
    let metadata = struct_column(
        batch
            .data
            .as_record_batch()
            .column_by_name("_storage")
            .ok_or_else(invalid)?
            .as_ref(),
    )?;
    let sequence = typed::<UInt64Array>(
        metadata
            .column_by_name("sequence")
            .ok_or_else(invalid)?
            .as_ref(),
    )?;
    records
        .into_iter()
        .enumerate()
        .map(|(row, record)| {
            if sequence.is_null(row) {
                return Err(invalid());
            }
            if optional_id(metadata, "stream", row)?
                .is_some_and(|stream| stream != batch.destination)
            {
                return Err(invalid());
            }
            Ok(StreamInput {
                record,
                stream: batch.destination.clone(),
                sequence: sequence.value(row),
            })
        })
        .collect()
}

pub(crate) fn objects(
    batch: &DecodedBatch,
    schema: &Schema,
    max_key_bytes: NonZeroUsize,
) -> Result<Vec<ObjectValue>> {
    let data = batch.data.as_record_batch();
    let metadata = struct_column(
        data.column_by_name("_storage")
            .ok_or_else(invalid)?
            .as_ref(),
    )?;
    let items = typed::<StringArray>(
        metadata
            .column_by_name("item")
            .ok_or_else(invalid)?
            .as_ref(),
    )?;
    let payload = struct_column(data.column_by_name("payload").ok_or_else(invalid)?.as_ref())?;
    (0..data.num_rows())
        .map(|row| {
            if metadata.is_null(row) || items.is_null(row) {
                return Err(invalid());
            }
            let mut bytes = Vec::new();
            encode_message(
                schema,
                schema.root,
                payload,
                row,
                batch.schema.payload_encoding,
                0,
                &mut bytes,
            )?;
            Ok(ObjectValue {
                schema: batch.schema.clone(),
                item: ObjectKey::new(items.value(row), max_key_bytes)?,
                payload: bytes,
            })
        })
        .collect()
}

/* === Private Functions === */
trait Sink {
    fn append(&mut self, value: &[u8]) -> Result<()>;
}
struct Counter {
    bytes: usize,
    limit: usize,
}
impl Sink for Counter {
    fn append(&mut self, value: &[u8]) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(value.len())
            .filter(|bytes| *bytes <= self.limit)
            .ok_or_else(exhausted)?;
        Ok(())
    }
}
impl Sink for Vec<u8> {
    fn append(&mut self, value: &[u8]) -> Result<()> {
        self.extend_from_slice(value);
        Ok(())
    }
}
fn scalar<T: serde::Serialize>(value: T, output: &mut impl Sink) -> Result<()> {
    let mut bytes = [0u8; 16];
    let encoded = postcard::to_slice(&value, &mut bytes).map_err(|_| invalid())?;
    output.append(encoded)
}
fn encode_message(
    schema: &Schema,
    definition: usize,
    values: &StructArray,
    row: usize,
    encoding: PayloadEncoding,
    depth: usize,
    output: &mut impl Sink,
) -> Result<()> {
    if depth > MAX_DEPTH || values.is_null(row) {
        return Err(invalid());
    }
    let fields = schema.definitions.get(definition).ok_or_else(invalid)?;
    if fields
        .iter()
        .any(|field| field.cardinality == Cardinality::Variant)
    {
        let mut selected = None;
        for (index, field) in fields.iter().enumerate() {
            if let Some(value) = values.column_by_name(&field.name)
                && !value.is_null(row)
            {
                if selected.is_some() {
                    return Err(invalid());
                }
                selected = Some((index, field, value));
            }
        }
        let (index, field, value) = selected.ok_or_else(invalid)?;
        let discriminant = typed::<UInt32Array>(
            values
                .column_by_name("__variant")
                .ok_or_else(invalid)?
                .as_ref(),
        )?;
        if discriminant.is_null(row) || discriminant.value(row) as usize != index {
            return Err(invalid());
        }
        if encoding == PayloadEncoding::PostcardV1 {
            scalar(index as u32, output)?;
        }
        return encode_value(
            schema,
            field,
            value.as_ref(),
            row,
            encoding,
            depth + 1,
            output,
        );
    }
    for field in fields {
        let value = values.column_by_name(&field.name).ok_or_else(invalid)?;
        match field.cardinality {
            Cardinality::Optional => {
                if encoding == PayloadEncoding::PostcardV1 {
                    scalar(!value.is_null(row), output)?;
                }
                if value.is_null(row) {
                    continue;
                }
            }
            Cardinality::Repeated => {
                let lists = typed::<ListArray>(value.as_ref())?;
                if lists.is_null(row) {
                    return Err(invalid());
                }
                let start = lists.value_offsets()[row] as usize;
                let end = lists.value_offsets()[row + 1] as usize;
                if encoding == PayloadEncoding::PostcardV1 {
                    scalar((end - start) as u64, output)?;
                }
                for index in start..end {
                    encode_value(
                        schema,
                        field,
                        lists.values().as_ref(),
                        index,
                        encoding,
                        depth + 1,
                        output,
                    )?;
                }
                continue;
            }
            Cardinality::Required | Cardinality::Variant => {}
        }
        encode_value(
            schema,
            field,
            value.as_ref(),
            row,
            encoding,
            depth + 1,
            output,
        )?;
    }
    Ok(())
}
fn encode_value(
    schema: &Schema,
    field: &Field,
    array: &dyn Array,
    row: usize,
    encoding: PayloadEncoding,
    depth: usize,
    output: &mut impl Sink,
) -> Result<()> {
    if array.is_null(row) {
        return Err(invalid());
    }
    let protobuf = encoding == PayloadEncoding::ProtobufV1;
    macro_rules! number {
        ($array:ty, $wire:expr, $encode:expr) => {{
            let value = typed::<$array>(array)?.value(row);
            if protobuf {
                varint((u64::from(field.tag) << 3) | $wire, output)?;
                ($encode)(value, output)?;
            } else {
                scalar(value, output)?;
            }
        }};
    }
    match field.kind {
        Kind::Bool => number!(BooleanArray, 0, |value: bool, out: &mut _| varint(
            u64::from(value),
            out
        )),
        Kind::U32 => number!(UInt32Array, 0, |value: u32, out: &mut _| varint(
            u64::from(value),
            out
        )),
        Kind::U64 => number!(UInt64Array, 0, |value: u64, out: &mut _| varint(value, out)),
        Kind::I32 => number!(Int32Array, 0, |value: i32, out: &mut _| varint(
            value as i64 as u64,
            out
        )),
        Kind::I64 => number!(Int64Array, 0, |value: i64, out: &mut _| varint(
            value as u64,
            out
        )),
        Kind::F32 => number!(Float32Array, 5, |value: f32, out: &mut _| Sink::append(
            out,
            &value.to_le_bytes()
        )),
        Kind::F64 => number!(Float64Array, 1, |value: f64, out: &mut _| Sink::append(
            out,
            &value.to_le_bytes()
        )),
        Kind::String => {
            let value = typed::<StringArray>(array)?.value(row);
            if protobuf {
                header(field.tag, value.len(), output)?;
            } else {
                scalar(value.len() as u64, output)?;
            }
            output.append(value.as_bytes())?;
        }
        Kind::Bytes => {
            let value = typed::<BinaryArray>(array)?.value(row);
            if protobuf {
                header(field.tag, value.len(), output)?;
            } else {
                scalar(value.len() as u64, output)?;
            }
            output.append(value)?;
        }
        Kind::Message { definition } => {
            let values = struct_column(array)?;
            if protobuf {
                let mut count = Counter {
                    bytes: 0,
                    limit: usize::MAX,
                };
                encode_message(
                    schema,
                    definition,
                    values,
                    row,
                    encoding,
                    depth + 1,
                    &mut count,
                )?;
                header(field.tag, count.bytes, output)?;
            }
            encode_message(schema, definition, values, row, encoding, depth + 1, output)?;
        }
    }
    Ok(())
}

fn typed<T: Array + 'static>(array: &dyn Array) -> Result<&T> {
    array.as_any().downcast_ref().ok_or_else(invalid)
}
fn struct_column(array: &dyn Array) -> Result<&StructArray> {
    typed(array)
}
fn optional_id(values: &StructArray, name: &str, row: usize) -> Result<Option<Identifier>> {
    let Some(column) = values.column_by_name(name) else {
        return Ok(None);
    };
    let strings = typed::<StringArray>(column.as_ref())?;
    if strings.is_null(row) {
        return Ok(None);
    }
    Identifier::new(strings.value(row))
        .map(Some)
        .map_err(|_| invalid())
}
fn required_id(values: &StructArray, name: &str, row: usize) -> Result<Identifier> {
    optional_id(values, name, row)?.ok_or_else(invalid)
}
fn varint(mut value: u64, output: &mut impl Sink) -> Result<()> {
    let mut bytes = [0u8; 10];
    let mut count = 0;
    while value >= 128 {
        bytes[count] = (value as u8 & 127) | 128;
        value >>= 7;
        count += 1;
    }
    bytes[count] = value as u8;
    output.append(&bytes[..=count])
}
fn header(tag: u32, length: usize, output: &mut impl Sink) -> Result<()> {
    varint((u64::from(tag) << 3) | 2, output)?;
    varint(length as u64, output)
}
fn exhausted() -> Error {
    Error::ResourceExhausted {
        resource: "Arrow payload encoding bytes",
    }
}
fn invalid() -> Error {
    Error::InvalidInput {
        field: "write Arrow batch",
        reason: "batch metadata or typed payload does not match its schema",
    }
}

/* === Tests === */
