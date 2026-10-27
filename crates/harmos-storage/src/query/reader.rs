/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Query Result Reader

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Third Party Imports
use arrow_array::RecordBatch;
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::execute::Plan;
use super::object_schemas::NormalizedObjects;
use super::{QueryCompletion, QueryOutput, Source};
use crate::storage::arrow::{ArrowBatch, ArrowSchema};
use crate::storage::backend::ScanPage;
use crate::storage::config::SCAN_ROWS;
use crate::storage::manager::Inner;
use crate::storage::selection::Entry;
use crate::storage::snapshot::Snapshot;
use crate::storage::{FileReference, ObjectEntry, SchemaKey, StorageManager, StorageSchema, StoredRecord, StoredRow};
use crate::{Error, MemorySize, Result};
use harmos_domain::model::storage::ArrowIpc;
use harmos_domain::operations::storage::Query;

/* === Constants === */

// Two Protobuf Keys (Five Bytes Each) And Two Length Varints (Ten Bytes Each),
// Less The Postcard Variant And Length Bytes They Replace.
const PROTOBUF_FRAMING: usize = 30;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct QueryReader {
    context: crate::execution::Context,
    inner: Arc<Inner>,
    query: Query,
    snapshot: Option<Snapshot>,
    active: Option<OwnedSemaphorePermit>,
    schema: Option<StorageSchema>,
    objects: Option<NormalizedObjects>,
    plan: Option<Plan>,
    after: u64,
    rows: NonZeroUsize,
    source_done: bool,
    pending: Option<RecordBatch>,
    pending_offset: usize,
    delivered: u64,
    exhausted: bool,
    failed: bool,
    inflight: bool,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum QueryArrow {
    Schema(ArrowSchema),
    Batch(ArrowBatch),
}
struct CancelWork(Arc<AtomicBool>);

impl Drop for CancelWork {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

enum Rows {
    Objects(Vec<ObjectEntry>),
    Records(Vec<StoredRecord>),
    Stream(Vec<StoredRow>),
    Schemas(Vec<StorageSchema>),
    Files(Vec<FileReference>),
}

/* === Traits === */

/* === Implementations === */

impl QueryArrow {
    /// Bounds The Transport Value Before IPC Conversion.
    pub fn output_memory_size(&self) -> usize {
        let capacity = match self {
            Self::Schema(schema) => schema.ipc_capacity(),
            Self::Batch(batch) => batch.ipc_capacity(),
        };
        std::mem::size_of::<QueryOutput>().saturating_add(capacity)
    }

    #[tracing::instrument(name = "storage.query.reader.output", level = "trace", skip_all)]
    pub fn output(self) -> QueryOutput {
        match self {
            Self::Schema(schema) => QueryOutput::Schema(ArrowIpc::from(schema)),
            Self::Batch(batch) => QueryOutput::Batch(ArrowIpc::from(batch)),
        }
    }
}

impl TryFrom<QueryOutput> for QueryArrow {
    type Error = Error;

    fn try_from(output: QueryOutput) -> Result<Self> {
        Ok(match output {
            QueryOutput::Schema(ipc) => Self::Schema(ArrowSchema::from_ipc(ipc.into_bytes())?),
            QueryOutput::Batch(ipc) => Self::Batch(ArrowBatch::from_ipc(ipc.into_bytes())?),
        })
    }
}

impl MemorySize for QueryArrow {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Schema(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Batch(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
            }
    }
}
impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.query", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub async fn query(&self, query: Query) -> Result<QueryReader> {
        let result = async {
            query.validate()?;
            self.inner.primary.history().check(&query.at.history)?;

            let active = query_slot(&self.inner).await?;

            // Pin The Requested Image For The Whole Invocation.
            let snapshot = self.snapshot(&query.at).await?;
            let after = match &query.source {
                Source::Stream(selection) => selection.after,
                _ => 0,
            };
            Ok(QueryReader {
                context: crate::execution::Context::current(),
                inner: Arc::clone(&self.inner),
                query,
                snapshot: Some(snapshot),
                active: Some(active),
                schema: None,
                objects: None,
                plan: None,
                after,
                rows: SCAN_ROWS,
                source_done: false,
                pending: None,
                pending_offset: 0,
                delivered: 0,
                exhausted: false,
                failed: false,
                inflight: false,
            })
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }
}

impl QueryReader {
    pub async fn next(&mut self) -> Result<Option<QueryArrow>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page"));
        let future = async {
            if self.failed || self.inflight {
                self.fail();
                return Err(Error::NotReady { domain: "failed or interrupted storage query" });
            }
            self.inflight = true;
            let result = self.advance().await;
            self.inflight = false;
            if result.is_err() {
                self.fail();
            }
            result
        };
        context.scope(tracing::Instrument::instrument(future, span)).await
    }

    #[tracing::instrument(name = "storage.query.reader.completion", level = "trace", skip_all)]
    pub fn completion(&self) -> Result<QueryCompletion> {
        if !self.exhausted || self.failed || self.inflight {
            return Err(Error::NotReady { domain: "storage query completion" });
        }
        Ok(QueryCompletion { at: self.query.at.clone(), rows: self.delivered })
    }

    fn fail(&mut self) {
        self.failed = true;
        self.pending = None;
        self.plan = None;
        self.schema = None;
        self.objects = None;
        self.release();
    }

    // Release Admission And Any Remaining Snapshot At Completion Or Failure.
    fn release(&mut self) {
        self.snapshot = None;
        self.active = None;
    }

    #[tracing::instrument(name = "storage.query.reader.advance", level = "trace", skip_all)]
    async fn advance(&mut self) -> Result<Option<QueryArrow>> {
        if self.exhausted {
            return Ok(None);
        }
        if self.plan.is_none() {
            let schema = self.prepare().await?;
            return Ok(Some(QueryArrow::Schema(schema)));
        }
        loop {
            if let Some(batch) = self.deliver().await? {
                return Ok(Some(QueryArrow::Batch(batch)));
            }
            let plan = self.plan.as_ref().expect("prepared plan");
            if plan.is_satisfied() {
                self.source_done = true;
            }
            if !self.source_done {
                self.next_page().await?;
                continue;
            }
            self.snapshot = None;
            if !self.finish().await? {
                self.release();
                self.exhausted = true;
                return Ok(None);
            }
        }
    }

    // Resolve Source Schemas, Validate The Plan And Fix The Output Schema.
    #[tracing::instrument(name = "storage.query.reader.prepare", level = "trace", skip_all)]
    async fn prepare(&mut self) -> Result<ArrowSchema> {
        let snapshot = self.snapshot.as_ref().expect("prepared query owns its snapshot");
        let mut transforms = std::mem::take(&mut self.query.transforms);
        match &self.query.source {
            Source::Objects(selection) => {
                let objects = NormalizedObjects::resolve(&self.inner, snapshot, selection, &mut transforms).await?;
                self.objects = Some(objects);
            }
            Source::Records(selection) => {
                let schema = schema_at(&self.inner, snapshot, &selection.schema).await?;
                self.schema = Some(schema);
            }
            Source::Stream(selection) => {
                let schema = schema_at(&self.inner, snapshot, &selection.schema).await?;
                self.schema = Some(schema);
            }
            Source::Schemas(_) | Source::Files(_) => {}
        }

        let empty = self.convert(None)?;
        let plan = Plan::new(empty.schema(), transforms, self.query.slice)?;
        let schema = ArrowSchema::try_new(plan.output_schema())?;
        self.plan = Some(plan);
        Ok(schema)
    }

    // Load, Convert And Process One Source Page As One Reloadable Attempt.
    #[tracing::instrument(name = "storage.query.reader.next_page", level = "trace", skip_all, fields(after = self.after, rows = self.rows.get()))]
    async fn next_page(&mut self) -> Result<()> {
        self.page_attempt().await
    }

    #[tracing::instrument(name = "storage.query.reader.page_attempt", level = "trace", skip_all)]
    async fn page_attempt(&mut self) -> Result<()> {
        let scan_bytes = self.inner.settings.scan_bytes();
        let page = ScanPage { after: self.after, rows: self.rows, bytes: scan_bytes };

        let snapshot = self.snapshot.as_ref().expect("running query owns its snapshot");

        let (rows, _) = match &self.query.source {
            Source::Objects(selection) => scan(snapshot, selection, page, Rows::Objects).await?,
            Source::Records(selection) => scan(snapshot, selection, page, Rows::Records).await?,
            Source::Stream(selection) => scan(snapshot, selection, page, Rows::Stream).await?,
            Source::Schemas(selection) => scan(snapshot, selection, page, Rows::Schemas).await?,
            Source::Files(selection) => scan(snapshot, selection, page, Rows::Files).await?,
        };

        let Some(last) = rows.last() else {
            self.source_done = true;
            return Ok(());
        };

        // Advance Only After The Complete Page Converts.
        let batch = self.convert(Some(&rows))?;
        drop(rows);
        let output = self.work(Some(batch)).await?;
        self.pending = output.filter(|output| output.num_rows() != 0);
        self.pending_offset = 0;
        self.after = last;
        Ok(())
    }

    // Produce The Global Output Once; `false` When Nothing Remains.
    #[tracing::instrument(name = "storage.query.reader.finish", level = "trace", skip_all)]
    async fn finish(&mut self) -> Result<bool> {
        if self.plan.is_none() {
            return Ok(false);
        }
        match self.work(None).await? {
            Some(output) if output.num_rows() != 0 => {
                self.pending = Some(output);
                self.pending_offset = 0;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    // A Dropped Caller Cancels Blocking Work Before Its Next Bounded Step.
    async fn work(&mut self, batch: Option<RecordBatch>) -> Result<Option<RecordBatch>> {
        let mut plan = self.plan.take().expect("prepared query plan");
        let cancelled = Arc::new(AtomicBool::new(false));
        let guard = CancelWork(Arc::clone(&cancelled));
        let context = crate::execution::Context::current();
        let result = tokio::task::spawn_blocking(move || {
            context.blocking(|| {
                let mut check = || {
                    if cancelled.load(Ordering::Acquire) { Err(Error::Closing { domain: "cancelled storage query" }) } else { Ok(()) }
                };
                let output = match batch {
                    Some(batch) => plan.page(&batch, &mut check),
                    None => plan.finish(&mut check),
                }?;
                Ok::<_, Error>((plan, output))
            })
        })
        .await
        .map_err(|error| Error::backend("query worker", crate::BackendFailure::Permanent, error))??;
        drop(guard);
        self.plan = Some(result.0);
        Ok(result.1)
    }

    // Split Pending Output Into Response Batches Within Both Ceilings.
    #[tracing::instrument(name = "storage.query.reader.deliver", level = "trace", skip_all)]
    async fn deliver(&mut self) -> Result<Option<ArrowBatch>> {
        let Some(batch) = &self.pending else {
            return Ok(None);
        };
        if self.pending_offset >= batch.num_rows() {
            self.pending = None;
            self.pending_offset = 0;
            return Ok(None);
        }
        let settings = self.inner.settings.response;
        let mut rows = settings.batch_rows.get().min(batch.num_rows() - self.pending_offset);
        loop {
            let slice = batch.slice(self.pending_offset, rows);
            let output = ArrowBatch::try_new(slice)?;

            // The Wire Output Adds Its Envelope Around The Arrow Bytes Field.
            let envelope = QueryOutput::Batch(ArrowIpc::new(output.ipc_bytes().to_vec()));
            // Protobuf Frames This One-Variant Bytes Envelope With Two Keys And Two
            // Length Prefixes Beyond The Postcard Bytes; Bound Both Encodings.
            let encoded = crate::Message::encoded_len(&envelope).saturating_add(PROTOBUF_FRAMING);
            drop(envelope);
            if encoded <= settings.batch_bytes.get() {
                self.pending_offset += rows;
                self.delivered += rows as u64;
                return Ok(Some(output));
            }
            if rows == 1 {
                return Err(Error::ResourceExhausted { resource: "oversized query row" });
            }
            rows /= 2;
        }
    }

    #[tracing::instrument(name = "storage.query.reader.convert", level = "trace", skip_all)]
    fn convert(&self, rows: Option<&Rows>) -> Result<RecordBatch> {
        let schema = self.schema.as_ref();
        match (&self.query.source, rows) {
            (Source::Objects(_), rows) => {
                let objects = self.objects.as_ref().expect("resolved object schemas");
                match rows {
                    Some(Rows::Objects(rows)) => objects.map(rows),
                    _ => Ok(objects.empty()),
                }
            }
            (Source::Records(_), Some(Rows::Records(rows))) => super::source::records(schema.expect("record schema"), rows),
            (Source::Records(_), _) => super::source::records(schema.expect("record schema"), &[]),
            (Source::Stream(_), Some(Rows::Stream(rows))) => super::source::stream(schema.expect("stream schema"), rows),
            (Source::Stream(_), _) => super::source::stream(schema.expect("stream schema"), &[]),
            (Source::Schemas(_), Some(Rows::Schemas(rows))) => super::source::schemas(rows),
            (Source::Schemas(_), _) => super::source::schemas(&[]),
            (Source::Files(_), Some(Rows::Files(rows))) => super::source::files(rows),
            (Source::Files(_), _) => super::source::files(&[]),
        }
    }
}

impl Rows {
    fn last(&self) -> Option<u64> {
        let position = match self {
            Self::Objects(rows) => rows.last().map(Entry::position),
            Self::Records(rows) => rows.last().map(Entry::position),
            Self::Stream(rows) => rows.last().map(Entry::position),
            Self::Schemas(rows) => rows.last().map(Entry::position),
            Self::Files(rows) => rows.last().map(Entry::position),
        };
        position.map(|position| position.sequence)
    }
}

/* === Public Functions === */

#[tracing::instrument(name = "storage.query.reader.schema_at", level = "trace", skip_all)]
pub(crate) async fn schema_at(inner: &Arc<Inner>, snapshot: &Snapshot, key: &SchemaKey) -> Result<StorageSchema> {
    find_schema(inner, snapshot, key)
        .await?
        .ok_or(Error::InvalidInput { field: "storage schema", reason: "schema is unavailable at requested cursor" })
}

#[tracing::instrument(name = "storage.query.reader.find_schema", level = "trace", skip_all)]
pub(crate) async fn find_schema(inner: &Arc<Inner>, snapshot: &Snapshot, key: &SchemaKey) -> Result<Option<StorageSchema>> {
    let selection = super::SchemaSelection { schema: Some(key.clone()) };
    let page = ScanPage { after: 0, rows: NonZeroUsize::MIN, bytes: inner.settings.scan_bytes() };
    let (mut schemas, _) = snapshot.scan::<StorageSchema>(&selection, page).await?;
    Ok(schemas.pop())
}

#[tracing::instrument(name = "storage.query.reader.query_slot", level = "trace", skip_all)]
pub(crate) async fn query_slot(inner: &Arc<Inner>) -> Result<OwnedSemaphorePermit> {
    if let Ok(active) = Arc::clone(&inner.query_active).try_acquire_owned() {
        return Ok(active);
    }
    let _pending = Arc::clone(&inner.query_pending)
        .try_acquire_owned()
        .map_err(|_| Error::ResourceExhausted { resource: "pending storage queries" })?;
    Arc::clone(&inner.query_active).acquire_owned().await.map_err(|_| Error::Closing { domain: "storage queries" })
}

/* === Private Functions === */

#[tracing::instrument(name = "storage.query.reader.scan", level = "trace", skip_all)]
async fn scan<T: Entry>(snapshot: &Snapshot, selection: &T::Selection, page: ScanPage, wrap: fn(Vec<T>) -> Rows) -> Result<(Rows, usize)> {
    let (entries, bytes) = snapshot.scan::<T>(selection, page).await?;
    Ok((wrap(entries), bytes))
}

/* === Tests === */
