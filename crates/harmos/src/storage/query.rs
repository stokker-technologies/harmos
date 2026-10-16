/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod contract;
mod objects;
mod reader;
pub use reader::QueryReader;
pub mod execute;
mod object_schemas;
pub mod reduction;
mod source;
pub mod transform;
pub use contract::{
    FileSelection, ObjectSelection, ObjectVersions, Query, QueryCompletion, QueryOutput,
    RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
};
pub(super) mod sealed {
    pub trait Supported {
        fn options_mut(&mut self) -> Option<&mut super::QueryOptions>;
    }
}

/* === Re-Exports === */
pub use objects::EncodedObjectQuery;

/* === Imports === */
// Standard Library Imports
use std::fmt;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::manager::{ENTRY_OVERHEAD, MemoryReservation};
use super::*;
use crate::catalog::Contains;
use crate::storage::catalog::{decode_admitted, object_key, object_schema};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type RecordQueryResult<T> = QueryResult<StoredRecord, StoredRecord<T>>;
pub type StreamQueryResult<T> = QueryResult<StoredRow, StoredRow<T>>;
pub type FileQueryResult = QueryResult<FileReference, FileReference>;
pub type SchemaQueryResult = QueryResult<StorageSchema, StorageSchema>;
pub type ObjectQueryResult<T> = QueryResult<ObjectEntry, ObjectRevision<T>>;
type PayloadDecode<T> = dyn Fn(&[u8], NonZeroUsize) -> Result<T> + Send + Sync;
type DecodingMemory = dyn Fn(&[u8]) -> Result<usize> + Send + Sync;
type Decode<R, T> = dyn Fn(R, NonZeroUsize) -> Result<T> + Send + Sync;

/* === Structs === */
/// A view's read budget. `memory_bytes` is the per-view ceiling; `admission`
/// decides how much of it the shared read pool reserves when the view opens.
///
/// View acquisition, view-less reads, productions and projections all obey
/// the same shared ceiling. A covered view charge draws from its reservation;
/// other reads cannot borrow capacity reserved for an admitted working stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryLimits {
    pub memory_bytes: NonZeroUsize,
    pub admission: Admission,
    pub spill_bytes: usize,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct QueryOptions {
    pub after: u64,
    pub through: Option<u64>,
    pub order: ReadOrder,
    pub page: Option<ReadLimits>,
    pub execution: Option<QueryLimits>,
    pub decoded_bytes: Option<NonZeroUsize>,
}

pub struct RecordQuery<T> {
    pub schema: SchemaKey,
    pub filter: Option<RecordFilter>,
    pub source_time: Option<SourceTimeRange>,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub options: QueryOptions,
    decode: Box<PayloadDecode<T>>,
}

pub struct StreamQuery<T> {
    pub schema: SchemaKey,
    pub source_time: Option<SourceTimeRange>,
    pub scope: Option<Identifier>,
    pub stream: Option<Identifier>,
    pub item: Option<Identifier>,
    pub options: QueryOptions,
    decode: Box<PayloadDecode<T>>,
}

#[derive(Clone, Debug, Default)]
pub struct SchemaQuery {
    pub schema: Option<SchemaKey>,
    pub options: QueryOptions,
}

#[derive(Clone, Debug, Default)]
pub struct FileQuery {
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub options: QueryOptions,
}

pub struct ObjectQuery<'a, T: StoredObject> {
    pub item: Option<&'a T::Key>,
    pub versions: super::ObjectVersions,
    pub history: ReadHistory,
    pub options: QueryOptions,
    decoding_memory: Option<Box<DecodingMemory>>,
}

pub struct QueryResult<R, T> {
    reader: StorageReader<R>,
    retained_view: Option<StorageView>,
    pending: Option<QueryPage<T>>,
    page: ReadLimits,
    decode: Box<Decode<R, T>>,
    decoded_bytes: NonZeroUsize,
}

pub struct QueryPage<T> {
    entries: Vec<T>,
    _reservation: MemoryReservation,
}

pub struct QueryValue<T> {
    value: T,
    _reservation: MemoryReservation,
    execution: Option<Arc<super::production::Execution>>,
}

pub struct SharedQueryValue<T> {
    owner: Arc<QueryValue<T>>,
}

pub(super) struct ActiveBudget<'a> {
    pub(super) remaining: &'a mut Duration,
    pub(super) started: tokio::time::Instant,
}

/* === Enums === */
// How A View Holds The Shared Read Pool (`StorageSettings::budget`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    /// Reserve the caller-supplied ceiling at open. Reads under the view draw
    /// from that ceiling; only bytes beyond it charge the pool. Canonical Query
    /// uses elastic admission and storage-owned temporary working reservations.
    #[default]
    Guaranteed,
    /// Reserve a floor at open (one query page plus view metadata); each
    /// production charges its real bytes against the ceiling and the pool and
    /// releases them on drop. A charge that would overfill the pool fails with
    /// `ResourceExhausted { resource: "storage read memory" }` for a retry.
    Elastic,
}

/* === Traits === */
pub trait StorageQuery<C>: sealed::Supported {
    type Reader;

    fn options(&self) -> &QueryOptions;
    fn requested_cursor(&self) -> Option<&StorageCursor> {
        None
    }
    fn request_memory_bytes(&self) -> usize {
        std::mem::size_of_val(self)
    }

    fn preflight(&self, _manager: &StorageManager<C>, _limits: QueryLimits) -> Result<()> {
        Ok(())
    }

    fn open(self, view: StorageView, page: ReadLimits, remaining: Duration)
    -> Result<Self::Reader>;
}

pub trait FollowQuery<C>: sealed::Supported {
    type Raw;
    type Value: MemorySize;

    fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> impl std::future::Future<Output = Result<super::follow::QueryFollower<Self::Raw, Self::Value>>>;
}

/* === Implementations === */
impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            after: 0,
            through: None,
            order: ReadOrder::Ascending,
            page: None,
            execution: None,
            decoded_bytes: None,
        }
    }
}

impl QueryLimits {
    // Reserve Only A Floor At Open And Charge Productions As They Happen.
    pub fn elastic(self) -> Self {
        self.with_admission(Admission::Elastic)
    }

    pub fn with_admission(mut self, admission: Admission) -> Self {
        self.admission = admission;
        self
    }

    pub fn check(self) -> Result<()> {
        if self.timeout.is_zero() {
            return Err(Error::InvalidInput {
                field: "query execution",
                reason: "timeout must be nonzero",
            });
        }
        Ok(())
    }
}

impl Drop for ActiveBudget<'_> {
    fn drop(&mut self) {
        *self.remaining = self.remaining.saturating_sub(self.started.elapsed());
    }
}

impl QueryOptions {
    pub(super) fn selection(&self, kind: ReadKind) -> ReadSelection {
        let mut selection = ReadSelection::new(kind);
        selection.after = self.after;
        selection.through = self.through;
        selection.order = self.order;
        selection
    }
}

impl<T> RecordQuery<T> {
    pub fn new(
        schema: SchemaKey,
        decode: impl Fn(&[u8], NonZeroUsize) -> Result<T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            schema,
            filter: None,
            source_time: None,
            decode: Box::new(decode),
            scope: None,
            item: None,
            options: QueryOptions::default(),
        }
    }
}

impl<T> StreamQuery<T> {
    pub fn new(
        schema: SchemaKey,
        decode: impl Fn(&[u8], NonZeroUsize) -> Result<T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            schema,
            source_time: None,
            decode: Box::new(decode),
            scope: None,
            stream: None,
            item: None,
            options: QueryOptions::default(),
        }
    }
}

impl SchemaQuery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FileQuery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T: StoredObject> Default for ObjectQuery<'_, T> {
    fn default() -> Self {
        Self {
            item: None,
            versions: super::ObjectVersions::Latest,
            history: ReadHistory::Retained,
            options: QueryOptions::default(),
            decoding_memory: None,
        }
    }
}

impl<T: StoredObject> ObjectQuery<'_, T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decoding_memory(
        mut self,
        estimate: impl Fn(&[u8]) -> Result<usize> + Send + Sync + 'static,
    ) -> Self {
        self.decoding_memory = Some(Box::new(estimate));
        self
    }
}

impl<T> sealed::Supported for RecordQuery<T> {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}

impl<T> sealed::Supported for StreamQuery<T> {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}

impl sealed::Supported for FileQuery {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}

impl sealed::Supported for SchemaQuery {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}

impl<T: StoredObject> sealed::Supported for ObjectQuery<'_, T> {
    fn options_mut(&mut self) -> Option<&mut QueryOptions> {
        Some(&mut self.options)
    }
}

impl<C, T: MemorySize + 'static> FollowQuery<C> for RecordQuery<T> {
    type Raw = StoredRecord;
    type Value = StoredRecord<T>;
    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<super::follow::QueryFollower<Self::Raw, Self::Value>> {
        if self.filter.is_some() {
            return Err(Error::InvalidInput {
                field: "record filter",
                reason: "payload filters require saved-query paging",
            });
        }
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        let mut selection = self.options.selection(ReadKind::Records);
        selection.schema = Some(self.schema);
        selection.source_time = self.source_time;
        selection.scope = self.scope;
        selection.item = self.item;
        let mode = if replay {
            super::follow::Mode::Replay
        } else {
            super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<StoredRecord>(selection, mode, execution)
            .await?;
        let decode = self.decode;
        let decode = Box::new(move |record: &StoredRecord, bytes| {
            decode_record_borrowed(record, decode.as_ref(), bytes)
        });
        Ok(super::follow::QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

impl<C, T: MemorySize + 'static> FollowQuery<C> for StreamQuery<T> {
    type Raw = StoredRow;
    type Value = StoredRow<T>;
    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<super::follow::QueryFollower<Self::Raw, Self::Value>> {
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        let mut selection = self.options.selection(ReadKind::Stream);
        selection.schema = Some(self.schema);
        selection.source_time = self.source_time;
        selection.scope = self.scope;
        selection.stream = self.stream;
        selection.item = self.item;
        let mode = if replay {
            super::follow::Mode::Replay
        } else {
            super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<StoredRow>(selection, mode, execution)
            .await?;
        let decode = self.decode;
        let decode = Box::new(move |row: &StoredRow, bytes: NonZeroUsize| {
            let metadata = row
                .stream
                .memory_size()
                .saturating_sub(std::mem::size_of::<Identifier>());
            let allowance = NonZeroUsize::new(
                bytes
                    .get()
                    .checked_sub(metadata)
                    .ok_or_else(decoded_limit)?,
            )
            .ok_or_else(decoded_limit)?;
            let record = decode_record_borrowed(&row.record, decode.as_ref(), allowance)?;
            Ok(StoredRow {
                record,
                stream: row.stream.clone(),
                sequence: row.sequence,
            })
        });
        Ok(super::follow::QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

impl<C, T: MemorySize + 'static> StorageQuery<C> for RecordQuery<T> {
    type Reader = RecordQueryResult<T>;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn preflight(&self, manager: &StorageManager<C>, limits: QueryLimits) -> Result<()> {
        if let Some(filter) = &self.filter {
            filter.check()?;
            let _reservation = manager.reserve_read_now(filter.memory_size(), 1)?;
            let mut selection = self.options.selection(ReadKind::Records);
            selection.schema = Some(self.schema.clone());
            selection.record_filter = Some(filter.clone());
            manager
                .inner
                .primary
                .validate_record_filter(&selection, limits)?;
        }
        Ok(())
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let started = tokio::time::Instant::now();
        view.manager()
            .check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let mut selection = self.options.selection(ReadKind::Records);
        selection.schema = Some(self.schema);
        selection.source_time = self.source_time;
        selection.scope = self.scope;
        selection.item = self.item;
        selection.record_filter = self.filter;
        let decode = self.decode;
        computation.finish(QueryResult {
            retained_view: None,
            pending: None,
            reader: view.read_records(selection)?,
            decoded_bytes: view.manager().query_decoded_bytes(&self.options)?,
            page,
            decode: Box::new(move |record, bytes| decode_record(record, decode.as_ref(), bytes)),
        })
    }
}

impl<C, T: MemorySize + 'static> StorageQuery<C> for StreamQuery<T> {
    type Reader = StreamQueryResult<T>;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let started = tokio::time::Instant::now();
        view.manager()
            .check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let mut selection = self.options.selection(ReadKind::Stream);
        selection.schema = Some(self.schema);
        selection.source_time = self.source_time;
        selection.scope = self.scope;
        selection.stream = self.stream;
        selection.item = self.item;
        let decode = self.decode;
        computation.finish(QueryResult {
            retained_view: None,
            pending: None,
            reader: view.read_stream(selection)?,
            decoded_bytes: view.manager().query_decoded_bytes(&self.options)?,
            page,
            decode: Box::new(move |row, bytes| {
                Ok(StoredRow {
                    stream: row.stream,
                    sequence: row.sequence,
                    record: decode_record(row.record, decode.as_ref(), bytes)?,
                })
            }),
        })
    }
}

impl<C> FollowQuery<C> for SchemaQuery {
    type Raw = StorageSchema;
    type Value = StorageSchema;
    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<super::follow::QueryFollower<Self::Raw, Self::Value>> {
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        let mut selection = self.options.selection(ReadKind::Schemas);
        selection.schema = self.schema;
        let mode = if replay {
            super::follow::Mode::Replay
        } else {
            super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<StorageSchema>(selection, mode, execution)
            .await?;
        let decode = Box::new(|schema: &StorageSchema, allowance: NonZeroUsize| {
            if schema.memory_size() > allowance.get() {
                return Err(decoded_limit());
            }
            Ok(schema.clone())
        });
        Ok(super::follow::QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

impl<C> FollowQuery<C> for FileQuery {
    type Raw = FileReference;
    type Value = FileReference;
    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<super::follow::QueryFollower<Self::Raw, Self::Value>> {
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        let mut selection = self.options.selection(ReadKind::Files);
        selection.scope = self.scope;
        selection.item = self.item;
        let mode = if replay {
            super::follow::Mode::Replay
        } else {
            super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<FileReference>(selection, mode, execution)
            .await?;
        let decode = Box::new(|file: &FileReference, allowance: NonZeroUsize| {
            if file.memory_size() > allowance.get() {
                return Err(decoded_limit());
            }
            Ok(file.clone())
        });
        Ok(super::follow::QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

impl<C> StorageQuery<C> for SchemaQuery {
    type Reader = SchemaQueryResult;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let started = tokio::time::Instant::now();
        view.manager()
            .check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let mut selection = self.options.selection(ReadKind::Schemas);
        selection.schema = self.schema;
        computation.finish(QueryResult {
            retained_view: None,
            pending: None,
            reader: view.reader::<StorageSchema>(selection)?,
            decoded_bytes: view.manager().query_decoded_bytes(&self.options)?,
            page,
            decode: Box::new(|schema, _| Ok(schema)),
        })
    }
}

impl<C> StorageQuery<C> for FileQuery {
    type Reader = FileQueryResult;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let started = tokio::time::Instant::now();
        view.manager()
            .check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let mut selection = self.options.selection(ReadKind::Files);
        selection.history = ReadHistory::Retained;
        selection.scope = self.scope;
        selection.item = self.item;
        computation.finish(QueryResult {
            retained_view: None,
            pending: None,
            reader: view.read_files(selection)?,
            decoded_bytes: view.manager().query_decoded_bytes(&self.options)?,
            page,
            decode: Box::new(|file, _| Ok(file)),
        })
    }
}

impl<C: ObjectCatalog + Contains<T>, T: StoredObject> StorageQuery<C> for ObjectQuery<'_, T> {
    type Reader = ObjectQueryResult<T>;
    fn options(&self) -> &QueryOptions {
        &self.options
    }

    fn preflight(&self, manager: &StorageManager<C>, _limits: QueryLimits) -> Result<()> {
        manager
            .inner
            .primary
            .validate_object_versions(self.versions)
    }

    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let started = tokio::time::Instant::now();
        view.manager()
            .check_query_open(&view, &self.options, page, remaining)?;
        let computation = view
            .computation_with(Some(remaining), started.elapsed())?
            .ok_or(Error::ViewExpired)?;
        let mut selection = self.options.selection(ReadKind::Objects);
        let schema = object_schema::<T>()?;
        selection.schema = Some(schema.clone());
        selection.object_key = self
            .item
            .map(|key| object_key(key, page.bytes))
            .transpose()?;
        selection.history = self.history;
        selection.object_versions = self.versions;
        let decoding_memory = self.decoding_memory;
        computation.finish(QueryResult {
            retained_view: None,
            pending: None,
            reader: view.read_objects(selection)?,
            decoded_bytes: view.manager().query_decoded_bytes(&self.options)?,
            page,
            decode: Box::new(move |entry, allowance| {
                // Preserve Native Codec Errors Within The Original Output Preclaim.
                let error_bytes = std::mem::size_of::<crate::MessageError>();
                let native_bytes = allowance
                    .get()
                    .checked_sub(error_bytes)
                    .ok_or_else(decoded_limit)?;
                let required = match &decoding_memory {
                    Some(estimate) => estimate(&entry.value.payload)?,
                    None => T::decoding_memory(&entry.value.payload).map_err(|error| {
                        Error::backend("preflight object", crate::BackendFailure::Corrupt, error)
                    })?,
                };
                if required > native_bytes {
                    return Err(decoded_limit());
                }
                Ok(ObjectRevision {
                    value: decode_admitted::<T>(&entry.value, &schema)?,
                    position: entry.position,
                    version: entry.version,
                })
            }),
        })
    }
}

impl<C: ObjectCatalog + Contains<T>, T: StoredObject> FollowQuery<C> for ObjectQuery<'_, T> {
    type Raw = ObjectEntry;
    type Value = ObjectRevision<T>;

    async fn open_follow(
        self,
        manager: &StorageManager<C>,
        replay: bool,
    ) -> Result<super::follow::QueryFollower<Self::Raw, Self::Value>> {
        let started = tokio::time::Instant::now();
        let execution = manager.query_execution(&self.options)?;
        let page = manager.query_page(&self.options)?;
        let bytes = manager.query_decoded_bytes(&self.options)?;
        manager
            .inner
            .primary
            .validate_object_versions(self.versions)?;
        let schema = object_schema::<T>()?;
        let mut selection = self.options.selection(ReadKind::Objects);
        selection.schema = Some(schema.clone());
        selection.object_key = self
            .item
            .map(|key| object_key(key, page.bytes))
            .transpose()?;
        selection.history = self.history;
        selection.object_versions = self.versions;
        let mode = if replay {
            super::follow::Mode::Replay
        } else {
            super::follow::Mode::Live
        };
        let source = manager
            .follow_raw::<ObjectEntry>(selection, mode, execution)
            .await?;
        let decoding_memory = self.decoding_memory;
        let decode = Box::new(move |entry: &ObjectEntry, allowance: NonZeroUsize| {
            // Clone Metadata Only After The Original Pending Page's Output Preclaim.
            let metadata = entry
                .position
                .memory_size()
                .checked_add(entry.version.memory_size())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<crate::MessageError>()))
                .ok_or_else(decoded_limit)?;
            let native_bytes = allowance
                .get()
                .checked_sub(metadata)
                .ok_or_else(decoded_limit)?;
            let required = match &decoding_memory {
                Some(estimate) => estimate(&entry.value.payload)?,
                None => T::decoding_memory(&entry.value.payload).map_err(|error| {
                    Error::backend("preflight object", crate::BackendFailure::Corrupt, error)
                })?,
            };
            if required > native_bytes {
                return Err(decoded_limit());
            }
            Ok(ObjectRevision {
                value: decode_admitted::<T>(&entry.value, &schema)?,
                position: entry.position.clone(),
                version: entry.version.clone(),
            })
        });
        Ok(super::follow::QueryFollower::new(
            source,
            decode,
            page,
            bytes,
            remaining_budget(execution.timeout, started)?,
        ))
    }
}

impl<C> StorageManager<C> {
    pub async fn query<Q: StorageQuery<C>>(&self, query: Q) -> Result<Q::Reader> {
        let started = tokio::time::Instant::now();
        let page = self.query_page(query.options())?;
        let execution = if query.requested_cursor().is_some() {
            QueryLimits {
                memory_bytes: self.inner.settings.budget.read_memory_bytes,
                admission: Admission::Elastic,
                spill_bytes: 0,
                timeout: self.inner.settings.query_execution.timeout,
            }
        } else {
            self.query_execution(query.options())?
        };
        query.preflight(self, execution)?;
        let timeout = remaining_budget(execution.timeout, started)?;
        // FIFO Semaphore Admission; Queued Descriptions Remain Memory-Bounded.
        let description_bytes =
            query
                .request_memory_bytes()
                .checked_mul(4)
                .ok_or(Error::ResourceExhausted {
                    resource: "query plan memory",
                })?;
        let description = self.reserve_read_now(description_bytes, 1)?;
        let active = match self.inner.query_active.clone().try_acquire_owned() {
            Ok(active) => active,
            Err(_) => {
                let pending = self
                    .inner
                    .query_pending
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| Error::ResourceExhausted {
                        resource: "pending storage queries",
                    })?;
                let active =
                    tokio::time::timeout(timeout, self.inner.query_active.clone().acquire_owned())
                        .await
                        .map_err(|_| execution_timeout())?
                        .map_err(|_| Error::Closing {
                            domain: "storage queries",
                        })?;
                drop(pending);
                active
            }
        };
        let remaining = remaining_budget(execution.timeout, started)?;
        let acquire = async {
            match query.requested_cursor() {
                Some(at) => self.view_at(at, execution).await,
                None => self.view_with_limits(execution).await,
            }
        };
        let view = tokio::time::timeout(remaining, acquire)
            .await
            .map_err(|_| execution_timeout())??;
        view.retain_query_permit(active);
        view.retain_query_description(description);
        let remaining = execution.timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(execution_timeout());
        }
        view.limit_computation(remaining)?;
        query.open(view, page, remaining)
    }

    pub fn query_in<Q: StorageQuery<C>>(
        &self,
        view: &StorageView,
        mut query: Q,
    ) -> Result<Q::Reader> {
        let started = tokio::time::Instant::now();
        if !Arc::ptr_eq(&self.inner, &view.manager().inner) {
            return Err(Error::InvalidInput {
                field: "query view",
                reason: "view belongs to another manager",
            });
        }
        let computation = view.computation_with(None, started.elapsed())?;
        let page = self.query_page(query.options())?;
        // Queries Without Explicit Execution Limits Inherit The View Budget.
        let budget = view.inherited_budget(query.options().execution)?;
        query
            .options_mut()
            .ok_or(Error::InvalidInput {
                field: "query view",
                reason: "declarative queries own their cursor-bound view",
            })?
            .execution = Some(budget);
        let execution = self.query_execution(query.options())?;
        query.preflight(self, execution)?;
        drop(computation);
        query.open(
            view.clone(),
            page,
            remaining_budget(execution.timeout, started)?,
        )
    }

    // The sealed trait's public dispatch seam cannot open an unbounded raw view
    // or manufacture a larger page/time budget than the manager entry points.
    pub(super) fn check_query_open(
        &self,
        view: &StorageView,
        options: &QueryOptions,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<()> {
        let execution = options
            .execution
            .unwrap_or(self.inner.settings.query_execution);
        self.query_decoded_bytes(options)?;
        if view.execution_limits()? != Some(execution)
            || page != self.query_page(options)?
            || remaining.is_zero()
            || remaining > execution.timeout
        {
            return Err(Error::InvalidInput {
                field: "query admission",
                reason: "view, page or computation budget does not match query admission",
            });
        }
        Ok(())
    }

    pub(super) fn query_execution(&self, options: &QueryOptions) -> Result<QueryLimits> {
        self.query_decoded_bytes(options)?;
        let execution = options
            .execution
            .unwrap_or(self.inner.settings.query_execution);
        execution.check()?;
        self.inner.primary.validate_query_limits(execution)?;
        Ok(execution)
    }

    pub(super) fn query_decoded_bytes(&self, options: &QueryOptions) -> Result<NonZeroUsize> {
        let bytes = options
            .decoded_bytes
            .unwrap_or(self.inner.settings.query_decoded_bytes);
        if bytes > self.inner.settings.budget.read_memory_bytes {
            return Err(decoded_limit());
        }
        Ok(bytes)
    }

    fn query_page(&self, options: &QueryOptions) -> Result<ReadLimits> {
        let page = options.page.unwrap_or(self.inner.settings.query_page);
        // A Page That Can Never Fit The Shared Budget Is Refused Rather Than Awaited.
        if page.bytes > self.inner.settings.budget.read_memory_bytes {
            return Err(Error::InvalidInput {
                field: "read page",
                reason: "page exceeds read memory budget",
            });
        }
        Ok(page)
    }
}

impl<R, T: MemorySize> QueryResult<R, T> {
    pub fn prepare_work(&self) -> Result<super::StorageReadWorkPreparation> {
        let view = self.view().cloned().ok_or(Error::ViewExpired)?;
        super::StorageReadWorkPreparation::saved(view)
    }

    pub fn decoded_bytes(&self) -> NonZeroUsize {
        self.decoded_bytes
    }

    pub fn view(&self) -> Option<&StorageView> {
        self.retained_view.as_ref().or_else(|| self.reader.view())
    }

    pub fn retain_view(&mut self) -> Result<()> {
        if self.retained_view.is_none() {
            let view = self.reader.view().ok_or(Error::ViewExpired)?;
            let _live = view.pin()?;
            self.retained_view = Some(view.clone());
        }
        Ok(())
    }

    pub fn project<U: MemorySize, V: MemorySize>(
        &mut self,
        input: &QueryValue<U>,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&U, NonZeroUsize) -> Result<V>,
    ) -> Result<QueryValue<V>> {
        let view = self.view().cloned().ok_or(Error::ViewExpired)?;
        input.check_origin(view.manager())?;
        let computation = view.computation()?.ok_or(Error::ViewExpired)?;
        let timeout = computation.timeout();
        let started = tokio::time::Instant::now();
        let _live = view.pin()?;
        if started.elapsed() >= timeout {
            return Err(execution_timeout());
        }
        let output = input.try_project(max_bytes, map)?;
        if started.elapsed() >= timeout {
            return Err(execution_timeout());
        }
        computation.finish(output)
    }

    pub async fn next_page(&mut self) -> Result<Option<QueryPage<T>>> {
        match self.next_page_with(false, None, |_| Ok(())).await? {
            Some(()) => Ok(self.pending.take()),
            None => Ok(None),
        }
    }

    pub async fn next_page_projected<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<Option<QueryValue<U>>> {
        self.project_page(false, max_bytes, map).await
    }

    pub async fn next_page_projected_including_empty<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<Option<QueryValue<U>>> {
        self.project_page(true, max_bytes, map).await
    }

    async fn project_page<U: MemorySize>(
        &mut self,
        include_empty: bool,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<Option<QueryValue<U>>> {
        let value = self
            .next_page_with(include_empty, Some(max_bytes), |page| {
                page.try_project(max_bytes, map)
            })
            .await?;
        if value.is_some() {
            self.pending.take();
        }
        Ok(value)
    }

    async fn next_page_with<U>(
        &mut self,
        include_empty: bool,
        projection: Option<NonZeroUsize>,
        map: impl FnOnce(&QueryPage<T>) -> Result<U>,
    ) -> Result<Option<U>> {
        if self.reader.exhausted() {
            return Ok(None);
        }
        let view = self.reader.view().cloned().ok_or(Error::ViewExpired)?;
        let computation = view.begin_computation_wait().await?;
        let timeout = computation.timeout();
        let started = tokio::time::Instant::now();
        let mut working = None;
        while self.pending.is_none() {
            let mut required = 0usize;
            let decode = &self.decode;
            let bytes = self.decoded_bytes;
            let page = tokio::time::timeout(
                timeout.saturating_sub(started.elapsed()),
                self.reader
                    .stage_page_mapped(self.page, |entries, raw_bytes| {
                        required = raw_bytes
                            .checked_add(bytes.get())
                            .and_then(|total| total.checked_add(ENTRY_OVERHEAD))
                            .ok_or_else(decoded_limit)?;
                        let mut reservation = view.reserve_read(bytes.get(), 1)?;
                        let inline = entries
                            .len()
                            .checked_mul(std::mem::size_of::<T>())
                            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<T>>()))
                            .ok_or_else(decoded_limit)?;
                        if inline > bytes.get() {
                            return Err(decoded_limit());
                        }
                        let mut result = Vec::with_capacity(entries.len());
                        let mut retained = std::mem::size_of::<Vec<T>>().saturating_add(
                            result.capacity().saturating_mul(std::mem::size_of::<T>()),
                        );
                        if retained > bytes.get() {
                            return Err(decoded_limit());
                        }
                        for entry in entries {
                            if started.elapsed() >= timeout {
                                return Err(execution_timeout());
                            }
                            let allowance = NonZeroUsize::new(bytes.get() - retained)
                                .ok_or_else(decoded_limit)?;
                            let value = decode(entry, allowance)?;
                            retained = retained.saturating_add(
                                value.memory_size().saturating_sub(std::mem::size_of::<T>()),
                            );
                            if retained > bytes.get() {
                                return Err(decoded_limit());
                            }
                            result.push(value);
                        }
                        if started.elapsed() >= timeout {
                            return Err(execution_timeout());
                        }
                        reservation.retain_read(retained.saturating_add(ENTRY_OVERHEAD));
                        Ok(QueryPage {
                            entries: result,
                            _reservation: reservation,
                        })
                    }),
            )
            .await
            .map_err(|_| execution_timeout())?;
            let page = match page {
                Ok(page) => page,
                Err(Error::ResourceExhausted {
                    resource: "storage read memory",
                }) if required != 0 => {
                    // The Failed Mapping Released Raw And Partial Decoded Data.
                    // Admit Their Combined Peak Before Reloading The Same Cursor.
                    drop(working.take());
                    working = Some(view.reserve_working_wait(required).await?);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let Some((page, _empty)) = page else {
                return Ok(None);
            };
            self.pending = Some(page);
        }
        drop(working);
        if !include_empty && self.pending.as_ref().is_some_and(|page| page.is_empty()) {
            computation.check()?;
            self.pending.take();
            self.reader.commit_staged_page();
            return Ok(None);
        }
        // Admit Output Before Invoking The One-Shot Projection Callback.
        let _projection = if let Some(bytes) = projection {
            let required = bytes
                .get()
                .checked_add(ENTRY_OVERHEAD)
                .ok_or_else(decoded_limit)?;
            Some(
                tokio::time::timeout(
                    timeout.saturating_sub(started.elapsed()),
                    view.reserve_working_wait(required),
                )
                .await
                .map_err(|_| execution_timeout())??,
            )
        } else {
            None
        };
        // Retries Borrow The Same Decoded Page And Its Original Read Charge.
        let _live = self.reader.view().ok_or(Error::ViewExpired)?.pin()?;
        let projected = map(self
            .pending
            .as_ref()
            .expect("original decoded page retained"))?;
        if started.elapsed() >= timeout {
            return Err(execution_timeout());
        }
        computation.check()?;
        self.reader.commit_staged_page();
        Ok(Some(projected))
    }
}

impl<T> Deref for QueryPage<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.entries
    }
}

impl<T> QueryPage<T> {
    pub(super) async fn reserve_projection_wait(
        &self,
        bytes: NonZeroUsize,
        timeout: Duration,
    ) -> Result<MemoryReservation> {
        self._reservation.projection_wait(bytes, timeout).await
    }

    pub(super) fn project_reserved<U: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        reservation: MemoryReservation,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let value = map(&self.entries, max_bytes)?;
        if value.memory_size() > max_bytes.get() {
            return Err(decoded_limit());
        }
        QueryValue::retain(value, reservation)
    }

    pub fn try_project<U: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let reservation = self._reservation.projection(max_bytes)?;
        self.project_reserved(max_bytes, reservation, map)
    }
}

impl<T: MemorySize> QueryPage<T> {
    pub fn try_into_value<U: MemorySize>(
        self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(Vec<T>, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let Self {
            entries,
            _reservation: mut reservation,
        } = self;
        reservation.reserve_projection(max_bytes)?;
        let value = map(entries, max_bytes)?;
        if value.memory_size() > max_bytes.get() {
            return Err(decoded_limit());
        }
        QueryValue::retain(value, reservation)
    }

    pub(super) fn retain(entries: Vec<T>, mut reservation: MemoryReservation) -> Result<Self> {
        let charged = entries.memory_size().saturating_add(ENTRY_OVERHEAD);
        if charged > reservation.bytes {
            return Err(decoded_limit());
        }
        reservation.retain_read(charged);
        Ok(Self {
            entries,
            _reservation: reservation,
        })
    }
}

impl<T: fmt::Debug> fmt::Debug for QueryPage<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.entries.fmt(formatter)
    }
}

impl<T> std::borrow::Borrow<T> for QueryValue<T> {
    fn borrow(&self) -> &T {
        &self.value
    }
}

impl<T> Deref for QueryValue<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for QueryValue<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

impl QueryValue<Vec<u8>> {
    // FileChunk Already Owns The Validated Buffer And Its Original Read Charge.
    pub(super) fn from_file_chunk(chunk: FileChunk) -> Self {
        let (value, reservation) = chunk.into_charged_parts();
        Self {
            value,
            _reservation: reservation,
            execution: None,
        }
    }
}

impl<T> QueryValue<T> {
    pub fn prepare_work(&self) -> Result<super::StorageReadWorkPreparation> {
        let execution = self.execution.as_ref().ok_or(Error::InvalidInput {
            field: "storage read work",
            reason: "value has no live producer profile",
        })?;
        super::StorageReadWorkPreparation::producer(execution.clone())
    }

    pub fn computation(&self) -> Result<super::StorageProductionComputation> {
        self.execution
            .as_ref()
            .ok_or(Error::InvalidInput {
                field: "storage production computation",
                reason: "value has no live producer profile",
            })?
            .begin()
    }

    pub fn check_origin<C>(&self, manager: &StorageManager<C>) -> Result<()> {
        if !Arc::ptr_eq(&manager.inner, &self._reservation.inner) {
            return Err(Error::InvalidInput {
                field: "query projection",
                reason: "input belongs to another read ledger",
            });
        }
        Ok(())
    }
}

impl<T: MemorySize> QueryValue<T> {
    pub fn try_into_shared(self) -> Result<SharedQueryValue<T>> {
        Ok(SharedQueryValue {
            owner: self.shared_owner(std::mem::size_of::<SharedQueryValue<T>>())?,
        })
    }

    pub fn try_into_response(self) -> Result<crate::operations::Response<T>>
    where
        T: Send + Sync + 'static,
    {
        let handle = std::mem::size_of::<crate::operations::Response<T>>();
        let handle = handle.max(std::mem::size_of::<crate::operations::StreamItem<T>>());
        let owner = self.shared_owner(handle)?;
        Ok(crate::operations::Response::from_owner(owner))
    }

    fn shared_owner(mut self, handle_bytes: usize) -> Result<Arc<Self>> {
        let metadata = std::mem::size_of::<MemoryReservation>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(handle_bytes))
            .ok_or_else(decoded_limit)?;
        let retained = self
            ._reservation
            .bytes
            .checked_add(metadata)
            .ok_or_else(decoded_limit)?;
        self._reservation
            .reserve_projection(NonZeroUsize::new(metadata).ok_or_else(decoded_limit)?)?;
        self._reservation.retain_read(retained);
        Ok(Arc::new(self))
    }

    pub fn try_project<U: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let computation = self
            .execution
            .as_ref()
            .map(|execution| execution.begin())
            .transpose()?;
        let output = self.project_in_computation(max_bytes, map)?;
        if let Some(computation) = &computation {
            computation.check()?;
        }
        Ok(output)
    }

    fn project_in_computation<U: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let reservation = self._reservation.projection(max_bytes)?;
        let value = map(&self.value, max_bytes)?;
        if value.memory_size() > max_bytes.get() {
            return Err(decoded_limit());
        }
        let mut output = QueryValue::retain(value, reservation)?;
        output.execution = self.execution.clone();
        Ok(output)
    }

    pub fn try_map<U: MemorySize>(
        self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        let Self {
            value,
            _reservation: mut reservation,
            execution,
        } = self;
        let computation = execution
            .as_ref()
            .map(|execution| execution.begin())
            .transpose()?;
        reservation.reserve_projection(max_bytes)?;
        let value = map(value, max_bytes)?;
        if value.memory_size() > max_bytes.get() {
            return Err(decoded_limit());
        }
        if let Some(computation) = &computation {
            computation.check()?;
        }
        let mut output = QueryValue::retain(value, reservation)?;
        output.execution = execution;
        Ok(output)
    }

    pub(super) fn retain(value: T, mut reservation: MemoryReservation) -> Result<Self> {
        let bytes = value.memory_size();
        let charged = bytes.saturating_add(ENTRY_OVERHEAD);
        if charged > reservation.bytes {
            return Err(decoded_limit());
        }
        reservation.retain_read(charged);
        Ok(Self {
            value,
            _reservation: reservation,
            execution: None,
        })
    }

    pub(super) fn from_production(
        value: T,
        reservation: MemoryReservation,
        execution: Option<Arc<super::production::Execution>>,
    ) -> Result<Self> {
        if execution.is_none() {
            return Self::retain(value, reservation);
        }
        if value.memory_size().saturating_add(ENTRY_OVERHEAD) > reservation.bytes {
            return Err(decoded_limit());
        }
        // The Original Source Keeps Its Preclaimed Working Workspace.
        Ok(Self {
            value,
            _reservation: reservation,
            execution,
        })
    }
}

impl<T> Clone for SharedQueryValue<T> {
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
        }
    }
}

impl<T> SharedQueryValue<T> {
    /// Charges temporary work to the same read pool as this value.
    /// Keep the returned claim with the work, including after caller cancellation.
    pub fn reserve_work(&self, bytes: NonZeroUsize) -> Result<super::arrow::RetainedMemory> {
        let reservation = self.owner._reservation.projection(bytes)?;
        Ok(super::manager::retain_memory(reservation))
    }

    pub fn prepare_work(&self) -> Result<super::StorageReadWorkPreparation> {
        self.owner.prepare_work()
    }

    pub fn computation(&self) -> Result<super::StorageProductionComputation> {
        self.owner.computation()
    }

    pub fn check_origin<C>(&self, manager: &StorageManager<C>) -> Result<()> {
        self.owner.check_origin(manager)
    }
}

impl super::StorageProductionComputation {
    pub fn project<T: MemorySize, U: MemorySize>(
        &self,
        input: &QueryValue<T>,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        if !input
            .execution
            .as_ref()
            .is_some_and(|execution| Arc::ptr_eq(execution, &self.execution))
        {
            return Err(Error::InvalidInput {
                field: "storage production computation",
                reason: "input belongs to another producer profile",
            });
        }
        self.check()?;
        let output = input.project_in_computation(max_bytes, map)?;
        self.check()?;
        Ok(output)
    }

    pub fn project_shared<T: MemorySize, U: MemorySize>(
        &self,
        input: &SharedQueryValue<T>,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        self.project(&input.owner, max_bytes, map)
    }
}

impl<T> Deref for SharedQueryValue<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.owner.value
    }
}

impl<T: MemorySize> SharedQueryValue<T> {
    pub fn try_project<U: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        self.owner.try_project(max_bytes, map)
    }
}

impl<T: fmt::Debug> fmt::Debug for SharedQueryValue<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.owner.value.fmt(formatter)
    }
}

/* === Public Functions === */
pub(super) fn decoded_limit() -> Error {
    Error::ResourceExhausted {
        resource: "query decoded memory",
    }
}

pub(super) fn remaining_budget(
    budget: Duration,
    started: tokio::time::Instant,
) -> Result<Duration> {
    let remaining = budget.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(execution_timeout());
    }
    Ok(remaining)
}

pub(super) fn execution_timeout() -> Error {
    Error::ResourceExhausted {
        resource: "query computation time",
    }
}

/* === Private Functions === */
fn decode_record_borrowed<T>(
    record: &StoredRecord,
    decode: &PayloadDecode<T>,
    bytes: NonZeroUsize,
) -> Result<StoredRecord<T>> {
    let metadata = record
        .memory_size()
        .saturating_sub(std::mem::size_of::<StoredRecord>())
        .saturating_sub(record.payload.capacity());
    let allowance = NonZeroUsize::new(
        bytes
            .get()
            .checked_sub(metadata)
            .ok_or_else(decoded_limit)?,
    )
    .ok_or_else(decoded_limit)?;
    let payload = decode(&record.payload, allowance)?;
    Ok(StoredRecord {
        position: record.position.clone(),
        schema: record.schema.clone(),
        scope: record.scope.clone(),
        item: record.item.clone(),
        source: record.source.clone(),
        source_time: record.source_time,
        payload,
    })
}

fn decode_record<T>(
    record: StoredRecord,
    decode: &PayloadDecode<T>,
    bytes: NonZeroUsize,
) -> Result<StoredRecord<T>> {
    let payload = decode(&record.payload, bytes)?;
    Ok(StoredRecord {
        position: record.position,
        schema: record.schema,
        scope: record.scope,
        item: record.item,
        source: record.source,
        source_time: record.source_time,
        payload,
    })
}

/* === Tests === */
