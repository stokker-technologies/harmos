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

mod columns;

#[cfg(test)]
mod tests;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
// Third Party Imports
use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray, StructArray, TimestampNanosecondArray, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Fields, Schema, TimeUnit};
// Local Imports
use super::transform::project::Project;
use crate::schema::{self, Cardinality, Event, PathElement, Scalar, graph};
use crate::storage::{FileReference, ObjectEntry, StorageCursor, StorageSchema, StoredRecord, StoredRow};
use crate::{Error, Result};

/* === Constants === */

const MAX_DEPTH: usize = 32;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

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

pub(super) fn projected<'a>(
    schema: &StorageSchema, rows: impl ExactSizeIterator<Item = &'a StoredRecord>, project: &Project, output: Arc<Schema>,
) -> Result<RecordBatch> {
    let settings = harmos_domain::config::SchemaSettings::default();
    let validator = schema::Validator::from_schema(&schema.schema, &settings).map_err(|_| invalid("invalid payload schema"))?;
    let mut columns = output.fields().iter().map(|field| columns::Column::new(field.data_type(), rows.len())).collect::<Vec<_>>();

    for (row_index, row) in rows.enumerate() {
        if row.schema != schema.key {
            return Err(invalid("record batch has incompatible schemas"));
        }
        validator
            .visit(&row.payload, |path, event| {
                for (column, projected) in columns.iter_mut().zip(&project.fields) {
                    let prefix = &projected.field.fields[1..];
                    if let Event::Variant { index } = event
                        && prefix.last().is_some_and(|name| name == "__variant")
                        && path.len() == prefix.len()
                        && path_matches(&path[..path.len() - 1], &prefix[..prefix.len() - 1])
                    {
                        column.event(&[], Event::Scalar(Scalar::U32(index)))?;
                        continue;
                    }
                    if !path_matches(path, prefix) {
                        continue;
                    }
                    let relative = &path[prefix.len()..];
                    if relative.is_empty() && matches!(event, Event::Variant { .. }) {
                        continue;
                    }
                    column.event(relative, event)?;
                }
                Ok::<_, Error>(())
            })
            .map_err(|_| invalid("payload decoding failed"))?;
        // An Absent Parent Or Unselected Enum Variant Nulls The Projected Child.
        for column in &mut columns {
            if column.len() == row_index {
                column.null()?;
            }
        }
    }
    let columns = columns.into_iter().map(columns::Column::finish).collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new(output, columns).map_err(|_| invalid("projected payload batch mismatch"))
}

pub(super) fn objects(schema: &StorageSchema, rows: &[ObjectEntry]) -> Result<RecordBatch> {
    // The caller resolves every selected schema and establishes compatibility.
    // Equivalent payload graphs may have different logical schema identifiers.

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
        encoded.push(crate::Message::encode(row.schema.as_ref()));
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

fn path_matches(path: &[PathElement<'_>], prefix: &[String]) -> bool {
    path.len() >= prefix.len()
        && path.iter().zip(prefix).all(|(part, name)| matches!(part, PathElement::Field { name: field, .. } if *field == name))
}

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
    let settings = harmos_domain::config::SchemaSettings::default();
    let validator = schema::Validator::from_schema(&schema.schema, &settings).map_err(|_| invalid("invalid payload schema"))?;
    let kind = DataType::Struct(payload_fields(&schema.schema)?);
    let mut columns = columns::Column::new(&kind, payloads.len());
    for bytes in payloads {
        validator.visit(bytes, |path, event| columns.event(path, event)).map_err(|_| invalid("payload decoding failed"))?;
    }
    columns.finish()
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
