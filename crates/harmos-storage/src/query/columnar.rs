use crate::{BackendSnapshot, Error, MemorySize, Result, ScanPage, StorageBackend};
use harmos_domain::model::storage::{ObjectSelection, ObjectVersions, RecordSelection, SchemaSelection, StreamSelection};
use harmos_query::columnar::{ArrowPage, ArrowScan, Cancellation, ColumnKind, ScanEvidence, finish_page, flatten};
use std::num::NonZeroUsize;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageCapabilities {
    pub file_sessions: bool,
    pub file_deletion: bool,
    pub mutation_receipts: bool,
    pub native_arrow: bool,
    pub native_index: bool,
    pub durable_projections: bool,
    pub conditional_objects: bool,
}
impl MemorySize for StorageCapabilities {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

use harmos_query::source;
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
    let (batch, bytes, count) = match read.source.kind {
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
            (source::records(schema, rows.iter())?, bytes, rows.len())
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
            (source::stream(schema, &rows)?, bytes, rows.len())
        }
        ColumnKind::Objects => {
            let selection = ObjectSelection {
                schema: Some(read.source.schema.clone()),
                key: None,
                versions: ObjectVersions::Latest,
            };
            let rows = backend.list_object_revisions(snapshot, &selection, page).await?;
            let bytes = rows.iter().try_fold(0usize, |total, row| Ok::<_, Error>(total.saturating_add(crate::encoded_size(row)?)))?;
            (source::objects(schema, &rows)?, bytes, rows.len())
        }
    };
    cancel.check()?;
    read.work.check(count, bytes)?;
    let flat = flatten(&batch)?;
    if flat.get_array_memory_size() > read.work.bytes.get() {
        return Err(exhausted());
    }
    Ok(finish_page(
        snapshot,
        read,
        &flat,
        ScanEvidence {
            complete: count == 0,
            bytes,
            columns: read.required_columns(),
            payload_rows: count,
        },
        cancel,
    )?)
}

pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage columns", reason }
}
pub(crate) fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "columnar query work" }
}
