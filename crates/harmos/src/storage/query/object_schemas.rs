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
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
use arrow_array::{RecordBatch, make_array};
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::transform::{Transform, project::Project};
use super::{ObjectSelection, Source};
use crate::storage::arrow::{RetainedMemory, resident_bytes};
use crate::storage::{ObjectEntry, QueryValue, ReadLimits, StorageSchema, StorageView};
use crate::{Error, MemorySize, Result};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// Resolved schemas for one finite object selection. A leading projection may
/// normalize heterogeneous payloads before the common query pipeline begins.
pub(crate) struct NormalizedObjects {
    schemas: Vec<QueryValue<StorageSchema>>,
    projection: Option<Project>,
    output: Arc<Schema>,
    _metadata: Vec<RetainedMemory>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl NormalizedObjects {
    pub(crate) async fn resolve(
        view: &StorageView,
        selection: &ObjectSelection,
        transforms: &mut Vec<Transform>,
        page: ReadLimits,
    ) -> Result<Self> {
        let manager = view.manager();
        let mut schemas: Vec<QueryValue<StorageSchema>> = Vec::new();
        let mut metadata = Vec::new();
        if let Some(key) = &selection.schema {
            schemas.push(manager.lookup_schema(view, key).await?);
        } else {
            // Inspect Actual Selected Objects, Not Unrelated Schema Registrations.
            let selected = Source::Objects(selection.clone()).selection()?;
            let mut reader = view.read_objects(selected)?;
            let mut credit = None;
            loop {
                let Some(rows) = reader.stage_page(page).await? else {
                    break;
                };
                let bytes = rows
                    .iter()
                    .enumerate()
                    .try_fold(0usize, |total, (index, row)| {
                        if schemas.iter().any(|schema| schema.key == row.value.schema)
                            || rows
                                .iter()
                                .take(index)
                                .any(|prior| prior.value.schema == row.value.schema)
                        {
                            return Ok(total);
                        }
                        row.value
                            .schema
                            .memory_size()
                            .checked_mul(4)
                            .and_then(|bytes| bytes.checked_add(1024))
                            .and_then(|bytes| total.checked_add(bytes))
                            .ok_or_else(exhausted)
                    })?;
                if bytes == 0 {
                    reader.commit_staged_page();
                    drop(rows);
                    drop(credit.take());
                    continue;
                }
                let working = bytes
                    .checked_add(crate::storage::manager::ENTRY_OVERHEAD)
                    .ok_or_else(exhausted)?;
                if credit.is_none() {
                    match view.try_reserve_working(working) {
                        Ok(admitted) => credit = Some(admitted),
                        Err(Error::ResourceExhausted { .. }) => {
                            let peak = rows
                                .reserved_bytes()
                                .checked_add(working)
                                .ok_or_else(exhausted)?;
                            drop(rows);
                            reader.discard_staged_page();
                            credit = Some(view.reserve_working_wait(peak).await?);
                            continue;
                        }
                        Err(error) => return Err(error),
                    }
                }
                let reservation =
                    crate::storage::manager::retain_memory(view.reserve_read(bytes, 1)?);
                let mut keys = Vec::new();
                for row in rows.iter() {
                    if schemas.iter().any(|schema| schema.key == row.value.schema)
                        || keys.iter().any(|key| key == &row.value.schema)
                    {
                        continue;
                    }
                    keys.push(row.value.schema.clone());
                }
                reader.commit_staged_page();
                // Only The Small, Admitted Key Descriptions Survive Schema I/O.
                // Raw Pages Are Reloadable And Must Never Occupy A Wait Cycle.
                drop(rows);
                drop(credit.take());
                metadata.push(reservation);
                for key in keys {
                    schemas.push(manager.lookup_schema(view, &key).await?);
                }
            }
        }
        if schemas.is_empty() {
            return Err(invalid(
                "empty untyped object selection requires an explicit schema",
            ));
        }
        let projection = match transforms.first() {
            Some(Transform::Project(project)) => Some(project.clone()),
            _ => None,
        };
        let schema_bytes = schemas.iter().try_fold(0usize, |total, schema| {
            total
                .checked_add((**schema).memory_size())
                .ok_or_else(exhausted)
        })?;
        let allowance = schema_bytes
            .checked_mul(64)
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or_else(exhausted)?;
        let construction =
            crate::storage::manager::retain_memory(view.reserve_read_wait(allowance, 1).await?);
        let mut output: Option<Arc<Schema>> = None;
        for schema in &schemas {
            let empty = super::source::objects(schema, &[], allowance)?;
            let candidate = match &projection {
                Some(project) => canonical_schema(&project.output_schema(empty.schema().as_ref())?),
                None => empty.schema().as_ref().clone(),
            };
            crate::storage::arrow::validate_schema(&candidate)?;
            if output
                .as_ref()
                .is_some_and(|schema| schema.as_ref() != &candidate)
            {
                return Err(invalid(
                    "selected object schemas require a leading projection to compatible fields",
                ));
            }
            output.get_or_insert_with(|| Arc::new(candidate));
        }
        if projection.is_some() {
            transforms.remove(0);
        }
        let output = output.expect("nonempty selected schemas");
        let retained = crate::storage::arrow::schema_memory(&output)
            .checked_mul(2)
            .and_then(|bytes| {
                projection
                    .as_ref()
                    .map_or(0, MemorySize::memory_size)
                    .checked_mul(2)
                    .and_then(|projection| bytes.checked_add(projection))
            })
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(exhausted)?;
        construction.shrink_to(retained)?;
        metadata.push(construction);
        Ok(Self {
            schemas,
            projection,
            output,
            _metadata: metadata,
        })
    }

    pub(crate) fn empty(&self) -> RecordBatch {
        RecordBatch::new_empty(Arc::clone(&self.output))
    }

    pub(crate) fn memory_size(&self) -> usize {
        self.schemas
            .iter()
            .map(|schema| (**schema).memory_size())
            .fold(0usize, usize::saturating_add)
            .saturating_add(crate::storage::arrow::schema_memory(&self.output))
            .saturating_add(self.projection.as_ref().map_or(0, MemorySize::memory_size))
    }

    pub(crate) fn map(&self, rows: &[ObjectEntry], allowance: usize) -> Result<RecordBatch> {
        if rows.is_empty() {
            return Ok(self.empty());
        }
        let mut batches = Vec::new();
        let mut retained = 0usize;
        let mut start = 0usize;
        // Contiguous Runs Preserve The Original Capture Ordering Across Schemas.
        while start < rows.len() {
            let key = &rows[start].value.schema;
            let end = rows[start..]
                .iter()
                .position(|row| &row.value.schema != key)
                .map_or(rows.len(), |length| start + length);
            let schema = self
                .schemas
                .iter()
                .find(|schema| &schema.key == key)
                .ok_or_else(|| {
                    invalid("object schema was not resolved at the requested boundary")
                })?;
            let remaining = allowance.checked_sub(retained).ok_or_else(exhausted)?;
            let source = super::source::objects(schema, &rows[start..end], remaining / 3)?;
            let projected = match &self.projection {
                Some(project) => canonical_batch(
                    project.apply(&source, &mut || Ok(()))?,
                    Arc::clone(&self.output),
                )?,
                None => source,
            };
            if projected.schema() != self.output {
                return Err(invalid("normalized object schema changed"));
            }
            retained = retained
                .checked_add(resident_bytes(&projected)?)
                .ok_or_else(exhausted)?;
            if retained
                .checked_mul(3)
                .is_none_or(|bytes| bytes > allowance)
            {
                return Err(exhausted());
            }
            batches.push(projected);
            start = end;
        }
        arrow_select::concat::concat_batches(&self.output, &batches)
            .map_err(|_| invalid("normalized object batch concatenation failed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn canonical_schema(schema: &Schema) -> Schema {
    Schema::new_with_metadata(
        schema
            .fields()
            .iter()
            .map(|field| canonical_field(field))
            .collect::<Vec<_>>(),
        schema.metadata().clone(),
    )
}

fn canonical_field(field: &Field) -> Field {
    let mut metadata = field.metadata().clone();
    // Source Wire Tags Describe The Original Message, Not A Projected Column.
    metadata.remove("harmos.tag");
    field
        .clone()
        .with_data_type(canonical_type(field.data_type()))
        .with_nullable(true)
        .with_metadata(metadata)
}

fn canonical_type(data_type: &DataType) -> DataType {
    match data_type {
        DataType::Struct(fields) => DataType::Struct(
            fields
                .iter()
                .map(|field| Arc::new(canonical_field(field)))
                .collect::<Vec<_>>()
                .into(),
        ),
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
    RecordBatch::try_new(schema, columns)
        .map_err(|_| invalid("projected object schema normalization failed"))
}

fn retarget(data: ArrayData, data_type: &DataType) -> Result<ArrayData> {
    let expected = match data_type {
        DataType::Struct(fields) => fields
            .iter()
            .map(|field| field.data_type())
            .collect::<Vec<_>>(),
        DataType::List(field) => vec![field.data_type()],
        _ => Vec::new(),
    };
    if expected.len() != data.child_data().len() {
        return Err(invalid("projected nested layout mismatch"));
    }
    let children = data
        .child_data()
        .iter()
        .zip(expected)
        .map(|(child, kind)| retarget(child.clone(), kind))
        .collect::<Result<Vec<_>>>()?;
    data.into_builder()
        .data_type(data_type.clone())
        .child_data(children)
        .build()
        .map_err(|_| invalid("projected object array normalization failed"))
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "object query",
        reason,
    }
}
fn exhausted() -> Error {
    Error::ResourceExhausted {
        resource: "object schema normalization",
    }
}

/* === Tests === */
