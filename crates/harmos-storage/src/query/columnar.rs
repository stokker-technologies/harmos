/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Columnar Capabilities and Bounded Fallback Scans

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;

// Local Imports
use crate::{BackendSnapshot, Error, MemorySize, Result, ScanPage, StorageBackend, StorageSchema};
use harmos_domain::model::storage::{ObjectSelection, ObjectVersions, RecordSelection, SchemaSelection, StreamSelection};
use harmos_query::columnar::{self, ArrowPage, ArrowScan, Cancellation, ColumnKind, ScanEvidence, finish_scan};
use harmos_query::source;
use harmos_query::transform::FieldPath;
use harmos_query::transform::project::{Project, Projection};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageCapabilities {
    pub file_sessions: bool,
    pub file_deletion: bool,
    pub mutation_receipts: bool,
    pub native_arrow: bool,
    pub columnar_append: bool,
    pub native_index: bool,
    pub durable_projections: bool,
    pub conditional_objects: bool,
    pub compound_mutations: bool,
}

struct Columns<'a> {
    schema: &'a StorageSchema,
    metadata: RecordBatch,
    payloads: Vec<&'a [u8]>,
    template: RecordBatch,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Columns<'_> {
    fn load(&self, names: &[String], indices: &[u64]) -> harmos_query::Result<(RecordBatch, usize)> {
        let schema = self.template.schema();
        let fields = names
            .iter()
            .map(|name| schema.field_with_name(name).cloned().map_err(|_| columnar::invalid("select an available column")))
            .collect::<harmos_query::Result<Vec<_>>>()?;
        let project = Project {
            fields: names
                .iter()
                .filter(|name| name.starts_with("payload."))
                .map(|name| Projection {
                    field: FieldPath::from(name.as_str()),
                    name: name.clone(),
                })
                .collect(),
        };
        let mut decoded = 0;
        let payload = if project.fields.is_empty() {
            None
        } else {
            let fields = fields.iter().filter(|field| field.name().starts_with("payload.")).cloned().collect::<Vec<_>>();
            let output = Arc::new(Schema::new(fields));
            let payloads = indices.iter().map(|index| self.payloads[*index as usize]);
            decoded = indices.len();
            Some(source::projected_payloads(self.schema, payloads, &project, output)?)
        };
        let metadata = columnar::take(&self.metadata, indices)?;
        let arrays = names
            .iter()
            .map(|name| {
                metadata
                    .column_by_name(name)
                    .or_else(|| payload.as_ref().and_then(|payload| payload.column_by_name(name)))
                    .cloned()
                    .ok_or_else(|| columnar::invalid("select an available column"))
            })
            .collect::<harmos_query::Result<Vec<_>>>()?;
        let batch =
            RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).map_err(|_| columnar::invalid("selected column batch mismatch"))?;
        Ok((batch, decoded))
    }
}

impl MemorySize for StorageCapabilities {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

pub async fn list_column_rows(
    backend: &dyn StorageBackend, snapshot: &BackendSnapshot, read: &ArrowScan, cancel: &Cancellation,
) -> Result<ArrowPage> {
    cancel.check()?;
    read.position(snapshot)?;
    if backend.capabilities().native_arrow {
        return backend.list_column_rows(snapshot, read, cancel).await;
    }
    fallback(backend, snapshot, read, cancel).await
}

pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage columns", reason }
}
pub(crate) fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "columnar query work" }
}

/* === Private Functions === */

async fn fallback(backend: &dyn StorageBackend, snapshot: &BackendSnapshot, read: &ArrowScan, cancel: &Cancellation) -> Result<ArrowPage> {
    let after = read.position(snapshot)?;
    let schemas = backend
        .list_schemas(
            snapshot,
            &SchemaSelection { schema: Some(read.source.schema.clone()) },
            ScanPage {
                after: 0,
                rows: NonZeroUsize::MIN,
                bytes: read.work.bytes,
            },
        )
        .await?;
    let schema = schemas.first().ok_or_else(|| invalid("schema is unavailable at requested cut"))?;
    let page = ScanPage {
        after,
        rows: read.work.rows.min(read.limits.rows),
        bytes: read.work.bytes,
    };
    match read.source.kind {
        ColumnKind::Records => {
            let selection = RecordSelection {
                schema: read.source.schema.clone(),
                scope: None,
                item: None,
                source_start: None,
                source_end: None,
            };
            let rows = backend.list_records(snapshot, &selection, page).await?;
            let bytes = rows.iter().try_fold(0usize, |total, row| Ok::<_, Error>(total.saturating_add(crate::encoded_size(row)?)))?;
            let refs = rows.iter().collect::<Vec<_>>();
            let metadata = source::record_metadata(schema, &refs, None)?;
            let payloads = rows.iter().map(|row| row.payload.as_slice()).collect();
            scan(snapshot, read, cancel, schema, metadata, payloads, bytes)
        }
        ColumnKind::Stream => {
            let selection = StreamSelection {
                schema: read.source.schema.clone(),
                scope: None,
                item: None,
                stream: None,
                source_start: None,
                source_end: None,
                after: 0,
            };
            let rows = backend.list_samples(snapshot, &selection, page).await?;
            let bytes = rows.iter().try_fold(0usize, |total, row| Ok::<_, Error>(total.saturating_add(crate::encoded_size(row)?)))?;
            let refs = rows.iter().map(|row| &row.record).collect::<Vec<_>>();
            let metadata = source::record_metadata(schema, &refs, Some(&rows))?;
            let payloads = rows.iter().map(|row| row.record.payload.as_slice()).collect();
            scan(snapshot, read, cancel, schema, metadata, payloads, bytes)
        }
        ColumnKind::Objects => {
            let selection = ObjectSelection {
                schema: Some(read.source.schema.clone()),
                key: None,
                versions: ObjectVersions::Latest,
            };
            let rows = backend.list_object_revisions(snapshot, &selection, page).await?;
            let bytes = rows.iter().try_fold(0usize, |total, row| Ok::<_, Error>(total.saturating_add(crate::encoded_size(row)?)))?;
            let metadata = source::object_metadata(&rows)?;
            let payloads = rows.iter().map(|row| row.value.payload.as_slice()).collect();
            scan(snapshot, read, cancel, schema, metadata, payloads, bytes)
        }
    }
}

fn scan(
    snapshot: &BackendSnapshot, read: &ArrowScan, cancel: &Cancellation, schema: &StorageSchema, metadata: RecordBatch,
    payloads: Vec<&[u8]>, bytes: usize,
) -> Result<ArrowPage> {
    cancel.check()?;
    let count = metadata.num_rows();
    read.work.check(count, bytes)?;
    let template = columnar::empty(schema, read.source.kind)?;
    for name in read.required_columns() {
        template.schema().index_of(&name).map_err(|_| invalid("select an available column"))?;
    }
    if let Some(predicate) = &read.predicate {
        let field =
            template.schema().field_with_name(&predicate.column).cloned().map_err(|_| invalid("select an available predicate column"))?;
        predicate.validate_type(field.data_type())?;
    }
    let columns = Columns { schema, metadata, payloads, template };
    let mut names = vec!["_storage.position".to_owned()];
    if let Some(predicate) = &read.predicate {
        names.push(predicate.column.clone());
    }
    if let Some(time) = &read.time {
        names.push(time.column.clone());
    }
    names.sort();
    names.dedup();
    let indices = (0..count as u64).collect::<Vec<_>>();
    let (filter, payload_rows) = columns.load(&names, &indices)?;
    read.work.check(count, filter.get_array_memory_size())?;
    Ok(finish_scan(
        snapshot,
        read,
        &filter,
        ScanEvidence {
            complete: count == 0,
            bytes,
            columns: read.required_columns(),
            payload_rows,
        },
        cancel,
        |indices| {
            cancel.check()?;
            let (batch, decoded) = columns.load(&read.columns, indices)?;
            read.work.check(count, filter.get_array_memory_size().saturating_add(batch.get_array_memory_size()))?;
            Ok((batch, decoded))
        },
    )?)
}

/* === Tests === */
