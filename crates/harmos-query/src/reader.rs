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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Third Party Imports
use arrow_array::RecordBatch;

// Local Imports
use crate::arrow::{ArrowBatch, ArrowSchema};
use crate::batches::{BatchReader, BatchScan, BatchSource};
use crate::execute::Plan;
use crate::merge::TimeMergeReader;
use crate::transform::Transform;
use crate::{Error, Result};
use harmos_domain::MemorySize;
use harmos_domain::model::storage::ReadLimits;
use harmos_domain::model::storage::query::{Query, QueryCompletion};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct QueryResponse {
    pub batch_rows: NonZeroUsize,
    pub batch_bytes: NonZeroUsize,
}

pub struct QueryReader {
    query: Query,
    source: Option<Box<dyn BatchReader>>,
    plan: Option<Plan>,
    output: QueryResponse,
    maximum: QueryResponse,
    pending: Option<RecordBatch>,
    pending_offset: usize,
    delivered: u64,
    source_done: bool,
    schema_sent: bool,
    exhausted: bool,
    failed: bool,
    inflight: bool,
}

struct CancelWork(Arc<AtomicBool>);

/* === Enums === */

#[derive(Clone, Debug)]
pub enum QueryArrow {
    Schema(ArrowSchema),
    Batch(ArrowBatch),
}

pub enum QueryStep {
    Output(QueryArrow),
    Progress,
    Complete,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryOutput {
    Schema(Vec<u8>),
    Batch(Vec<u8>),
}

/* === Traits === */

/* === Implementations === */

impl Drop for CancelWork {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl QueryArrow {
    // Bounds The Transport Value Before IPC Conversion.
    // One Maximum Protobuf Field Key and Length Prefix Cover the IPC Bytes.
    pub fn output_memory_size(&self) -> usize {
        let capacity = match self {
            Self::Schema(schema) => schema.ipc_capacity(),
            Self::Batch(batch) => batch.ipc_capacity(),
        };
        std::mem::size_of::<Vec<u8>>().saturating_add(capacity).saturating_add(15)
    }

    #[tracing::instrument(name = "storage.query.reader.output", level = "trace", skip_all)]
    pub fn output(self) -> QueryOutput {
        match self {
            Self::Schema(schema) => QueryOutput::Schema(schema.ipc_bytes().to_vec()),
            Self::Batch(batch) => QueryOutput::Batch(batch.ipc_bytes().to_vec()),
        }
    }
}

impl MemorySize for QueryOutput {
    fn memory_size(&self) -> usize {
        let bytes = match self {
            Self::Schema(bytes) | Self::Batch(bytes) => bytes,
        };
        std::mem::size_of::<Self>().saturating_add(bytes.capacity())
    }
}

impl TryFrom<QueryOutput> for QueryArrow {
    type Error = Error;
    fn try_from(output: QueryOutput) -> Result<Self> {
        match output {
            QueryOutput::Schema(ipc) => Ok(Self::Schema(ArrowSchema::from_ipc(ipc)?)),
            QueryOutput::Batch(ipc) => Ok(Self::Batch(ArrowBatch::from_ipc(ipc)?)),
        }
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

impl QueryReader {
    pub async fn open(source: &dyn BatchSource, query: Query, scan: BatchScan, output: QueryResponse) -> Result<Self> {
        query.validate()?;
        let reader = source.open(&query.source, &query.at, &scan).await?;
        let secondary = match &query.merge {
            Some(merge) => Some(source.open(&merge.source, &query.at, &scan).await?),
            None => None,
        };
        Self::from_sources(reader, secondary, query, scan.projection.is_some(), output, scan.limits)
    }

    pub fn from_sources(
        primary: Box<dyn BatchReader>, secondary: Option<Box<dyn BatchReader>>, query: Query, projected: bool, output: QueryResponse,
        limits: ReadLimits,
    ) -> Result<Self> {
        let source: Box<dyn BatchReader> = match (&query.merge, secondary) {
            (Some(merge), Some(secondary)) => {
                if projected {
                    return Err(Error::InvalidInput {
                        field: "open time merge",
                        reason: "project after merging both sources",
                    });
                }
                Box::new(TimeMergeReader::new(primary, secondary, merge.clone(), limits.rows)?)
            }
            (None, None) => primary,
            _ => {
                return Err(Error::InvalidInput {
                    field: "open time merge",
                    reason: "request and second source must agree",
                });
            }
        };
        Self::from_reader(source, query, projected, output)
    }

    pub fn from_reader(source: Box<dyn BatchReader>, mut query: Query, projected: bool, output: QueryResponse) -> Result<Self> {
        query.validate()?;
        if source.at() != &query.at {
            return Err(Error::InvalidInput {
                field: "open query",
                reason: "source returned a different cut",
            });
        }
        let mut transforms = std::mem::take(&mut query.transforms);
        if projected {
            if !matches!(transforms.first(), Some(Transform::Project(_))) {
                return Err(Error::InvalidInput {
                    field: "open query",
                    reason: "source projection requires the leading transform",
                });
            }
            transforms.remove(0);
        }
        let plan = Plan::new(source.schema(), transforms, query.slice)?;
        Ok(Self {
            query,
            source: Some(source),
            plan: Some(plan),
            output,
            maximum: output,
            pending: None,
            pending_offset: 0,
            delivered: 0,
            source_done: false,
            schema_sent: false,
            exhausted: false,
            failed: false,
            inflight: false,
        })
    }

    pub fn configure_page(&mut self, rows: NonZeroUsize, bytes: NonZeroUsize) {
        self.output.batch_rows = rows.min(self.maximum.batch_rows);
        self.output.batch_bytes = bytes.min(self.maximum.batch_bytes);
    }

    pub async fn next(&mut self) -> Result<Option<QueryArrow>> {
        loop {
            match self.step().await? {
                QueryStep::Output(value) => return Ok(Some(value)),
                QueryStep::Progress => {}
                QueryStep::Complete => return Ok(None),
            }
        }
    }

    pub async fn step(&mut self) -> Result<QueryStep> {
        if self.failed || self.inflight {
            self.fail();
            return Err(Error::NotReady {
                domain: "advance failed or interrupted query",
            });
        }
        self.inflight = true;
        let result = self.advance_step().await;
        self.inflight = false;
        if result.is_err() {
            self.fail();
        }
        result
    }

    fn fail(&mut self) {
        self.failed = true;
        self.pending = None;
        self.plan = None;
        self.source = None;
    }

    async fn advance_step(&mut self) -> Result<QueryStep> {
        if self.exhausted {
            return Ok(QueryStep::Complete);
        }
        if !self.schema_sent {
            let schema = self.plan.as_ref().expect("open plan").output_schema();
            self.schema_sent = true;
            return Ok(QueryStep::Output(QueryArrow::Schema(ArrowSchema::try_new(schema)?)));
        }
        if let Some(batch) = self.deliver().await? {
            return Ok(QueryStep::Output(QueryArrow::Batch(batch)));
        }
        if self.plan.as_ref().expect("open plan").is_satisfied() {
            self.source_done = true;
        }
        if !self.source_done {
            let page = self.source.as_mut().expect("running source").next().await?;
            self.source_done = page.complete;
            self.pending = self.work(Some(page.batch)).await?.filter(|batch| batch.num_rows() > 0);
            self.pending_offset = 0;
            return Ok(QueryStep::Progress);
        }
        self.pending = self.work(None).await?.filter(|batch| batch.num_rows() > 0);
        self.pending_offset = 0;
        if self.pending.is_some() {
            return Ok(QueryStep::Progress);
        }
        self.plan = None;
        self.source = None;
        self.exhausted = true;
        Ok(QueryStep::Complete)
    }

    pub fn completion(&self) -> Result<QueryCompletion> {
        if !self.exhausted || self.failed || self.inflight {
            return Err(Error::NotReady { domain: "storage query completion" });
        }
        Ok(QueryCompletion {
            at: self.query.at.clone(),
            rows: self.delivered,
        })
    }
    async fn work(&mut self, batch: Option<RecordBatch>) -> Result<Option<RecordBatch>> {
        let plan = self.plan.as_mut().expect("prepared query plan");
        if plan.is_inline() {
            return match batch {
                Some(batch) => plan.page(&batch, &mut || Ok(())),
                None => plan.finish(&mut || Ok(())),
            };
        }
        let mut plan = self.plan.take().expect("prepared query plan");
        let cancelled = Arc::new(AtomicBool::new(false));
        let guard = CancelWork(Arc::clone(&cancelled));
        let dispatcher = tracing::dispatcher::get_default(Clone::clone);
        let result = tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatcher, || {
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
    async fn deliver(&mut self) -> Result<Option<ArrowBatch>> {
        let Some(batch) = &self.pending else {
            return Ok(None);
        };
        if self.pending_offset >= batch.num_rows() {
            self.pending = None;
            self.pending_offset = 0;
            return Ok(None);
        }
        let settings = self.output;
        let mut rows = settings.batch_rows.get().min(batch.num_rows() - self.pending_offset);
        loop {
            let slice = batch.slice(self.pending_offset, rows);
            let output = ArrowBatch::try_new(slice)?;

            let encoded = output.ipc_bytes().len().saturating_add(15);
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
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
