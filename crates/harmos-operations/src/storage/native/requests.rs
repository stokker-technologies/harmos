/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Storage Requests and Results

/* === Modules === */

/* === Re-Exports === */

pub use harmos_domain::model::storage::query::transform::{TimeBucketPlan, Transform};
pub use harmos_domain::model::storage::{
    ByteRange, CaptureConsistency, CompoundOutcome, DataBatch, DirectoryCompletion, DirectoryEntry, DirectoryEvent, DirectorySelection,
    FileBatch, FileCompletion, FileEvent, FileHash, FileMetadata, FileProgress, FileReference, FileSession, FileSource, FollowStart,
    MutationId, MutationItem, MutationWindow, ObjectEntry, ObjectKey, ObjectValue, ObjectVersionToken, SchemaKey, StagedFile,
    StorageCursor, StoreHistory, StoredRecord, StoredRow,
};
pub use harmos_storage::query::Source;
pub use harmos_storage::query::columnar::StorageCapabilities;
pub use harmos_storage::{FileProducer, FileProduction, ReadLimits};

/* === Imports === */

// Standard Library Imports
use std::mem;

// Third Party Imports
use prost::Message;

// Local Imports
use harmos_domain::model::storage::query::TimeMerge;
use harmos_domain::schema::graph::Schema;
use harmos_domain::{Identifier, MemorySize};
use harmos_protocol::model::{PageRequest, ReadCut, Receipt};
use harmos_query::{columnar, index};
use harmos_storage::query::projection;

/* === Constants === */

/* === Macros === */

macro_rules! memory {
    ($ty:ty, $($field:ident),*) => {
        impl MemorySize for $ty { fn memory_size(&self) -> usize {
            mem::size_of::<Self>()$(.saturating_add(self.$field.memory_size()))*
        } }
    };
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ListFileProductionsRequest {
    pub scope: Identifier,
    pub page: Option<PageRequest>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListFileProductionsResponse {
    pub productions: Vec<FileProduction>,
    pub next_page_token: String,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColumnSource {
    pub kind: ColumnKind,
    pub schema: SchemaKey,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ColumnPredicate {
    pub column: String,
    pub lower: Option<f64>,
    pub upper: Option<f64>,

    /// Nonempty Case-Sensitive TEXT Prefix; Numeric Bounds Must Be Absent.
    pub prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TimeBounds {
    pub column: String,
    pub lower_ns: i64,
    pub upper_ns: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexDefinition {
    pub name: Identifier,
    pub version: u32,
    pub source: ColumnSource,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionDefinition {
    pub name: Identifier,
    pub version: u32,
    pub source: ColumnSource,
    pub transform: ProjectionTransform,
}

/// A Self-Contained Arrow IPC Batch and Its Announced Schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpcBatch {
    pub schema: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnPage {
    pub batch: IpcBatch,
    pub complete: bool,
    pub scanned: usize,
    pub bytes_read: usize,
    pub columns_read: Vec<String>,
    pub payload_rows: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub key: Vec<u8>,
    pub position: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexPage {
    pub entries: Vec<IndexEntry>,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionPage {
    pub batch: IpcBatch,
    pub frontier: StorageCursor,
    pub revision: u64,
    pub changed_keys: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionStatus {
    pub frontier: StorageCursor,
    pub revision: u64,
    pub rows: usize,
    pub changed_keys: usize,
}

#[derive(Clone, Debug)]
pub struct DeleteScopeRequest {
    pub mutation: Option<MutationId>,
    pub scope: Identifier,
}

#[derive(Clone, Debug)]
pub struct DeleteObjectRequest {
    pub mutation: Option<MutationId>,
    pub schema: Identifier,
    pub item: ObjectKey,
    pub expected: Option<ObjectVersionToken>,
}

#[derive(Clone, Debug)]
pub struct MutateRequest {
    pub mutation: Option<MutationId>,
    pub items: Vec<MutationItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MutateResponse {
    pub outcome: CompoundOutcome,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedSnapshot {
    pub token: Vec<u8>,
    pub at: Option<StorageCursor>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaInput {
    pub key: Option<SchemaKey>,
    pub graph: Option<Schema>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct ColumnSelection {
    pub source: Option<ColumnSource>,
    pub columns: Vec<String>,
    pub predicate: Option<ColumnPredicate>,
    pub time: Option<TimeBounds>,
    pub start: u64,
    pub work: Option<ReadLimits>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct IndexSelection {
    pub name: String,
    pub lower: Option<Vec<u8>>,
    pub upper: Option<Vec<u8>>,
    pub prefix: Vec<u8>,
    pub descending: bool,
    pub work: Option<ReadLimits>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct ProjectionSelection {
    pub name: String,
    pub frontier: Option<StorageCursor>,
    pub revision: Vec<u8>,
    pub work: Option<ReadLimits>,
}

#[derive(Clone, Debug)]
pub struct CreateFileProductionRequest {
    pub metadata: Option<FileMetadata>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateFileProductionResponse {
    pub production: Option<FileProducer>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct GetFileProductionRequest {
    pub file: Option<StagedFile>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetFileProductionResponse {
    pub production: Option<FileProgress>,
    pub writer_generation: u64,
    pub resolved_at: Option<ReadCut>,
    pub accepted_length: u64,
    pub durable_length: u64,
}

#[derive(Clone, Debug)]
pub struct AppendFileBytesRequest {
    pub session: Option<FileSession>,
    pub offset: u64,
    pub data: Vec<u8>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppendFileBytesResponse {
    pub production: Option<FileProducer>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct CreateFileWriterRequest {
    pub session: Option<FileSession>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateFileWriterResponse {
    pub production: Option<FileProducer>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct PublishFileProductionRequest {
    pub session: Option<FileSession>,
    pub length: u64,
    pub sha256: Vec<u8>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishFileProductionResponse {
    pub file: Option<FileReference>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct DeleteFileProductionRequest {
    pub session: Option<FileSession>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteFileProductionResponse {
    pub production: Option<FileProgress>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct GetFileBytesRequest {
    pub file: Option<FileReference>,
    pub offset: u64,
    pub max_bytes: u64,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetFileBytesResponse {
    pub data: Vec<u8>,
    pub total_length: u64,
    pub sha256: Vec<u8>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct CapabilitiesRequest {}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesResponse {
    pub capabilities: Option<StorageCapabilities>,
    pub mutations: Option<MutationWindow>,
    pub file_batch_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct CreateSnapshotRequest {
    pub at: Option<ReadCut>,
}

/// Durable Ownership of One Scope's Cut; Scope Deletion Releases It.
#[derive(Clone, Debug)]
pub struct RetainSnapshotRequest {
    pub at: Option<ReadCut>,
    pub scope: Option<Identifier>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateSnapshotResponse {
    pub snapshot: Option<SavedSnapshot>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct DeleteSnapshotRequest {
    pub snapshot: Option<SavedSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteSnapshotResponse {}

#[derive(Clone, Debug)]
pub struct ListColumnRowsRequest {
    pub selection: Option<ColumnSelection>,
    pub at: Option<ReadCut>,
    pub page: Option<PageRequest>,
}

#[derive(Debug)]
pub struct ListColumnRowsResponse {
    pub page: Option<ColumnPage>,
    pub resolved_at: Option<ReadCut>,
    pub next_page_token: String,
}

#[derive(Clone, Debug)]
pub struct ListIndexEntriesRequest {
    pub selection: Option<IndexSelection>,
    pub at: Option<ReadCut>,
    pub page: Option<PageRequest>,
    pub source: Option<ColumnSource>,
}

#[derive(Debug)]
pub struct ListIndexEntriesResponse {
    pub page: Option<IndexPage>,
    pub resolved_at: Option<ReadCut>,
    pub next_page_token: String,
}

#[derive(Clone, Debug)]
pub struct SetIndexDefinitionRequest {
    pub definition: Option<IndexDefinition>,
    pub expected_revision: Vec<u8>,
    pub create_if_absent: bool,
    pub rebuild: bool,
    pub work: Option<ReadLimits>,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SetIndexDefinitionResponse {
    pub definition: Option<IndexDefinition>,
    pub revision: Vec<u8>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct SetProjectionDefinitionRequest {
    pub definition: Option<ProjectionDefinition>,
    pub expected_revision: Vec<u8>,
    pub create_if_absent: bool,
    pub rebuild: bool,
    pub mutation: Option<MutationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SetProjectionDefinitionResponse {
    pub definition: Option<ProjectionDefinition>,
    pub revision: Vec<u8>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct AdvanceProjectionRequest {
    pub snapshot: Option<SavedSnapshot>,
    pub name: String,
    pub work: Option<ReadLimits>,
    pub source: Option<ColumnSource>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdvanceProjectionResponse {
    pub revision: Vec<u8>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct ListProjectionRowsRequest {
    pub selection: Option<ProjectionSelection>,
    pub at: Option<ReadCut>,
    pub page: Option<PageRequest>,
    pub source: Option<ColumnSource>,
}

#[derive(Debug)]
pub struct ListProjectionRowsResponse {
    pub page: Option<ProjectionPage>,
    pub resolved_at: Option<ReadCut>,
    pub next_page_token: String,
}

#[derive(Clone, Debug)]
pub struct GetProjectionStatusRequest {
    pub name: String,
    pub revision: Vec<u8>,
    pub at: Option<ReadCut>,
    pub source: Option<ColumnSource>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetProjectionStatusResponse {
    pub status: Option<ProjectionStatus>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct SetObjectRequest {
    pub mutation: Option<MutationId>,
    pub expected_revision: Vec<u8>,
    pub create_if_absent: bool,
    pub schema: Option<SchemaInput>,
    pub value: Option<ObjectValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SetObjectResponse {
    pub outcome: Option<set_object_response::Outcome>,
}

#[derive(Clone, Debug)]
pub struct ListRowsRequest {
    pub merge: Option<TimeMerge>,
    pub source: Option<Source>,
    pub at: Option<ReadCut>,
    pub transforms: Vec<Transform>,
    pub page: Option<PageRequest>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListRowsResponse {
    pub rows: Vec<RowOutput>,
    pub resolved_at: Option<ReadCut>,
    pub next_page_token: String,
}

#[derive(Clone, Debug)]
pub struct WatchRowsRequest {
    pub source: Option<Source>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WatchRowsResponse {
    pub resolved_at: Option<ReadCut>,
    pub event: Option<watch_rows_response::Event>,
}

#[derive(Clone, Debug)]
pub struct AppendRecordsRequest {
    pub mutation: Option<MutationId>,
    pub scope: Option<String>,
    pub batch: Option<DataBatch>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppendRecordsResponse {
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct AppendSamplesRequest {
    pub mutation: Option<MutationId>,
    pub scope: Option<String>,
    pub batch: Option<DataBatch>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppendSamplesResponse {
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct AppendObjectRevisionsRequest {
    pub mutation: Option<MutationId>,
    pub scope: Option<String>,
    pub batch: Option<DataBatch>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppendObjectRevisionsResponse {
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct GetFileHashRequest {
    pub source: Option<FileSource>,
    pub range: Option<ByteRange>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetFileHashResponse {
    pub hash: Option<FileHash>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct CaptureFileRequest {
    pub source: Option<FileSource>,
    pub range: Option<ByteRange>,
    pub metadata: Option<FileMetadata>,
    pub consistency: CaptureConsistency,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureFileResponse {
    pub file: Option<FileReference>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct WatchFileBytesRequest {
    pub source: Option<FileSource>,
    pub start: Option<FollowStart>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WatchFileBytesResponse {
    pub event: Option<FileEvent>,
    pub completion: Option<FileCompletion>,
}

#[derive(Clone, Debug)]
pub struct WatchFileProductionRequest {
    pub file: Option<StagedFile>,
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WatchFileProductionResponse {
    pub event: Option<watch_file_production_response::Event>,
}

#[derive(Clone, Debug)]
pub struct ListDirectoryEntriesRequest {
    pub selection: Option<DirectorySelection>,
    pub page: Option<PageRequest>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListDirectoryEntriesResponse {
    pub entries: Vec<DirectoryEntry>,
    pub next_page_token: String,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct WatchDirectoryEntriesRequest {
    pub selection: Option<DirectorySelection>,
    pub include_existing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WatchDirectoryEntriesResponse {
    pub event: Option<DirectoryEvent>,
}

#[derive(Clone, Debug)]
pub struct GetIndexDefinitionRequest {
    pub name: String,
    pub source: Option<ColumnSource>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetIndexDefinitionResponse {
    pub definition: Option<IndexDefinition>,
    pub revision: Vec<u8>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct GetProjectionDefinitionRequest {
    pub name: String,
    pub source: Option<ColumnSource>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetProjectionDefinitionResponse {
    pub definition: Option<ProjectionDefinition>,
    pub revision: Vec<u8>,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct GetObjectRequest {
    pub schema: Option<SchemaKey>,
    pub item: String,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetObjectResponse {
    pub value: Option<ObjectValue>,
    pub revision: Vec<u8>,
    pub exists: bool,
    pub resolved_at: Option<ReadCut>,
}

#[derive(Clone, Debug)]
pub struct InfoRequest {}

#[derive(Clone, Debug, PartialEq)]
pub struct InfoResponse {
    pub accepted: Option<StorageCursor>,
    pub saved: Option<StorageCursor>,
    pub mutations: Option<MutationWindow>,
    pub file_batch_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct DeleteFileRequest {
    pub mutation: Option<MutationId>,
    pub file: Option<StagedFile>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteFileResponse {
    pub file: Option<StagedFile>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct SetSchemaRequest {
    pub mutation: Option<MutationId>,
    pub schema: Option<SchemaInput>,
    pub expected_revision: Vec<u8>,
    pub create_if_absent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SetSchemaResponse {
    pub schema: Option<SchemaInput>,
    pub revision: Vec<u8>,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug)]
pub struct GetSchemaRequest {
    pub key: Option<SchemaKey>,
    pub at: Option<ReadCut>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetSchemaResponse {
    pub schema: Option<SchemaInput>,
    pub revision: Vec<u8>,
    pub exists: bool,
    pub resolved_at: Option<ReadCut>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColumnKind {
    Objects,
    Records,
    Stream,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProjectionTransform {
    Select(Vec<String>),
    TimeBuckets(TimeBucketPlan),
}

/// Ordered Arrow IPC Schema and Batch Frames Returned by ListRows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowOutput {
    Schema(Vec<u8>),
    Batch(Vec<u8>),
}

pub mod set_object_response {
    use super::*;
    #[derive(Clone, Debug, PartialEq)]
    pub struct Applied {
        pub value: Option<ObjectValue>,
        pub revision: Vec<u8>,
        pub receipt: Option<Receipt>,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct Conflict {
        pub current_revision: Vec<u8>,
        pub action: String,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub enum Outcome {
        Applied(Box<Applied>),
        Conflict(Conflict),
    }
}
pub mod watch_rows_response {
    use super::*;
    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        Record(StoredRecord),
        Sample(StoredRow),
        Object(ObjectEntry),
        Boundary(()),
        DurableProgress(()),
        ResyncRequired(String),
    }
}
pub mod watch_file_production_response {
    use super::*;
    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        Data(FileBatch),
        Progress(FileProgress),
    }
}

/* === Traits === */

/* === Implementations === */

memory!(ColumnSource, schema);
memory!(ColumnPredicate, column, prefix);
memory!(TimeBounds, column);
memory!(IndexDefinition, name, source, columns);
memory!(ProjectionDefinition, name, source, transform);

impl MemorySize for ProjectionTransform {
    fn memory_size(&self) -> usize {
        let bytes = match self {
            Self::Select(columns) => columns.memory_size(),
            Self::TimeBuckets(plan) => plan.memory_size(),
        };
        mem::size_of::<Self>().saturating_add(bytes)
    }
}

impl ColumnSource {
    pub(super) fn into_engine(self) -> columnar::ColumnSource {
        let kind = match self.kind {
            ColumnKind::Objects => columnar::ColumnKind::Objects,
            ColumnKind::Records => columnar::ColumnKind::Records,
            ColumnKind::Stream => columnar::ColumnKind::Stream,
        };
        columnar::ColumnSource { kind, schema: self.schema }
    }

    fn from_engine(source: columnar::ColumnSource) -> Self {
        let kind = match source.kind {
            columnar::ColumnKind::Objects => ColumnKind::Objects,
            columnar::ColumnKind::Records => ColumnKind::Records,
            columnar::ColumnKind::Stream => ColumnKind::Stream,
        };
        Self { kind, schema: source.schema }
    }
}

impl IndexDefinition {
    pub(super) fn into_engine(self) -> index::IndexDefinition {
        index::IndexDefinition {
            name: self.name,
            version: self.version,
            source: self.source.into_engine(),
            columns: self.columns,
        }
    }

    pub(super) fn from_engine(definition: index::IndexDefinition) -> Self {
        Self {
            name: definition.name,
            version: definition.version,
            source: ColumnSource::from_engine(definition.source),
            columns: definition.columns,
        }
    }
}

impl ProjectionDefinition {
    pub(super) fn into_engine(self) -> projection::ProjectionDefinition {
        let transform = match self.transform {
            ProjectionTransform::Select(columns) => projection::ProjectionTransform::Select(columns),
            ProjectionTransform::TimeBuckets(plan) => projection::ProjectionTransform::TimeBuckets(plan),
        };
        projection::ProjectionDefinition {
            name: self.name,
            version: self.version,
            source: self.source.into_engine(),
            transform,
        }
    }

    pub(super) fn from_engine(definition: projection::ProjectionDefinition) -> Self {
        let transform = match definition.transform {
            projection::ProjectionTransform::Select(columns) => ProjectionTransform::Select(columns),
            projection::ProjectionTransform::TimeBuckets(plan) => ProjectionTransform::TimeBuckets(plan),
        };
        Self {
            name: definition.name,
            version: definition.version,
            source: ColumnSource::from_engine(definition.source),
            transform,
        }
    }
}

memory!(IpcBatch, schema, data);
memory!(ColumnPage, batch, columns_read);
memory!(IndexEntry, key);
memory!(IndexPage, entries);
memory!(ProjectionPage, batch, frontier, changed_keys);
memory!(ProjectionStatus, frontier);

impl MemorySize for RowOutput {
    fn memory_size(&self) -> usize {
        let bytes = match self {
            Self::Schema(bytes) | Self::Batch(bytes) => bytes,
        };
        mem::size_of::<Self>().saturating_add(bytes.capacity())
    }
}

memory!(DeleteObjectRequest, mutation, schema, item, expected);
memory!(DeleteScopeRequest, mutation, scope);

memory!(MutateRequest, mutation, items);
impl MemorySize for MutateResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.outcome.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}

impl From<harmos_storage::StorageInfo> for InfoResponse {
    fn from(value: harmos_storage::StorageInfo) -> Self {
        Self {
            accepted: Some(value.accepted),
            saved: Some(value.saved),
            mutations: value.mutations,
            file_batch_bytes: value.file_batch_bytes,
        }
    }
}

memory!(SavedSnapshot, token, at);
memory!(SchemaInput, key, graph);
memory!(ColumnSelection, source, columns, predicate, time, work);
memory!(IndexSelection, name, lower, upper, prefix, work);
memory!(ProjectionSelection, name, frontier, work);
impl MemorySize for set_object_response::Outcome {
    fn memory_size(&self) -> usize {
        match self {
            Self::Applied(value) => value
                .value
                .memory_size()
                .saturating_add(value.revision.capacity())
                .saturating_add(value.receipt.as_ref().map_or(0, Message::encoded_len)),
            Self::Conflict(value) => value.current_revision.capacity().saturating_add(value.action.capacity()),
        }
    }
}
impl MemorySize for watch_rows_response::Event {
    fn memory_size(&self) -> usize {
        match self {
            Self::Record(value) => value.memory_size(),
            Self::Sample(value) => value.memory_size(),
            Self::Object(value) => value.memory_size(),
            Self::Boundary(_) | Self::DurableProgress(_) => 0,
            Self::ResyncRequired(value) => value.memory_size(),
        }
    }
}
impl MemorySize for watch_file_production_response::Event {
    fn memory_size(&self) -> usize {
        match self {
            Self::Data(value) => value.memory_size(),
            Self::Progress(value) => value.memory_size(),
        }
    }
}

impl MemorySize for CreateFileProductionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.metadata.memory_size()).saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for CreateFileProductionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.production.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileProductionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.file.memory_size()).saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileProductionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.production.memory_size())
            .saturating_add(self.writer_generation.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.accepted_length.memory_size())
            .saturating_add(self.durable_length.memory_size())
    }
}
impl MemorySize for AppendFileBytesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.session.memory_size())
            .saturating_add(self.offset.memory_size())
            .saturating_add(self.data.memory_size())
            .saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for AppendFileBytesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.production.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for CreateFileWriterRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.session.memory_size()).saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for CreateFileWriterResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.production.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for PublishFileProductionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.session.memory_size())
            .saturating_add(self.length.memory_size())
            .saturating_add(self.sha256.memory_size())
            .saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for PublishFileProductionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.file.memory_size()).saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for DeleteFileProductionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.session.memory_size()).saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for DeleteFileProductionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.production.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileBytesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.file.memory_size())
            .saturating_add(self.offset.memory_size())
            .saturating_add(self.max_bytes.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileBytesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.data.memory_size())
            .saturating_add(self.total_length.memory_size())
            .saturating_add(self.sha256.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for CapabilitiesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}
impl MemorySize for CapabilitiesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.capabilities.memory_size())
            .saturating_add(self.mutations.memory_size())
            .saturating_add(self.file_batch_bytes.memory_size())
    }
}
impl MemorySize for RetainSnapshotRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.at.as_ref().map_or(0, Message::encoded_len)).saturating_add(self.scope.memory_size())
    }
}
impl MemorySize for CreateSnapshotRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for CreateSnapshotResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.snapshot.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for DeleteSnapshotRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.snapshot.memory_size())
    }
}
impl MemorySize for DeleteSnapshotResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}
impl MemorySize for ListColumnRowsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.selection.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.page.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for ListColumnRowsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.page.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.next_page_token.memory_size())
    }
}
impl MemorySize for ListIndexEntriesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.selection.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.page.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.source.memory_size())
    }
}
impl MemorySize for ListIndexEntriesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.page.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.next_page_token.memory_size())
    }
}
impl MemorySize for SetIndexDefinitionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.expected_revision.memory_size())
            .saturating_add(self.create_if_absent.memory_size())
            .saturating_add(self.rebuild.memory_size())
            .saturating_add(self.work.memory_size())
            .saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for SetIndexDefinitionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for SetProjectionDefinitionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.expected_revision.memory_size())
            .saturating_add(self.create_if_absent.memory_size())
            .saturating_add(self.rebuild.memory_size())
            .saturating_add(self.mutation.memory_size())
    }
}
impl MemorySize for SetProjectionDefinitionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for AdvanceProjectionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.snapshot.memory_size())
            .saturating_add(self.name.memory_size())
            .saturating_add(self.work.memory_size())
            .saturating_add(self.source.memory_size())
    }
}
impl MemorySize for AdvanceProjectionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for ListProjectionRowsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.selection.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.page.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.source.memory_size())
    }
}
impl MemorySize for ListProjectionRowsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.page.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.next_page_token.memory_size())
    }
}
impl MemorySize for GetProjectionStatusRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.name.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.source.memory_size())
    }
}
impl MemorySize for GetProjectionStatusResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.status.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for SetObjectRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size())
            .saturating_add(self.expected_revision.memory_size())
            .saturating_add(self.create_if_absent.memory_size())
            .saturating_add(self.schema.memory_size())
            .saturating_add(self.value.memory_size())
    }
}
impl MemorySize for SetObjectResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.outcome.memory_size())
    }
}
impl MemorySize for ListRowsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.source.memory_size())
            .saturating_add(self.merge.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.transforms.memory_size())
            .saturating_add(self.page.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for ListRowsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.rows.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.next_page_token.memory_size())
    }
}
impl MemorySize for WatchRowsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.source.memory_size()).saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for WatchRowsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.event.memory_size())
    }
}
impl MemorySize for AppendRecordsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size())
            .saturating_add(self.scope.memory_size())
            .saturating_add(self.batch.memory_size())
    }
}
impl MemorySize for AppendRecordsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for AppendSamplesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size())
            .saturating_add(self.scope.memory_size())
            .saturating_add(self.batch.memory_size())
    }
}
impl MemorySize for AppendSamplesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for AppendObjectRevisionsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size())
            .saturating_add(self.scope.memory_size())
            .saturating_add(self.batch.memory_size())
    }
}
impl MemorySize for AppendObjectRevisionsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileHashRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.source.memory_size())
            .saturating_add(self.range.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetFileHashResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.hash.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for CaptureFileRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.source.memory_size())
            .saturating_add(self.range.memory_size())
            .saturating_add(self.metadata.memory_size())
            .saturating_add(self.consistency.memory_size())
    }
}
impl MemorySize for CaptureFileResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.file.memory_size()).saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for WatchFileBytesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.source.memory_size()).saturating_add(self.start.memory_size())
    }
}
impl MemorySize for WatchFileBytesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.event.memory_size()).saturating_add(self.completion.memory_size())
    }
}
impl MemorySize for WatchFileProductionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.file.memory_size()).saturating_add(self.offset.memory_size())
    }
}
impl MemorySize for WatchFileProductionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.event.memory_size())
    }
}
impl MemorySize for ListDirectoryEntriesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.selection.memory_size())
            .saturating_add(self.page.as_ref().map_or(0, Message::encoded_len))
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for ListDirectoryEntriesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.entries.memory_size())
            .saturating_add(self.next_page_token.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for WatchDirectoryEntriesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.selection.memory_size()).saturating_add(self.include_existing.memory_size())
    }
}
impl MemorySize for WatchDirectoryEntriesResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.event.memory_size())
    }
}
impl MemorySize for GetIndexDefinitionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.name.memory_size())
            .saturating_add(self.source.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetIndexDefinitionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetProjectionDefinitionRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.name.memory_size())
            .saturating_add(self.source.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetProjectionDefinitionResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.definition.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetObjectRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.schema.memory_size())
            .saturating_add(self.item.memory_size())
            .saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetObjectResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.value.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.exists.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for InfoRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}
impl MemorySize for InfoResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.accepted.memory_size())
            .saturating_add(self.saved.memory_size())
            .saturating_add(self.mutations.memory_size())
            .saturating_add(self.file_batch_bytes.memory_size())
    }
}
impl MemorySize for DeleteFileRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.mutation.memory_size()).saturating_add(self.file.memory_size())
    }
}
impl MemorySize for DeleteFileResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.file.memory_size()).saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for SetSchemaRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size())
            .saturating_add(self.schema.memory_size())
            .saturating_add(self.expected_revision.memory_size())
            .saturating_add(self.create_if_absent.memory_size())
    }
}
impl MemorySize for SetSchemaResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.schema.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.receipt.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetSchemaRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.key.memory_size()).saturating_add(self.at.as_ref().map_or(0, Message::encoded_len))
    }
}
impl MemorySize for GetSchemaResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.schema.memory_size())
            .saturating_add(self.revision.memory_size())
            .saturating_add(self.exists.memory_size())
            .saturating_add(self.resolved_at.as_ref().map_or(0, Message::encoded_len))
    }
}

impl MemorySize for ListFileProductionsRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            + self.scope.memory_size()
            + self.page.as_ref().map_or(0, Message::encoded_len)
            + self.at.as_ref().map_or(0, Message::encoded_len)
    }
}
impl MemorySize for ListFileProductionsResponse {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            + self.productions.memory_size()
            + self.next_page_token.capacity()
            + self.resolved_at.as_ref().map_or(0, Message::encoded_len)
    }
}

/* === Public Functions === */

pub(crate) fn required<T>(value: Option<T>) -> harmos_storage::Result<T> {
    value.ok_or(harmos_storage::Error::InvalidInput {
        field: "storage operation",
        reason: "required value is missing",
    })
}

/* === Private Functions === */

/* === Tests === */
