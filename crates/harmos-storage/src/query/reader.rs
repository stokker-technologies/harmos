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
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use tokio::sync::OwnedSemaphorePermit;

// Local Imports
use super::execute::Plan;
use super::object_schemas::NormalizedObjects;
use super::{QueryCompletion, QueryOutput, Source};
use crate::storage::arrow::{ArrowBatch, ArrowSchema, RetainedMemory, resident_bytes, retain_native};
use crate::storage::backend::ScanPage;
use crate::storage::capacity::{Held, Pool, Refusal, Reservation};
use crate::storage::config::SCAN_ROWS;
use crate::storage::manager::{ENTRY_OVERHEAD, Inner};
use crate::storage::memory::{MemoryBank, Shortfall};
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
/// Delivers one schema, then bounded batches, of one admitted query.
///
/// The reader owns an active query slot and a pinned snapshot until the source
/// is exhausted. Dropping it cancels remaining work; batches already returned
/// keep their memory charged until the consumer drops them. Dropping an
/// in-progress `next` future poisons the reader.
pub struct QueryReader {
    inner: Arc<Inner>,
    held: Held,
    query: Query,
    snapshot: Option<Snapshot>,
    active: Option<OwnedSemaphorePermit>,
    _description: Reservation,
    schema: Option<(StorageSchema, RetainedMemory)>,
    objects: Option<NormalizedObjects>,
    plan: Option<(Plan, RetainedMemory)>,
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
/// One native query item: the result schema first, then retained batches.
#[derive(Clone, Debug)]
pub enum QueryArrow {
    Schema(ArrowSchema),
    Batch(ArrowBatch),
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
    /// The wire form of this item: its Arrow IPC message.
    pub fn output(&self) -> QueryOutput {
        match self {
            Self::Schema(schema) => QueryOutput::Schema(ArrowIpc::new(schema.ipc_bytes().to_vec())),
            Self::Batch(batch) => QueryOutput::Batch(ArrowIpc::new(batch.ipc_bytes().to_vec())),
        }
    }
}

impl TryFrom<&QueryOutput> for QueryArrow {
    type Error = Error;

    /// Decode a wire item, preflighting the Arrow allocation it describes.
    fn try_from(output: &QueryOutput) -> Result<Self> {
        Ok(match output {
            QueryOutput::Schema(ipc) => Self::Schema(ArrowSchema::from_ipc(ipc.bytes())?),
            QueryOutput::Batch(ipc) => Self::Batch(ArrowBatch::from_ipc(ipc.bytes())?),
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
    /// Validate and admit a query at its fixed cursor. Results are produced by
    /// successive `QueryReader::next` calls.
    pub async fn query(&self, query: Query) -> Result<QueryReader> {
        query.validate()?;
        self.inner.primary.history().check(&query.at.history)?;

        // Admit A Query Slot Before Charging Its Description. Queued Callers
        // Hold No Read Bytes That An Active Reader Might Need To Finish.
        let active = query_slot(&self.inner).await?;
        let held = Held::default();
        let description_bytes = query.memory_size().saturating_add(ENTRY_OVERHEAD);
        let description = Reservation::wait(&self.inner, Pool::Read, &held, description_bytes, None).await.map_err(Refusal::into_error)?;

        // Pin The Requested Image For The Whole Invocation.
        let snapshot = self.snapshot(&query.at).await?;
        let after = match &query.source {
            Source::Stream(selection) => selection.after,
            _ => 0,
        };
        Ok(QueryReader {
            inner: Arc::clone(&self.inner),
            held,
            query,
            snapshot: Some(snapshot),
            active: Some(active),
            _description: description,
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
}

impl QueryReader {
    pub async fn next(&mut self) -> Result<Option<QueryArrow>> {
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
    }

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

    // Free The Active Slot And Snapshot Once No More Source Input Is Needed.
    fn release(&mut self) {
        self.snapshot = None;
        self.active = None;
    }

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
            let (plan, _) = self.plan.as_ref().expect("prepared plan");
            if plan.is_satisfied() {
                self.source_done = true;
            }
            if !self.source_done {
                self.next_page().await?;
                continue;
            }
            self.release();
            if !self.finish().await? {
                self.exhausted = true;
                return Ok(None);
            }
        }
    }

    // Resolve Source Schemas, Validate The Plan And Fix The Output Schema.
    async fn prepare(&mut self) -> Result<ArrowSchema> {
        let snapshot = self.snapshot.as_ref().expect("prepared query owns its snapshot");
        let mut transforms = std::mem::take(&mut self.query.transforms);
        match &self.query.source {
            Source::Objects(selection) => {
                let objects = NormalizedObjects::resolve(&self.inner, &self.held, snapshot, selection, &mut transforms).await?;
                self.objects = Some(objects);
            }
            Source::Records(selection) => {
                let schema = schema_at(&self.inner, &self.held, snapshot, &selection.schema).await?;
                self.schema = Some(schema);
            }
            Source::Stream(selection) => {
                let schema = schema_at(&self.inner, &self.held, snapshot, &selection.schema).await?;
                self.schema = Some(schema);
            }
            Source::Schemas(_) | Source::Files(_) => {}
        }

        // Derive The Source Schema From An Empty Conversion.
        let mut credit = 0usize;
        loop {
            let bank = admit(&self.inner, &self.held, credit).await?;
            let attempt = (|| {
                let empty = self.convert(None, &bank)?;
                let plan = Plan::new(empty.schema(), transforms.clone(), self.query.slice)?;
                let schema = ArrowSchema::try_new(plan.output_schema())?;
                let retained =
                    bank.reserve(plan.memory_size().saturating_add(schema.memory_size()).saturating_add(transforms.memory_size()))?;
                Ok::<_, Error>((plan, schema, retained))
            })();
            match attempt {
                Ok((plan, schema, retained)) => {
                    let output = schema.retain(retained.clone());
                    self.plan = Some((plan, retained));
                    return Ok(output);
                }
                Err(error) => credit = retry(&bank, error, None)?,
            }
        }
    }

    // Load, Convert And Process One Source Page As One Reloadable Attempt.
    async fn next_page(&mut self) -> Result<()> {
        let mut credit = 0usize;
        loop {
            let bank = match MemoryBank::admit(&self.inner, &self.held, credit, None).await {
                Ok(bank) => bank,
                Err(Refusal::Exceeded(_)) if self.rows.get() > 1 => {
                    self.rows = NonZeroUsize::new(self.rows.get() / 2).unwrap();
                    credit = 0;
                    continue;
                }
                Err(refusal) => return Err(refusal.into_error()),
            };
            match self.page_attempt(&bank).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let mut rows = self.rows;
                    credit = retry(&bank, error, Some(&mut rows))?;
                    self.rows = rows;
                }
            }
        }
    }

    async fn page_attempt(&mut self, bank: &MemoryBank) -> Result<()> {
        // Charge The Raw Page Before Backend I/O.
        let scan_bytes = self.inner.settings.scan_bytes();
        let page = ScanPage { after: self.after, rows: self.rows, bytes: scan_bytes };
        let mut raw = bank.scope();
        raw.claim(self.rows.get().saturating_mul(ENTRY_OVERHEAD).saturating_add(scan_bytes.get()))?;
        let snapshot = self.snapshot.as_ref().expect("running query owns its snapshot");
        let (rows, bytes) = match &self.query.source {
            Source::Objects(selection) => scan(snapshot, selection, page, Rows::Objects).await?,
            Source::Records(selection) => scan(snapshot, selection, page, Rows::Records).await?,
            Source::Stream(selection) => scan(snapshot, selection, page, Rows::Stream).await?,
            Source::Schemas(selection) => scan(snapshot, selection, page, Rows::Schemas).await?,
            Source::Files(selection) => scan(snapshot, selection, page, Rows::Files).await?,
        };
        // A Single Oversized Entry Is Charged At Its Actual Size.
        raw.claim(bytes.saturating_sub(scan_bytes.get()))?;
        let Some(last) = rows.last() else {
            self.source_done = true;
            return Ok(());
        };

        // Convert And Run The Page; State Changes Only After Every Claim Succeeded.
        let batch = self.convert(Some(&rows), bank)?;
        let retained = bank.reserve(resident_bytes(&batch)?)?;
        let batch = retain_native(batch, retained)?;
        drop(rows);
        let (plan, _) = self.plan.as_mut().expect("prepared plan");
        let output = plan.page(&batch, bank, &mut || Ok(()))?;
        self.pending = output.filter(|output| output.num_rows() != 0);
        self.pending_offset = 0;
        self.after = last;
        Ok(())
    }

    // Produce The Global Output Once; `false` When Nothing Remains.
    async fn finish(&mut self) -> Result<bool> {
        let Some((plan, _)) = self.plan.as_mut() else {
            return Ok(false);
        };
        let mut credit = plan.finish_bytes()?;
        loop {
            let bank = admit(&self.inner, &self.held, credit).await?;
            let (plan, _) = self.plan.as_mut().expect("prepared plan");
            match plan.finish(&bank, &mut || Ok(())) {
                Ok(Some(output)) if output.num_rows() != 0 => {
                    self.pending = Some(output);
                    self.pending_offset = 0;
                    return Ok(true);
                }
                Ok(_) => return Ok(false),
                Err(error) => credit = retry(&bank, error, None)?,
            }
        }
    }

    // Split Pending Output Into Response Batches Within Both Ceilings.
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
            let allowance = resident_bytes(&slice)?.saturating_mul(4).saturating_add(settings.batch_bytes.get());
            let mut reservation =
                Reservation::wait(&self.inner, Pool::Read, &self.held, allowance, None).await.map_err(Refusal::into_error)?;
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
                reservation.shrink_to(output.encoding_memory_size()?);
                return output.retain(reservation.retain()).map(Some);
            }
            if rows == 1 {
                return Err(Error::ResourceExhausted { resource: "oversized query row" });
            }
            rows /= 2;
        }
    }

    fn convert(&self, rows: Option<&Rows>, bank: &MemoryBank) -> Result<RecordBatch> {
        let schema = self.schema.as_ref().map(|(schema, _)| schema);
        match (&self.query.source, rows) {
            (Source::Objects(_), rows) => {
                let objects = self.objects.as_ref().expect("resolved object schemas");
                match rows {
                    Some(Rows::Objects(rows)) => objects.map(rows, bank),
                    _ => Ok(objects.empty()),
                }
            }
            (Source::Records(_), Some(Rows::Records(rows))) => super::source::records(schema.expect("record schema"), rows, bank),
            (Source::Records(_), _) => super::source::records(schema.expect("record schema"), &[], bank),
            (Source::Stream(_), Some(Rows::Stream(rows))) => super::source::stream(schema.expect("stream schema"), rows, bank),
            (Source::Stream(_), _) => super::source::stream(schema.expect("stream schema"), &[], bank),
            (Source::Schemas(_), Some(Rows::Schemas(rows))) => super::source::schemas(rows, bank),
            (Source::Schemas(_), _) => super::source::schemas(&[], bank),
            (Source::Files(_), Some(Rows::Files(rows))) => super::source::files(rows, bank),
            (Source::Files(_), _) => super::source::files(&[], bank),
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
/// Resolve the schema registered under `key` at the snapshot, charged to `held`.
pub(crate) async fn schema_at(
    inner: &Arc<Inner>, held: &Held, snapshot: &Snapshot, key: &SchemaKey,
) -> Result<(StorageSchema, RetainedMemory)> {
    find_schema(inner, held, snapshot, key)
        .await?
        .ok_or(Error::InvalidInput { field: "storage schema", reason: "schema is unavailable at requested cursor" })
}

/// The schema registered under `key` at the snapshot, or `None` when it was never written.
pub(crate) async fn find_schema(
    inner: &Arc<Inner>, held: &Held, snapshot: &Snapshot, key: &SchemaKey,
) -> Result<Option<(StorageSchema, RetainedMemory)>> {
    let selection = super::SchemaSelection { schema: Some(key.clone()) };
    let scan_bytes = inner.settings.scan_bytes();
    let mut credit = 0usize;
    loop {
        let bank = admit(inner, held, credit).await?;
        let attempt = async {
            let mut raw = bank.scope();
            raw.claim(scan_bytes.get().saturating_add(ENTRY_OVERHEAD))?;
            let page = ScanPage { after: 0, rows: NonZeroUsize::MIN, bytes: scan_bytes };
            let (mut schemas, _) = snapshot.scan::<StorageSchema>(&selection, page).await?;
            let Some(schema) = schemas.pop() else {
                return Ok::<_, Error>(None);
            };
            let retained = bank.reserve(schema.memory_size().saturating_add(ENTRY_OVERHEAD))?;
            Ok(Some((schema, retained)))
        };
        match attempt.await {
            Ok(resolved) => return Ok(resolved),
            Err(error) => credit = retry(&bank, error, None)?,
        }
    }
}

/// An active scan slot, queueing within the pending bound when all are busy.
pub(crate) async fn query_slot(inner: &Arc<Inner>) -> Result<OwnedSemaphorePermit> {
    if let Ok(active) = Arc::clone(&inner.query_active).try_acquire_owned() {
        return Ok(active);
    }
    let _pending = Arc::clone(&inner.query_pending)
        .try_acquire_owned()
        .map_err(|_| Error::ResourceExhausted { resource: "pending storage queries" })?;
    Arc::clone(&inner.query_active).acquire_owned().await.map_err(|_| Error::Closing { domain: "storage queries" })
}

/// Admit an attempt bank; an impossible credit is a precise capacity error.
pub(crate) async fn admit(inner: &Arc<Inner>, held: &Held, credit: usize) -> Result<MemoryBank> {
    MemoryBank::admit(inner, held, credit, None).await.map_err(Refusal::into_error)
}

/// Decide whether a failed attempt can retry: busy capacity retries with the
/// observed demand as credit; impossible demand halves the page when possible.
pub(crate) fn retry(bank: &MemoryBank, error: Error, rows: Option<&mut NonZeroUsize>) -> Result<usize> {
    match bank.shortfall() {
        Some(Shortfall::Busy) => Ok(bank.requested()),
        Some(Shortfall::Exceeded) => match rows {
            Some(rows) if rows.get() > 1 => {
                *rows = NonZeroUsize::new(rows.get() / 2).unwrap();
                Ok(0)
            }
            _ => Err(Error::ResourceExhausted { resource: "storage read memory" }),
        },
        None => Err(error),
    }
}

/* === Private Functions === */
async fn scan<T: Entry>(snapshot: &Snapshot, selection: &T::Selection, page: ScanPage, wrap: fn(Vec<T>) -> Rows) -> Result<(Rows, usize)> {
    let (entries, bytes) = snapshot.scan::<T>(selection, page).await?;
    Ok((wrap(entries), bytes))
}

/* === Tests === */
