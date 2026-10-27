/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native storage rows mapped to Arrow batches

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
// Third Party Imports
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, ListArray, RecordBatch, StringArray,
    StructArray, TimestampNanosecondArray, UInt32Array, UInt64Array,
};
use arrow_buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow_schema::{DataType, Field, Fields, Schema, TimeUnit};
// Local Imports
use crate::schema::{self, Cardinality, Event, PathElement, Scalar, graph};
use crate::storage::{FileReference, ObjectEntry, StorageCursor, StorageSchema, StoredRecord, StoredRow};
use crate::{Error, Result};

/* === Constants === */

const MAX_DEPTH: usize = 32;

/* === Macros === */

macro_rules! scalar_array {
    ($values:expr, $variant:ident, $array:ty) => {{
        let values = $values
            .iter()
            .map(|value| match value {
                Some(Node::Scalar(Scalar::$variant(value))) => Ok(Some(*value)),
                None | Some(Node::Null) => Ok(None),
                _ => Err(invalid("payload value differs from declared schema")),
            })
            .collect::<Result<Vec<_>>>()?;
        Arc::new(<$array>::from(values)) as ArrayRef
    }};
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

enum Node<'a> {
    Null,
    Scalar(Scalar<'a>),
    Message(Vec<(String, Node<'a>)>),
    List(Vec<Node<'a>>),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn payload_fields(schema: &graph::Schema) -> Result<Fields> {
    fields(schema, schema.root, 0, &mut 0)
}

pub(super) fn records(schema: &StorageSchema, rows: &[StoredRecord]) -> Result<RecordBatch> {
    let refs = rows.iter().collect::<Vec<_>>();
    record_batch(schema, &refs, None)
}

pub(super) fn stream(schema: &StorageSchema, rows: &[StoredRow]) -> Result<RecordBatch> {
    let refs = rows.iter().map(|row| &row.record).collect::<Vec<_>>();
    record_batch(schema, &refs, Some(rows))
}

pub(super) fn objects(schema: &StorageSchema, rows: &[ObjectEntry]) -> Result<RecordBatch> {
    // The caller resolves every selected schema and establishes compatibility.
    // Equivalent payload graphs may have different logical schema identifiers.
    for row in rows {
        row.value.schema.require_postcard()?;
    }

    let payloads = rows.iter().map(|row| row.value.payload.as_slice()).collect::<Vec<_>>();
    let payload = payload(schema, &payloads)?;

    let mut columns = cursor_columns(rows.iter().map(|row| &row.position));
    columns.push((Field::new("item", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.value.item.as_str())))));
    columns.push((
        Field::new("revision", DataType::UInt64, false),
        Arc::new(UInt64Array::from(rows.iter().map(|row| row.version.revision().get()).collect::<Vec<_>>())),
    ));

    columns.push((Field::new("schema_id", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.value.schema.id.as_str())))));
    columns.push((
        Field::new("schema_version", DataType::UInt32, false),
        Arc::new(UInt32Array::from(rows.iter().map(|row| row.value.schema.version.get()).collect::<Vec<_>>())),
    ));
    columns
        .push((Field::new("payload_encoding", DataType::Utf8, false), strings(rows.iter().map(|row| Some(encoding(&row.value.schema))))));
    assemble(columns, payload)
}

pub(super) fn files(rows: &[FileReference]) -> Result<RecordBatch> {
    let mut columns = cursor_columns(rows.iter().map(|row| &row.position));
    columns.extend([
        (Field::new("item", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.metadata.id.as_str())))),
        (Field::new("scope", DataType::Utf8, true), strings(rows.iter().map(|row| row.metadata.scope.as_ref().map(|v| v.as_str())))),
        (Field::new("media_type", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.metadata.media_type.as_str())))),
        (
            Field::new("expected_length", DataType::UInt64, true),
            Arc::new(UInt64Array::from(rows.iter().map(|row| row.metadata.expected_length).collect::<Vec<_>>())) as ArrayRef,
        ),
        (
            Field::new("expected_sha256", DataType::Binary, true),
            Arc::new(BinaryArray::from(
                rows.iter().map(|row| row.metadata.expected_sha256.as_ref().map(|value| value.as_slice())).collect::<Vec<_>>(),
            )) as ArrayRef,
        ),
        (
            Field::new("length", DataType::UInt64, false),
            Arc::new(UInt64Array::from(rows.iter().map(|row| row.length).collect::<Vec<_>>())) as ArrayRef,
        ),
        (
            Field::new("sha256", DataType::Binary, false),
            Arc::new(BinaryArray::from_iter_values(rows.iter().map(|row| row.sha256.as_slice()))) as ArrayRef,
        ),
    ]);
    flat(columns)
}

pub(super) fn schemas(rows: &[StorageSchema]) -> Result<RecordBatch> {
    let mut encoded = Vec::with_capacity(rows.len());
    for row in rows {
        encoded.push(crate::Message::encode_to_vec(row.schema.as_ref()));
    }
    let mut columns = cursor_columns(rows.iter().map(|row| &row.position));
    columns.extend([
        (Field::new("schema_id", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.key.id.as_str())))),
        (
            Field::new("schema_version", DataType::UInt32, false),
            Arc::new(UInt32Array::from(rows.iter().map(|row| row.key.version.get()).collect::<Vec<_>>())) as ArrayRef,
        ),
        (Field::new("payload_encoding", DataType::Utf8, false), strings(rows.iter().map(|row| Some(encoding(&row.key))))),
        (Field::new("schema_encoding", DataType::Utf8, false), strings(rows.iter().map(|_| Some("harmos.graph.postcard.v1")))),
        (
            Field::new("definition", DataType::Binary, false),
            Arc::new(BinaryArray::from_iter_values(encoded.iter().map(Vec::as_slice))) as ArrayRef,
        ),
    ]);
    flat(columns)
}

/* === Private Functions === */

fn record_batch(schema: &StorageSchema, rows: &[&StoredRecord], streams: Option<&[StoredRow]>) -> Result<RecordBatch> {
    for row in rows {
        if row.schema != schema.key {
            return Err(invalid("record batch has incompatible schemas"));
        }
    }

    let payloads = rows.iter().map(|row| row.payload.as_slice()).collect::<Vec<_>>();
    let payload = payload(schema, &payloads)?;

    let times = rows.iter().map(|row| row.source_time.map(timestamp).transpose()).collect::<Result<Vec<_>>>()?;
    let mut columns = cursor_columns(rows.iter().map(|row| &row.position));
    columns.extend([
        (Field::new("item", DataType::Utf8, false), strings(rows.iter().map(|row| Some(row.item.as_str())))),
        (Field::new("scope", DataType::Utf8, true), strings(rows.iter().map(|row| row.scope.as_ref().map(|v| v.as_str())))),
        (Field::new("source", DataType::Utf8, true), strings(rows.iter().map(|row| row.source.as_ref().map(|v| v.as_str())))),
        (
            Field::new("source_time", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), true),
            Arc::new(TimestampNanosecondArray::from(times).with_timezone("UTC")) as ArrayRef,
        ),
    ]);
    if let Some(streams) = streams {
        columns.push((Field::new("stream", DataType::Utf8, false), strings(streams.iter().map(|row| Some(row.stream.as_str())))));
        columns.push((
            Field::new("sequence", DataType::UInt64, false),
            Arc::new(UInt64Array::from(streams.iter().map(|row| row.sequence).collect::<Vec<_>>())),
        ));
    }
    append_schema_columns(&mut columns, &schema.key, rows.len())?;
    assemble(columns, payload)
}

fn fields(schema: &graph::Schema, root: usize, depth: usize, count: &mut usize) -> Result<Fields> {
    if depth >= MAX_DEPTH {
        return Err(invalid("recursive or excessive Arrow payload nesting"));
    }
    let definition = schema.definitions.get(root).ok_or_else(|| invalid("missing schema definition"))?;
    *count = count.checked_add(definition.len() + 1).ok_or_else(exhausted)?;
    if *count > super::super::arrow::MAX_FIELDS {
        return Err(invalid("expanded Arrow schema exceeds field limit"));
    }
    let mut output = Vec::with_capacity(definition.len() + 1);
    if definition.first().is_some_and(|field| field.cardinality == Cardinality::Variant) {
        if definition.iter().any(|field| field.name == "__variant") {
            return Err(invalid("enum variant uses reserved discriminant name"));
        }
        output.push(Field::new("__variant", DataType::UInt32, false));
    }
    for field in definition {
        let kind = match field.kind {
            graph::Kind::Bool => DataType::Boolean,
            graph::Kind::U32 => DataType::UInt32,
            graph::Kind::U64 => DataType::UInt64,
            graph::Kind::I32 => DataType::Int32,
            graph::Kind::I64 => DataType::Int64,
            graph::Kind::F32 => DataType::Float32,
            graph::Kind::F64 => DataType::Float64,
            graph::Kind::String => DataType::Utf8,
            graph::Kind::Bytes => DataType::Binary,
            graph::Kind::Message { definition } => DataType::Struct(fields(schema, definition, depth + 1, count)?),
        };
        let kind =
            if field.cardinality == Cardinality::Repeated { DataType::List(Arc::new(Field::new("item", kind, false))) } else { kind };
        output.push(
            Field::new(field.name.as_ref(), kind, matches!(field.cardinality, Cardinality::Optional | Cardinality::Variant))
                .with_metadata(HashMap::from([("harmos.tag".to_owned(), field.tag.to_string())])),
        );
    }
    Ok(output.into())
}

fn payload(schema: &StorageSchema, payloads: &[&[u8]]) -> Result<ArrayRef> {
    schema.key.require_postcard()?;
    let settings = harmos_domain::config::SchemaSettings::default();
    let validator = schema::Validator::from_schema(&schema.schema, &settings).map_err(|_| invalid("invalid payload schema"))?;
    let mut roots = Vec::with_capacity(payloads.len());
    for bytes in payloads {
        let mut root = Node::Null;
        validator
            .visit(bytes, |path, event| {
                match event {
                    Event::BeginMessage => *node_at(&mut root, path)? = Node::Message(Vec::new()),
                    Event::BeginRepeated { length } => *node_at(&mut root, path)? = Node::List((0..length).map(|_| Node::Null).collect()),
                    Event::Scalar(value) => *node_at(&mut root, path)? = Node::Scalar(value),
                    Event::Variant { index } => {
                        let parent = path.get(..path.len().saturating_sub(1)).ok_or_else(|| invalid("invalid enum path"))?;
                        let Node::Message(fields) = node_at(&mut root, parent)? else {
                            return Err(invalid("invalid enum parent"));
                        };
                        fields.push(("__variant".into(), Node::Scalar(Scalar::U32(index))));
                    }
                    Event::Optional { present: false } => *node_at(&mut root, path)? = Node::Null,
                    _ => {}
                }
                Ok::<_, Error>(())
            })
            .map_err(|_| invalid("payload decoding failed"))?;
        roots.push(root);
    }
    let refs = roots.iter().map(Some).collect::<Vec<_>>();
    array(&DataType::Struct(payload_fields(&schema.schema)?), &refs)
}

fn node_at<'n, 'a>(root: &'n mut Node<'a>, path: &[PathElement<'_>]) -> Result<&'n mut Node<'a>> {
    let Some((part, rest)) = path.split_first() else {
        return Ok(root);
    };
    match (root, part) {
        (Node::Message(fields), PathElement::Field { name, .. }) => {
            let index = match fields.iter().position(|(field, _)| field == name) {
                Some(index) => index,
                None => {
                    fields.push(((*name).to_owned(), Node::Null));
                    fields.len() - 1
                }
            };
            node_at(&mut fields[index].1, rest)
        }
        (Node::List(values), PathElement::Index(index)) => {
            node_at(values.get_mut(*index).ok_or_else(|| invalid("list index exceeds declaration"))?, rest)
        }
        _ => Err(invalid("payload path differs from schema")),
    }
}

fn array(kind: &DataType, values: &[Option<&Node<'_>>]) -> Result<ArrayRef> {
    Ok(match kind {
        DataType::Boolean => scalar_array!(values, Bool, BooleanArray),
        DataType::UInt32 => scalar_array!(values, U32, UInt32Array),
        DataType::UInt64 => scalar_array!(values, U64, UInt64Array),
        DataType::Int32 => scalar_array!(values, I32, Int32Array),
        DataType::Int64 => scalar_array!(values, I64, Int64Array),
        DataType::Float32 => scalar_array!(values, F32, Float32Array),
        DataType::Float64 => scalar_array!(values, F64, Float64Array),
        DataType::Utf8 => {
            let items = values
                .iter()
                .map(|value| match value {
                    Some(Node::Scalar(Scalar::String(value))) => Ok(Some(*value)),
                    None | Some(Node::Null) => Ok(None),
                    _ => Err(invalid("expected string payload")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(StringArray::from(items))
        }
        DataType::Binary => {
            let items = values
                .iter()
                .map(|value| match value {
                    Some(Node::Scalar(Scalar::Bytes(value))) => Ok(Some(*value)),
                    None | Some(Node::Null) => Ok(None),
                    _ => Err(invalid("expected binary payload")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(BinaryArray::from(items))
        }
        DataType::Struct(fields) => {
            let mut children = Vec::with_capacity(fields.len());
            for field in fields {
                let members = values
                    .iter()
                    .map(|value| match value {
                        Some(Node::Message(entries)) => Ok(entries.iter().find(|(name, _)| name == field.name()).map(|(_, value)| value)),
                        None | Some(Node::Null) => Ok(None),
                        _ => Err(invalid("expected nested message payload")),
                    })
                    .collect::<Result<Vec<_>>>()?;
                children.push(array(field.data_type(), &members)?);
            }
            let valid = values.iter().map(|value| matches!(value, Some(Node::Message(_)))).collect::<Vec<_>>();
            if fields.is_empty() {
                Arc::new(StructArray::new_empty_fields(values.len(), Some(NullBuffer::from(valid))))
            } else {
                Arc::new(
                    StructArray::try_new(fields.clone(), children, Some(NullBuffer::from(valid)))
                        .map_err(|_| invalid("nested Arrow schema mismatch"))?,
                )
            }
        }
        DataType::List(field) => {
            let mut offsets = Vec::with_capacity(values.len() + 1);
            let mut elements = Vec::new();
            let mut valid = Vec::with_capacity(values.len());
            offsets.push(0i32);
            for value in values {
                match value {
                    Some(Node::List(items)) => {
                        valid.push(true);
                        elements.extend(items.iter().map(Some));
                    }
                    None | Some(Node::Null) => valid.push(false),
                    _ => return Err(invalid("expected repeated payload")),
                }
                offsets.push(i32::try_from(elements.len()).map_err(|_| invalid("list exceeds Arrow offset range"))?);
            }
            Arc::new(
                ListArray::try_new(
                    field.clone(),
                    OffsetBuffer::new(ScalarBuffer::from(offsets)),
                    array(field.data_type(), &elements)?,
                    Some(NullBuffer::from(valid)),
                )
                .map_err(|_| invalid("list Arrow schema mismatch"))?,
            )
        }
        _ => return Err(invalid("unsupported payload type")),
    })
}

fn cursor_columns<'a>(rows: impl Iterator<Item = &'a StorageCursor> + Clone) -> Vec<(Field, ArrayRef)> {
    vec![
        (Field::new("store", DataType::Utf8, false), strings(rows.clone().map(|row| Some(row.history.store.as_str())))),
        (Field::new("epoch", DataType::Utf8, false), strings(rows.clone().map(|row| Some(row.history.epoch.as_str())))),
        (Field::new("position", DataType::UInt64, false), Arc::new(UInt64Array::from(rows.map(|row| row.sequence).collect::<Vec<_>>()))),
    ]
}
fn append_schema_columns(columns: &mut Vec<(Field, ArrayRef)>, key: &crate::storage::SchemaKey, rows: usize) -> Result<()> {
    columns.push((Field::new("schema_id", DataType::Utf8, false), strings((0..rows).map(|_| Some(key.id.as_str())))));
    columns.push((Field::new("schema_version", DataType::UInt32, false), Arc::new(UInt32Array::from(vec![key.version.get(); rows]))));
    columns.push((Field::new("payload_encoding", DataType::Utf8, false), strings((0..rows).map(|_| Some(encoding(key))))));
    Ok(())
}
fn encoding(key: &crate::storage::SchemaKey) -> &'static str {
    match key.payload_encoding {
        crate::storage::PayloadEncoding::PostcardV1 => "postcard_v1",
        crate::storage::PayloadEncoding::ProtobufV1 => "protobuf_v1",
    }
}
fn strings<'a>(values: impl Iterator<Item = Option<&'a str>>) -> ArrayRef {
    Arc::new(StringArray::from_iter(values))
}
fn flat(columns: Vec<(Field, ArrayRef)>) -> Result<RecordBatch> {
    let (fields, arrays): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
    RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).map_err(|_| invalid("Arrow metadata batch mismatch"))
}
fn assemble(columns: Vec<(Field, ArrayRef)>, payload: ArrayRef) -> Result<RecordBatch> {
    let (fields, arrays): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
    let fields: Fields = fields.into();
    let metadata = StructArray::try_new(fields.clone(), arrays, None).map_err(|_| invalid("Arrow metadata mismatch"))?;
    RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("_storage", DataType::Struct(fields), false),
            Field::new("payload", payload.data_type().clone(), false),
        ])),
        vec![Arc::new(metadata), payload],
    )
    .map_err(|_| invalid("Arrow result batch mismatch"))
}

fn timestamp(value: SystemTime) -> Result<i64> {
    let value = match value.duration_since(UNIX_EPOCH) {
        Ok(value) => i128::try_from(value.as_nanos()).map_err(|_| invalid("timestamp overflow"))?,
        Err(error) => -i128::try_from(error.duration().as_nanos()).map_err(|_| invalid("timestamp overflow"))?,
    };
    i64::try_from(value).map_err(|_| invalid("timestamp outside Arrow nanosecond range"))
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage Arrow source", reason }
}
fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "storage Arrow construction" }
}

/* === Tests === */
