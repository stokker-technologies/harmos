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

use crate::StorageCursor;
use harmos_query::Error as QueryError;

use super::object_schemas::NormalizedObjects;
use super::reader::{QueryReader, query_slot, schema_at};
use crate::backend::ScanPage;
use crate::config::SCAN_ROWS;
use crate::manager::Inner;
use crate::selection::Entry;
use crate::{Error, FileReference, ObjectEntry, Result, Snapshot, StorageManager, StorageSchema, StoredRecord, StoredRow};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use harmos_domain::model::storage::query::{Query, Source};
use harmos_query::QueryResponse;
use harmos_query::batches::{BatchPage, BatchReader, BatchScan, BatchSource, QueryFuture};
use harmos_query::transform::{ProjectExecution, Transform, project::Project};
use std::num::NonZeroUsize;
use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct StoredBatches {
    inner: Arc<Inner>,
    snapshot: Arc<Snapshot>,
    selection: Source,
    schema: Option<StorageSchema>,
    objects: Option<NormalizedObjects>,
    projection: Option<(Project, SchemaRef)>,
    final_projection: Option<Project>,
    output: SchemaRef,
    after: u64,
    rows: NonZeroUsize,
    bytes: NonZeroUsize,
    done: bool,
}

/* === Enums === */

enum Rows {
    Objects(Vec<ObjectEntry>),
    Records(Vec<StoredRecord>),
    RecordViews(Vec<crate::RecordView>),
    Stream(Vec<StoredRow>),
    Schemas(Vec<StorageSchema>),
    Files(Vec<FileReference>),
}

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    pub async fn query(&self, query: Query) -> Result<QueryReader> {
        let snapshot = Arc::new(self.snapshot(&query.at).await?);
        self.query_snapshot(query, snapshot).await
    }

    pub async fn query_snapshot(&self, mut query: Query, snapshot: Arc<Snapshot>) -> Result<QueryReader> {
        query.validate()?;
        let active = query_slot(&self.inner).await?;
        if !Arc::ptr_eq(&self.inner, &snapshot.inner) || snapshot.at() != &query.at {
            return Err(Error::InvalidInput {
                field: "open query",
                reason: "query must use its retained owner's exact cut",
            });
        }
        let projection = match query.transforms.first().filter(|_| query.merge.is_none()) {
            Some(Transform::Project(project)) => Some(project.clone()),
            _ => None,
        };
        let scan = BatchScan {
            limits: crate::ReadLimits {
                rows: SCAN_ROWS,
                bytes: self.inner.settings.scan_bytes(),
            },
            projection,
            pruning: None,
        };
        let skipped = if query.merge.is_none()
            && query.slice.offset > 0
            && query.transforms.iter().all(|transform| matches!(transform, Transform::Project(_)))
        {
            match &query.source {
                Source::Records(selection) => snapshot.get_record_position(selection, query.slice.offset).await?,
                _ => None,
            }
        } else {
            None
        };
        let mut source = StoredBatches::open(Arc::clone(&self.inner), Arc::clone(&snapshot), query.source.clone(), &scan).await?;
        if let Some(after) = skipped {
            source.after = after;
            query.slice.offset = 0;
        }
        if query.merge.is_none()
            && query.transforms.iter().all(|transform| matches!(transform, Transform::Project(_)))
            && let Some(limit) = query.slice.limit
        {
            let rows = query.slice.offset.saturating_add(limit).min(source.rows.get() as u64) as usize;
            if let Some(rows) = NonZeroUsize::new(rows) {
                source.rows = rows;
            } else {
                source.done = true;
            }
        }
        let output = QueryResponse {
            batch_rows: self.inner.settings.response.batch_rows,
            batch_bytes: self.inner.settings.response.batch_bytes,
        };
        let secondary = match &query.merge {
            Some(merge) => Some(Box::new(StoredBatches::open(Arc::clone(&self.inner), snapshot, merge.source.clone(), &scan).await?)
                as Box<dyn BatchReader>),
            None => None,
        };
        let reader =
            harmos_query::QueryReader::from_sources(Box::new(source), secondary, query, scan.projection.is_some(), output, scan.limits)?;
        Ok(QueryReader::new(Arc::clone(&self.inner), reader, active))
    }
}

impl BatchSource for StorageManager {
    fn open<'a>(&'a self, selection: &'a Source, at: &'a StorageCursor, scan: &'a BatchScan) -> QueryFuture<'a, Box<dyn BatchReader>> {
        Box::pin(async move {
            let active = query_slot(&self.inner).await.map_err(source_error)?;
            let snapshot = Arc::new(self.snapshot(at).await.map_err(source_error)?);
            let source = StoredBatches::open(Arc::clone(&self.inner), snapshot, selection.clone(), scan).await.map_err(source_error)?;
            drop(active);
            Ok(Box::new(AdmittedBatches { source, inner: Arc::clone(&self.inner) }) as Box<dyn BatchReader>)
        })
    }
}

impl StoredBatches {
    async fn open(inner: Arc<Inner>, snapshot: Arc<Snapshot>, selection: Source, scan: &BatchScan) -> Result<Self> {
        let after = match &selection {
            Source::Stream(value) => value.after,
            _ => 0,
        };
        let mut reader = Self {
            inner,
            snapshot,
            selection,
            schema: None,
            objects: None,
            projection: None,
            final_projection: None,
            output: Arc::new(arrow_schema::Schema::empty()),
            after,
            rows: scan.limits.rows.min(SCAN_ROWS),
            bytes: scan.limits.bytes,
            done: false,
        };
        let mut transforms = scan.projection.clone().map(Transform::Project).into_iter().collect::<Vec<_>>();
        match &reader.selection {
            Source::Objects(selection) => {
                reader.objects = Some(NormalizedObjects::resolve(&reader.inner, &reader.snapshot, selection, &mut transforms).await?);
            }
            Source::Records(selection) => reader.schema = Some(schema_at(&reader.inner, &reader.snapshot, &selection.schema).await?),
            Source::Stream(selection) => reader.schema = Some(schema_at(&reader.inner, &reader.snapshot, &selection.schema).await?),
            Source::Files(_) | Source::Schemas(_) => {}
        }
        let mut empty = reader.convert(None)?;
        if let Some(Transform::Project(project)) = transforms.pop() {
            empty = project.apply(&empty, &mut || Ok(()))?;
            if matches!(reader.selection, Source::Records(_) | Source::Stream(_))
                && project.fields.iter().all(|field| field.field.fields.first().is_some_and(|name| name == "payload"))
            {
                reader.projection = Some((project, empty.schema()));
            } else {
                reader.final_projection = Some(project);
            }
        }
        reader.output = empty.schema();
        Ok(reader)
    }

    async fn advance(&mut self) -> Result<BatchPage> {
        if self.done {
            return Ok(BatchPage {
                batch: RecordBatch::new_empty(Arc::clone(&self.output)),
                complete: true,
            });
        }
        let page = ScanPage {
            after: self.after,
            rows: self.rows,
            bytes: self.bytes.min(self.inner.settings.scan_bytes()),
        };
        let snapshot = &self.snapshot;
        let (rows, _) = match &self.selection {
            Source::Objects(selection) => scan(snapshot, selection, page, Rows::Objects).await?,
            Source::Records(selection) => match snapshot.record_views(selection, page).await? {
                Some(views) => (Rows::RecordViews(views), 0),
                None => scan(snapshot, selection, page, Rows::Records).await?,
            },
            Source::Stream(selection) => scan(snapshot, selection, page, Rows::Stream).await?,
            Source::Schemas(selection) => scan(snapshot, selection, page, Rows::Schemas).await?,
            Source::Files(selection) => scan(snapshot, selection, page, Rows::Files).await?,
        };
        let Some(last) = rows.last() else {
            self.done = true;
            return Ok(BatchPage {
                batch: RecordBatch::new_empty(Arc::clone(&self.output)),
                complete: true,
            });
        };
        let batch = self.convert(Some(&rows))?;
        self.after = last;
        Ok(BatchPage { batch, complete: false })
    }
    fn convert(&self, rows: Option<&Rows>) -> Result<RecordBatch> {
        let schema = self.schema.as_ref();
        let batch = match (&self.selection, rows) {
            (Source::Objects(_), rows) => {
                let objects = self.objects.as_ref().expect("resolved object schemas");
                match rows {
                    Some(Rows::Objects(rows)) => objects.map(rows).map_err(source_error),
                    _ => Ok::<_, QueryError>(objects.empty()),
                }
            }
            (Source::Records(_), Some(Rows::Records(rows))) => match &self.projection {
                Some((project, output)) => {
                    harmos_query::source::projected(schema.expect("record schema"), rows.iter(), project, output.clone())
                }
                None => harmos_query::source::records(schema.expect("record schema"), rows.iter()),
            },
            (Source::Records(_), Some(Rows::RecordViews(rows))) => {
                let records = rows.iter().map(crate::RecordView::record);
                match &self.projection {
                    Some((project, output)) => {
                        harmos_query::source::projected(schema.expect("record schema"), records, project, output.clone())
                    }
                    None => harmos_query::source::records(schema.expect("record schema"), records),
                }
            }
            (Source::Records(_), _) => harmos_query::source::records(schema.expect("record schema"), std::iter::empty()),
            (Source::Stream(_), Some(Rows::Stream(rows))) => match &self.projection {
                Some((project, output)) => {
                    let records = rows.iter().map(|row| &row.record);
                    harmos_query::source::projected(schema.expect("stream schema"), records, project, output.clone())
                }
                None => harmos_query::source::stream(schema.expect("stream schema"), rows),
            },
            (Source::Stream(_), _) => harmos_query::source::stream(schema.expect("stream schema"), &[]),
            (Source::Schemas(_), Some(Rows::Schemas(rows))) => harmos_query::source::schemas(rows),
            (Source::Schemas(_), _) => harmos_query::source::schemas(&[]),
            (Source::Files(_), Some(Rows::Files(rows))) => harmos_query::source::files(rows),
            (Source::Files(_), _) => harmos_query::source::files(&[]),
        }?;
        match &self.final_projection {
            Some(project) => Ok(project.apply(&batch, &mut || Ok(()))?),
            None => Ok(batch),
        }
    }
}

impl Rows {
    fn last(&self) -> Option<u64> {
        let position = match self {
            Self::Objects(rows) => rows.last().map(Entry::position),
            Self::Records(rows) => rows.last().map(Entry::position),
            Self::RecordViews(rows) => rows.last().map(|view| &view.record().position),
            Self::Stream(rows) => rows.last().map(Entry::position),
            Self::Schemas(rows) => rows.last().map(Entry::position),
            Self::Files(rows) => rows.last().map(Entry::position),
        };
        position.map(|position| position.sequence)
    }
}

impl BatchReader for StoredBatches {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.output)
    }
    fn at(&self) -> &StorageCursor {
        self.snapshot.at()
    }
    fn next(&mut self) -> QueryFuture<'_, BatchPage> {
        Box::pin(async move { self.advance().await.map_err(source_error) })
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn scan<T: Entry>(snapshot: &Snapshot, selection: &T::Selection, page: ScanPage, wrap: fn(Vec<T>) -> Rows) -> Result<(Rows, usize)> {
    let (entries, bytes) = snapshot.scan::<T>(selection, page).await?;
    Ok((wrap(entries), bytes))
}

fn source_error(error: Error) -> QueryError {
    QueryError::backend("read batch source", harmos_query::BackendFailure::Permanent, error)
}

/* === Tests === */

struct AdmittedBatches {
    source: StoredBatches,
    inner: Arc<Inner>,
}
impl BatchReader for AdmittedBatches {
    fn schema(&self) -> SchemaRef {
        self.source.schema()
    }
    fn at(&self) -> &StorageCursor {
        self.source.at()
    }
    fn next(&mut self) -> QueryFuture<'_, BatchPage> {
        Box::pin(async move {
            let _active = query_slot(&self.inner).await.map_err(source_error)?;
            self.source.next().await
        })
    }
}
