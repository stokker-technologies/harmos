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
use super::execute::{Pipeline, StreamSlice};
use super::{
    Query, QueryCompletion, QueryLimits, QueryOptions, QueryOutput, Source, StorageQuery, sealed,
};
use crate::storage::arrow::{ArrowBatch, ArrowSchema, RetainedMemory};
use crate::storage::{
    FileReference, ObjectEntry, ReadLimits, StorageCursor, StorageReader, StorageSchema,
    StorageView, StoredRecord, StoredRow,
};
use crate::{Error, MemorySize, Result};
use arrow_array::RecordBatch;
use std::time::Duration;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */
const OPTIONS: QueryOptions = QueryOptions {
    after: 0,
    through: None,
    order: crate::storage::ReadOrder::Ascending,
    page: None,
    execution: None,
    decoded_bytes: None,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct QueryReader {
    request: Query,
    view: StorageView,
    raw: RawReader,
    page: ReadLimits,
    schema: Option<crate::storage::QueryValue<StorageSchema>>,
    object_schemas: Option<super::object_schemas::NormalizedObjects>,
    pipeline: Option<Pipeline>,
    prefix: Option<Pipeline>,
    slicing: Option<StreamSlice>,
    reduction: Option<super::reduction::Reduction>,
    pending: Option<RecordBatch>,
    pending_offset: usize,
    global_done: bool,
    exhausted: bool,
    failed: bool,
    inflight: bool,
    rows: u64,
    schema_memory: Option<RetainedMemory>,
    staged_peak: usize,
}

/* === Enums === */
enum RawReader {
    Objects(StorageReader<ObjectEntry>),
    Records(StorageReader<StoredRecord>),
    Stream(StorageReader<StoredRow>),
    Schemas(StorageReader<StorageSchema>),
    Files(StorageReader<FileReference>),
}

/* === Traits === */

/* === Implementations === */
impl sealed::Supported for Query {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        None
    }
}

impl<C> StorageQuery<C> for Query {
    type Reader = QueryReader;
    fn options(&self) -> &QueryOptions {
        &OPTIONS
    }
    fn requested_cursor(&self) -> Option<&StorageCursor> {
        Some(&self.at)
    }
    fn request_memory_bytes(&self) -> usize {
        self.memory_size()
    }
    fn preflight(&self, _: &crate::storage::StorageManager<C>, _: QueryLimits) -> Result<()> {
        self.validate()
    }
    fn open(self, view: StorageView, page: ReadLimits, _: Duration) -> Result<QueryReader> {
        let mut selection = self.source.selection()?;
        selection.through = Some(self.at.sequence);
        let raw = match &self.source {
            Source::Objects(_) => RawReader::Objects(view.reader(selection)?),
            Source::Records(_) => RawReader::Records(view.reader(selection)?),
            Source::Stream(_) => RawReader::Stream(view.reader(selection)?),
            Source::Schemas(_) => RawReader::Schemas(view.reader(selection)?),
            Source::Files(_) => RawReader::Files(view.reader(selection)?),
        };
        Ok(QueryReader {
            request: self,
            view,
            raw,
            page,
            schema: None,
            object_schemas: None,
            pipeline: None,
            prefix: None,
            slicing: None,
            reduction: None,
            pending: None,
            pending_offset: 0,
            global_done: false,
            exhausted: false,
            failed: false,
            inflight: false,
            rows: 0,
            schema_memory: None,
            staged_peak: 0,
        })
    }
}

impl QueryReader {
    pub async fn next(&mut self) -> Result<Option<QueryOutput>> {
        if self.failed || self.inflight {
            self.fail();
            return Err(Error::NotReady {
                domain: "failed or interrupted storage query",
            });
        }
        // Dropping An In-Progress next() Future Poisons This Reader. A Later
        // Call Must Not Resume Beyond Already-Consumed Global Input Pages.
        self.inflight = true;
        let result = self.next_output().await;
        self.inflight = false;
        if result.is_err() {
            self.fail();
        }
        if self.exhausted || self.failed {
            self.view.release_query_permit();
        }
        result
    }

    fn fail(&mut self) {
        self.failed = true;
        self.pending = None;
        self.reduction = None;
        self.pipeline = None;
        self.prefix = None;
        self.schema = None;
        self.object_schemas = None;
        self.schema_memory = None;
        self.discard_source();
        self.view.release_query_permit();
    }

    pub fn completion(&self) -> Result<QueryCompletion> {
        if !self.exhausted || self.failed || self.inflight {
            return Err(Error::NotReady {
                domain: "storage query completion",
            });
        }
        Ok(QueryCompletion {
            at: self.request.at.clone(),
            rows: self.rows,
        })
    }

    async fn next_output(&mut self) -> Result<Option<QueryOutput>> {
        if self.exhausted {
            return Ok(None);
        }
        if self.pipeline.is_none() {
            self.prepare().await?;
            let pipeline = self.pipeline.as_ref().unwrap();
            let schema = ArrowSchema::try_new(pipeline.output_schema())?;
            let retained = pipeline
                .memory_size()
                .checked_add(self.prefix.as_ref().map_or(0, Pipeline::memory_size))
                .and_then(|bytes| {
                    bytes.checked_add(
                        self.reduction
                            .as_ref()
                            .map_or(0, |reduction| reduction.memory_size()),
                    )
                })
                .and_then(|bytes| bytes.checked_add(schema.memory_size()))
                .and_then(|bytes| bytes.checked_add(4096))
                .ok_or(Error::ResourceExhausted {
                    resource: "query retained plan",
                })?;
            let memory = self.schema_memory.as_ref().unwrap();
            memory.shrink_to(retained)?;
            return Ok(Some(QueryOutput::Schema(schema.retain(memory.clone()))));
        }
        loop {
            if let Some(output) = self.deliver_pending().await? {
                return Ok(Some(QueryOutput::Batch(output)));
            }
            let pipeline = self.pipeline.as_ref().unwrap();
            if pipeline.is_streaming() {
                if pipeline.is_finished(self.slicing.as_ref().unwrap()) {
                    self.exhausted = true;
                    return Ok(None);
                }
                let Some((input, bank, _credit, computation)) = self.next_working_source().await?
                else {
                    self.exhausted = true;
                    return Ok(None);
                };
                let mut reserve = |bytes| bank.reserve(bytes);
                let mut check = || computation.check();
                let output = self.pipeline.as_ref().unwrap().execute_batch(
                    input.as_record_batch(),
                    self.slicing.as_mut().unwrap(),
                    &mut check,
                    &mut reserve,
                )?;
                self.commit_source();
                self.pending = Some(output.as_record_batch().clone());
            } else if !self.global_done {
                let mut input = Vec::new();
                if self.reduction.is_some() {
                    while let Some((batch, bank, _credit, computation)) =
                        self.next_working_source().await?
                    {
                        let mut reserve = |bytes| bank.reserve(bytes);
                        self.reduction.as_mut().unwrap().push(
                            batch.as_record_batch(),
                            &mut || computation.check(),
                            &mut reserve,
                        )?;
                        self.commit_source();
                    }
                } else if self.prefix.is_some() {
                    while let Some((batch, bank, _credit, computation)) =
                        self.next_working_source().await?
                    {
                        let mut reserve = |bytes| bank.reserve(bytes);
                        let projected = self.prefix.as_ref().unwrap().execute_native_batch(
                            batch.as_record_batch(),
                            &mut || computation.check(),
                            &mut reserve,
                        )?;
                        input.push(projected);
                        self.commit_source();
                    }
                } else {
                    while let Some(batch) = self.next_source().await? {
                        input.push(batch.as_record_batch().clone());
                        self.commit_source();
                    }
                }
                let required = match &self.reduction {
                    Some(reduction) => reduction.finish_memory()?,
                    None => self.pipeline.as_ref().unwrap().complete_memory(&input)?,
                };
                let _credit = self
                    .view
                    .reserve_working_wait(
                        required
                            .checked_add(StorageView::computation_memory_bytes())
                            .and_then(|bytes| {
                                bytes.checked_add(crate::storage::manager::ENTRY_OVERHEAD)
                            })
                            .ok_or(Error::ResourceExhausted {
                                resource: "query working memory",
                            })?,
                    )
                    .await?;
                let computation = self.view.begin_computation()?;
                let reservation = self.view.reserve_read(required, 1)?;
                let bank = crate::storage::memory::MemoryBank::new(reservation, required);
                let mut reserve = |bytes| bank.reserve(bytes);
                let output = match self.reduction.take() {
                    Some(reduction) => {
                        reduction.finish(&mut || computation.check(), &mut reserve)?
                    }
                    None => self.pipeline.as_ref().unwrap().execute(
                        &input,
                        &mut || computation.check(),
                        &mut reserve,
                    )?,
                };
                self.pending = Some(output.as_record_batch().clone());
                self.global_done = true;
            } else {
                self.exhausted = true;
                return Ok(None);
            }
        }
    }

    async fn prepare(&mut self) -> Result<()> {
        let manager = self.view.manager();
        if let Source::Objects(selection) = &self.request.source {
            self.object_schemas = Some(
                super::object_schemas::NormalizedObjects::resolve(
                    &self.view,
                    selection,
                    &mut self.request.transforms,
                    self.page,
                )
                .await?,
            );
        }
        let key = match &self.request.source {
            Source::Records(selection) => Some(selection.schema.clone()),
            Source::Stream(selection) => Some(selection.schema.clone()),
            Source::Objects(_) | Source::Schemas(_) | Source::Files(_) => None,
        };
        self.schema = match key {
            Some(key) => Some(manager.lookup_schema(&self.view, &key).await?),
            None => None,
        };
        let schema_bytes = self
            .schema
            .as_ref()
            .map_or(0, |schema| (**schema).memory_size())
            .saturating_add(
                self.object_schemas
                    .as_ref()
                    .map_or(0, |schemas| schemas.memory_size()),
            );
        let allowance = schema_bytes
            .checked_mul(32)
            .and_then(|size| {
                self.request
                    .memory_size()
                    .checked_mul(8)
                    .and_then(|plan| size.checked_add(plan))
            })
            .and_then(|size| size.checked_add(64 * 1024))
            .ok_or(Error::ResourceExhausted {
                resource: "query schema memory",
            })?;
        let memory: RetainedMemory = crate::storage::manager::retain_memory(
            self.view.reserve_read_wait(allowance, 1).await?,
        );
        let empty = match &self.request.source {
            Source::Objects(_) => self.object_schemas.as_ref().unwrap().empty(),
            Source::Records(_) => {
                super::source::records(self.schema.as_ref().unwrap(), &[], allowance)?
            }
            Source::Stream(_) => {
                super::source::stream(self.schema.as_ref().unwrap(), &[], allowance)?
            }
            Source::Schemas(_) => super::source::schemas(&[], allowance)?,
            Source::Files(_) => super::source::files(&[], allowance)?,
        };
        let mut pipeline = Pipeline::new(
            empty.schema(),
            std::mem::take(&mut self.request.transforms),
            self.request.slice,
        )?;
        self.reduction = pipeline.incremental()?;
        if self.reduction.is_none()
            && let Some((prefix, suffix)) = pipeline.split_streaming_prefix()?
        {
            self.prefix = Some(prefix);
            pipeline = suffix;
        }
        self.slicing = Some(pipeline.stream_slice());
        self.pipeline = Some(pipeline);
        self.schema_memory = Some(memory);
        Ok(())
    }

    async fn next_source(&mut self) -> Result<Option<ArrowBatch>> {
        let page = self.page;
        let view = self.view.clone();
        let schema = self.schema.as_ref();
        let object_schemas = self.object_schemas.as_ref();
        let schema_memory = schema
            .map_or(0, |schema| (**schema).memory_size())
            .saturating_add(object_schemas.map_or(0, |schemas| schemas.memory_size()));
        macro_rules! fetch {
            ($reader:expr, $map:expr) => {{
                let mut credit = None;
                loop {
                    let Some(rows) = $reader.stage_page(page).await? else {
                        return Ok(None);
                    };
                    let bytes = rows
                        .iter()
                        .try_fold(0usize, |total, row| total.checked_add(row.memory_size()))
                        .ok_or(Error::ResourceExhausted {
                            resource: "query source conversion",
                        })?;
                    let allowance = bytes
                        .checked_mul(32)
                        .and_then(|bytes| bytes.checked_add(schema_memory.saturating_mul(16)))
                        .and_then(|bytes| bytes.checked_add(64 * 1024))
                        .ok_or(Error::ResourceExhausted {
                            resource: "query source conversion",
                        })?;
                    let peak = rows
                        .reserved_bytes()
                        .checked_add(allowance)
                        .and_then(|bytes| {
                            bytes.checked_add(StorageView::computation_memory_bytes())
                        })
                        .and_then(|bytes| {
                            bytes.checked_add(crate::storage::manager::ENTRY_OVERHEAD)
                        })
                        .ok_or(Error::ResourceExhausted {
                            resource: "query source conversion",
                        })?;
                    if credit.is_none() {
                        let working = allowance
                            .checked_add(StorageView::computation_memory_bytes())
                            .and_then(|bytes| {
                                bytes.checked_add(crate::storage::manager::ENTRY_OVERHEAD)
                            })
                            .ok_or(Error::ResourceExhausted {
                                resource: "query source conversion",
                            })?;
                        match view.try_reserve_working(working) {
                            Ok(admitted) => credit = Some(admitted),
                            Err(Error::ResourceExhausted { .. }) => {
                                drop(rows);
                                $reader.discard_staged_page();
                                credit = Some(view.reserve_working_wait(peak).await?);
                                continue;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    let _admitted_credit = credit.take().expect("source stage owns working credit");
                    let computation = view.begin_computation()?;
                    let reservation = view.reserve_read(allowance, 1)?;
                    let reservation = crate::storage::manager::retain_memory(reservation);
                    let batch: RecordBatch = $map(&rows, allowance)?;
                    let batch = ArrowBatch::try_new(batch)?;
                    if batch
                        .memory_size()
                        .checked_add(4096)
                        .is_none_or(|bytes| bytes > allowance)
                    {
                        return Err(Error::ResourceExhausted {
                            resource: "query source conversion",
                        });
                    }
                    reservation.shrink_to(batch.memory_size().saturating_add(4096))?;
                    self.staged_peak = peak;
                    let batch = batch.retain(reservation)?;
                    computation.check()?;
                    break Ok(Some(batch));
                }
            }};
        }
        match &mut self.raw {
            RawReader::Objects(reader) => {
                fetch!(reader, |rows: &[ObjectEntry], allowance| object_schemas
                    .unwrap()
                    .map(rows, allowance))
            }
            RawReader::Records(reader) => fetch!(reader, |rows: &[StoredRecord], allowance| {
                super::source::records(schema.unwrap(), rows, allowance)
            }),
            RawReader::Stream(reader) => fetch!(reader, |rows: &[StoredRow], allowance| {
                super::source::stream(schema.unwrap(), rows, allowance)
            }),
            RawReader::Schemas(reader) => fetch!(reader, |rows: &[StorageSchema], allowance| {
                super::source::schemas(rows, allowance)
            }),
            RawReader::Files(reader) => fetch!(reader, |rows: &[FileReference], allowance| {
                super::source::files(rows, allowance)
            }),
        }
    }

    fn commit_source(&mut self) {
        match &mut self.raw {
            RawReader::Objects(reader) => reader.commit_staged_page(),
            RawReader::Records(reader) => reader.commit_staged_page(),
            RawReader::Stream(reader) => reader.commit_staged_page(),
            RawReader::Schemas(reader) => reader.commit_staged_page(),
            RawReader::Files(reader) => reader.commit_staged_page(),
        }
    }

    fn discard_source(&mut self) {
        match &mut self.raw {
            RawReader::Objects(reader) => reader.discard_staged_page(),
            RawReader::Records(reader) => reader.discard_staged_page(),
            RawReader::Stream(reader) => reader.discard_staged_page(),
            RawReader::Schemas(reader) => reader.discard_staged_page(),
            RawReader::Files(reader) => reader.discard_staged_page(),
        }
    }

    async fn next_working_source(
        &mut self,
    ) -> Result<
        Option<(
            ArrowBatch,
            crate::storage::memory::MemoryBank,
            crate::storage::views::WorkingCredit,
            crate::storage::StorageViewComputation,
        )>,
    > {
        let Some(mut input) = self.next_source().await? else {
            return Ok(None);
        };
        let required = match (&self.reduction, &self.prefix) {
            (Some(reduction), _) => reduction.working_memory(input.as_record_batch())?,
            (_, Some(prefix)) => prefix.native_working_memory(input.as_record_batch())?,
            _ => self
                .pipeline
                .as_ref()
                .unwrap()
                .working_memory(input.as_record_batch())?,
        };
        let kernel_peak = required
            .checked_add(StorageView::computation_memory_bytes())
            .and_then(|bytes| bytes.checked_add(crate::storage::manager::ENTRY_OVERHEAD))
            .ok_or(Error::ResourceExhausted {
                resource: "query working memory",
            })?;
        let credit = match self.view.try_reserve_working(kernel_peak) {
            Ok(credit) => credit,
            Err(Error::ResourceExhausted { .. }) => {
                // Nothing Stateful Has Run And The Cursor Is Still Uncommitted.
                // Release This Page Before Waiting, Then Reload The Same Cut
                // Under One Reservation Covering Source And Kernel Coexistence.
                let peak =
                    self.staged_peak
                        .checked_add(kernel_peak)
                        .ok_or(Error::ResourceExhausted {
                            resource: "query working memory",
                        })?;
                drop(input);
                self.discard_source();
                let credit = self.view.reserve_working_wait(peak).await?;
                input = self.next_source().await?.ok_or(Error::InvalidInput {
                    field: "query source",
                    reason: "fixed cursor page disappeared during admission retry",
                })?;
                credit
            }
            Err(error) => return Err(error),
        };
        let computation = self.view.begin_computation()?;
        let reservation = self.view.reserve_read(required, 1)?;
        let bank = crate::storage::memory::MemoryBank::new(reservation, required);
        Ok(Some((input, bank, credit, computation)))
    }

    async fn deliver_pending(&mut self) -> Result<Option<ArrowBatch>> {
        let Some(batch) = &self.pending else {
            return Ok(None);
        };
        if self.pending_offset >= batch.num_rows() {
            self.pending = None;
            self.pending_offset = 0;
            return Ok(None);
        }
        let settings = self.view.manager().inner.settings.response;
        let mut rows = settings
            .batch_rows
            .get()
            .min(batch.num_rows() - self.pending_offset);
        loop {
            let slice = batch.slice(self.pending_offset, rows);
            let allowance = crate::storage::arrow::resident_bytes(&slice)?
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(settings.batch_bytes.get()))
                .ok_or(Error::ResourceExhausted {
                    resource: "query response encoding",
                })?;
            let reservation: RetainedMemory = crate::storage::manager::retain_memory(
                self.view.reserve_read_wait(allowance, 1).await?,
            );
            let output = ArrowBatch::try_new(slice)?;
            // QueryOutput adds its envelope around the Arrow bytes field.
            let envelope = QueryOutput::Batch(output.clone());
            let encoded_bytes = crate::Message::encoded_len(&envelope);
            #[cfg(feature = "grpc")]
            let encoded_bytes = encoded_bytes.max(crate::Protobuf::encoded_len(&envelope));
            if encoded_bytes <= settings.batch_bytes.get() {
                self.pending_offset += rows;
                self.rows = self
                    .rows
                    .checked_add(rows as u64)
                    .ok_or(Error::ResourceExhausted {
                        resource: "query row count",
                    })?;
                reservation.shrink_to(output.encoding_memory_size()?)?;
                return output.retain(reservation).map(Some);
            }
            if rows == 1 {
                return Err(Error::ResourceExhausted {
                    resource: "oversized query row",
                });
            }
            rows /= 2;
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
