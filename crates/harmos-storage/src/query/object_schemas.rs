/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Selected Object Schema Normalization

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::storage::query::transform::ProjectExecution;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
use arrow_array::{RecordBatch, make_array};
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::ObjectSelection;
use super::reader::schema_at;
use super::transform::{Transform, project::Project};
use crate::storage::backend::ScanPage;
use crate::storage::config::SCAN_ROWS;
use crate::storage::manager::Inner;
use crate::storage::snapshot::Snapshot;
use crate::storage::{ObjectEntry, SchemaKey, StorageSchema};
use crate::{Error, Result};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct NormalizedObjects {
    schemas: Vec<StorageSchema>,
    projection: Option<Project>,
    output: Arc<Schema>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl NormalizedObjects {
    #[tracing::instrument(name = "storage.query.object_schemas.resolve", level = "trace", skip_all)]
    pub(super) async fn resolve(
        inner: &Arc<Inner>, snapshot: &Snapshot, selection: &ObjectSelection, transforms: &mut Vec<Transform>,
    ) -> Result<Self> {
        let mut schemas: Vec<StorageSchema> = Vec::new();
        if let Some(key) = &selection.schema {
            schemas.push(schema_at(inner, snapshot, key).await?);
        } else {
            // Inspect Actual Selected Objects, Not Unrelated Schema Registrations.
            let scan_bytes = inner.settings.scan_bytes();
            let mut after = 0u64;
            let rows = SCAN_ROWS;
            loop {
                let page = ScanPage { after, rows, bytes: scan_bytes };
                let (objects, _) = snapshot.scan::<ObjectEntry>(selection, page).await?;
                let last = objects.last().map(|object| object.position.sequence);
                let mut keys: Vec<SchemaKey> = Vec::new();
                for object in &objects {
                    let key = &object.value.schema;
                    if schemas.iter().all(|schema| &schema.key != key) && keys.iter().all(|known| known != key) {
                        keys.push(key.clone());
                    }
                }
                let Some(last) = last else {
                    break;
                };
                after = last;
                for key in keys {
                    schemas.push(schema_at(inner, snapshot, &key).await?);
                }
            }
        }
        if schemas.is_empty() {
            return Err(invalid("empty untyped object selection requires an explicit schema"));
        }
        let projection = match transforms.first() {
            Some(Transform::Project(project)) => Some(project.clone()),
            _ => None,
        };

        let mut output: Option<Arc<Schema>> = None;
        for schema in &schemas {
            let empty = super::source::objects(schema, &[])?;
            let candidate = match &projection {
                Some(project) => canonical_schema(&project.output_schema(empty.schema().as_ref())?),
                None => empty.schema().as_ref().clone(),
            };
            crate::storage::arrow::validate_schema(&candidate)?;
            if output.as_ref().is_some_and(|schema| schema.as_ref() != &candidate) {
                return Err(invalid("selected object schemas require a leading projection to compatible fields"));
            }
            output.get_or_insert_with(|| Arc::new(candidate));
        }
        let output = output.expect("nonempty selected schemas");
        if projection.is_some() {
            transforms.remove(0);
        }
        Ok(Self { schemas, projection, output })
    }

    pub(super) fn empty(&self) -> RecordBatch {
        RecordBatch::new_empty(Arc::clone(&self.output))
    }

    pub(super) fn map(&self, rows: &[ObjectEntry]) -> Result<RecordBatch> {
        if rows.is_empty() {
            return Ok(self.empty());
        }
        let mut batches = Vec::new();
        let mut start = 0usize;
        // Contiguous Runs Preserve The Original Capture Ordering Across Schemas.
        while start < rows.len() {
            let key = &rows[start].value.schema;
            let end = rows[start..].iter().position(|row| &row.value.schema != key).map_or(rows.len(), |length| start + length);
            let schema = self
                .schemas
                .iter()
                .find(|schema| &schema.key == key)
                .ok_or_else(|| invalid("object schema was not resolved at the requested boundary"))?;
            let source = super::source::objects(schema, &rows[start..end])?;
            let projected = match &self.projection {
                Some(project) => canonical_batch(project.apply(&source, &mut || Ok(()))?, Arc::clone(&self.output))?,
                None => source,
            };
            if projected.schema() != self.output {
                return Err(invalid("normalized object schema changed"));
            }
            batches.push(projected);
            start = end;
        }
        arrow_select::concat::concat_batches(&self.output, &batches).map_err(|_| invalid("normalized object batch concatenation failed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn canonical_schema(schema: &Schema) -> Schema {
    Schema::new_with_metadata(schema.fields().iter().map(|field| canonical_field(field)).collect::<Vec<_>>(), schema.metadata().clone())
}

fn canonical_field(field: &Field) -> Field {
    let mut metadata = field.metadata().clone();
    // Source Wire Tags Describe The Original Message, Not A Projected Column.
    metadata.remove("harmos.tag");
    field.clone().with_data_type(canonical_type(field.data_type())).with_nullable(true).with_metadata(metadata)
}

fn canonical_type(data_type: &DataType) -> DataType {
    match data_type {
        DataType::Struct(fields) => {
            DataType::Struct(fields.iter().map(|field| Arc::new(canonical_field(field))).collect::<Vec<_>>().into())
        }
        DataType::List(field) => DataType::List(Arc::new(canonical_field(field))),
        data_type => data_type.clone(),
    }
}

fn canonical_batch(batch: RecordBatch, schema: Arc<Schema>) -> Result<RecordBatch> {
    let columns = batch
        .columns()
        .iter()
        .zip(schema.fields())
        .map(|(column, field)| retarget(column.to_data(), field.data_type()).map(make_array))
        .collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new(schema, columns).map_err(|_| invalid("projected object schema normalization failed"))
}

fn retarget(data: ArrayData, data_type: &DataType) -> Result<ArrayData> {
    let expected = match data_type {
        DataType::Struct(fields) => fields.iter().map(|field| field.data_type()).collect::<Vec<_>>(),
        DataType::List(field) => vec![field.data_type()],
        _ => Vec::new(),
    };
    if expected.len() != data.child_data().len() {
        return Err(invalid("projected nested layout mismatch"));
    }
    let children = data.child_data().iter().zip(expected).map(|(child, kind)| retarget(child.clone(), kind)).collect::<Result<Vec<_>>>()?;
    data.into_builder()
        .data_type(data_type.clone())
        .child_data(children)
        .build()
        .map_err(|_| invalid("projected object array normalization failed"))
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "object query", reason }
}

/* === Tests === */
