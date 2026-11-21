/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Storage Operation Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_operations::storage::{
    AdvanceProjectionRequest, AdvanceProjectionResponse, AppendFileBytesRequest, AppendFileBytesResponse, AppendObjectRevisionsRequest,
    AppendObjectRevisionsResponse, AppendRecordsRequest, AppendRecordsResponse, AppendSamplesRequest, AppendSamplesResponse, ByteRange,
    CapabilitiesRequest, CapabilitiesResponse, CaptureConsistency, CaptureFileRequest, CaptureFileResponse, ColumnKind, ColumnPage,
    ColumnPredicate, ColumnSelection, ColumnSource, CompoundOutcome, Contains, CopyBackendRequest, CreateFileProductionRequest,
    CreateFileProductionResponse, CreateFileWriterRequest, CreateFileWriterResponse, CreateSnapshotRequest, CreateSnapshotResponse,
    DataBatch, DeleteFileProductionRequest, DeleteFileProductionResponse, DeleteFileRequest, DeleteFileResponse, DeleteObjectRequest,
    DeleteScopeRequest, DeleteSnapshotRequest, DeleteSnapshotResponse, DirectoryCompletion, DirectoryEntry, DirectoryEvent,
    DirectorySelection, DisposeBackendRequest, FileBatch, FileCompletion, FileEvent, FileHash, FileMetadata, FileProducer, FileProduction,
    FileProgress, FileReference, FileSession, FileSource, FollowStart, GetFileBytesRequest, GetFileBytesResponse, GetFileHashRequest,
    GetFileHashResponse, GetFileProductionRequest, GetFileProductionResponse, GetIndexDefinitionRequest, GetIndexDefinitionResponse,
    GetObjectRequest, GetObjectResponse, GetProjectionDefinitionRequest, GetProjectionDefinitionResponse, GetProjectionStatusRequest,
    GetProjectionStatusResponse, GetSchemaRequest, GetSchemaResponse, IndexDefinition, IndexEntry, IndexPage, IndexSelection, InfoRequest,
    InfoResponse, IpcBatch, ListColumnRowsRequest, ListColumnRowsResponse, ListDirectoryEntriesRequest, ListDirectoryEntriesResponse,
    ListFileProductionsRequest, ListFileProductionsResponse, ListIndexEntriesRequest, ListIndexEntriesResponse, ListProjectionRowsRequest,
    ListProjectionRowsResponse, ListRowsRequest, ListRowsResponse, MutateRequest, MutateResponse, MutationId, MutationItem, MutationWindow,
    ObjectEntry, ObjectKey, ObjectValue, ObjectVersionToken, Offset, ProjectionDefinition, ProjectionPage, ProjectionSelection,
    ProjectionStatus, ProjectionTransform, PublishFileProductionRequest, PublishFileProductionResponse, ReadLimits, RowOutput,
    SavedSnapshot, ScalarValue, Scale, SchemaInput, SchemaKey, SetIndexDefinitionRequest, SetIndexDefinitionResponse, SetObjectRequest,
    SetObjectResponse, SetProjectionDefinitionRequest, SetProjectionDefinitionResponse, SetSchemaRequest, SetSchemaResponse, Source,
    StagedFile, StorageCapabilities, StorageCursor, StoreHistory, StoredRecord, StoredRow, TimeBounds, TimeBucketPlan, Transform,
    WatchDirectoryEntriesRequest, WatchDirectoryEntriesResponse, WatchFileBytesRequest, WatchFileBytesResponse, WatchFileProductionRequest,
    WatchFileProductionResponse, WatchRowsRequest, WatchRowsResponse, set_object_response, watch_file_production_response,
    watch_rows_response,
};
pub use harmos_protocol::model::{PageRequest, ReadCut, Receipt};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use crate::operations::{NativeInvocation, NativeOperation};
use harmos_operations::storage;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Storage<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Storage<'_> {
    pub async fn advance_projection(&self, request: AdvanceProjectionRequest) -> Result<AdvanceProjectionResponse> {
        self.call.call::<storage::AdvanceProjection>(request).await
    }

    pub async fn append_file_bytes(&self, request: AppendFileBytesRequest) -> Result<AppendFileBytesResponse> {
        self.call.call::<storage::AppendFileBytes>(request).await
    }

    pub async fn append_object_revisions(&self, request: AppendObjectRevisionsRequest) -> Result<AppendObjectRevisionsResponse> {
        self.call.call::<storage::AppendObjectRevisions>(request).await
    }

    pub async fn append_records(&self, request: AppendRecordsRequest) -> Result<AppendRecordsResponse> {
        self.call.call::<storage::AppendRecords>(request).await
    }

    pub async fn append_samples(&self, request: AppendSamplesRequest) -> Result<AppendSamplesResponse> {
        self.call.call::<storage::AppendSamples>(request).await
    }

    pub async fn capabilities(&self, request: CapabilitiesRequest) -> Result<CapabilitiesResponse> {
        self.call.call::<storage::Capabilities>(request).await
    }

    pub async fn capture_file(&self, request: CaptureFileRequest) -> Result<CaptureFileResponse> {
        self.call.call::<storage::CaptureFile>(request).await
    }

    pub async fn copy_backend(&self, request: CopyBackendRequest) -> Result<()> {
        self.call.call::<storage::CopyBackend>(request).await
    }

    pub async fn create_file_production(&self, request: CreateFileProductionRequest) -> Result<CreateFileProductionResponse> {
        self.call.call::<storage::CreateFileProduction>(request).await
    }

    pub async fn create_file_writer(&self, request: CreateFileWriterRequest) -> Result<CreateFileWriterResponse> {
        self.call.call::<storage::CreateFileWriter>(request).await
    }

    pub async fn create_snapshot(&self, request: CreateSnapshotRequest) -> Result<CreateSnapshotResponse> {
        self.call.call::<storage::CreateSnapshot>(request).await
    }

    pub async fn delete_file(&self, request: DeleteFileRequest) -> Result<DeleteFileResponse> {
        self.call.call::<storage::DeleteFile>(request).await
    }

    pub async fn delete_file_production(&self, request: DeleteFileProductionRequest) -> Result<DeleteFileProductionResponse> {
        self.call.call::<storage::DeleteFileProduction>(request).await
    }

    pub async fn delete_object(&self, request: DeleteObjectRequest) -> Result<MutateResponse> {
        self.call.call::<storage::DeleteObject>(request).await
    }

    pub async fn delete_scope(&self, request: DeleteScopeRequest) -> Result<MutateResponse> {
        self.call.call::<storage::DeleteScope>(request).await
    }

    pub async fn delete_snapshot(&self, request: DeleteSnapshotRequest) -> Result<DeleteSnapshotResponse> {
        self.call.call::<storage::DeleteSnapshot>(request).await
    }

    pub async fn dispose_backend(&self, request: DisposeBackendRequest) -> Result<()> {
        self.call.call::<storage::DisposeBackend>(request).await
    }

    pub async fn get_file_bytes(&self, request: GetFileBytesRequest) -> Result<GetFileBytesResponse> {
        self.call.call::<storage::GetFileBytes>(request).await
    }

    pub async fn get_file_hash(&self, request: GetFileHashRequest) -> Result<GetFileHashResponse> {
        self.call.call::<storage::GetFileHash>(request).await
    }

    pub async fn get_file_production(&self, request: GetFileProductionRequest) -> Result<GetFileProductionResponse> {
        self.call.call::<storage::GetFileProduction>(request).await
    }

    pub async fn get_index_definition(&self, request: GetIndexDefinitionRequest) -> Result<GetIndexDefinitionResponse> {
        self.call.call::<storage::GetIndexDefinition>(request).await
    }

    pub async fn get_object(&self, request: GetObjectRequest) -> Result<GetObjectResponse> {
        self.call.call::<storage::GetObject>(request).await
    }

    pub async fn get_projection_definition(&self, request: GetProjectionDefinitionRequest) -> Result<GetProjectionDefinitionResponse> {
        self.call.call::<storage::GetProjectionDefinition>(request).await
    }

    pub async fn get_projection_status(&self, request: GetProjectionStatusRequest) -> Result<GetProjectionStatusResponse> {
        self.call.call::<storage::GetProjectionStatus>(request).await
    }

    pub async fn get_schema(&self, request: GetSchemaRequest) -> Result<GetSchemaResponse> {
        self.call.call::<storage::GetSchema>(request).await
    }

    pub async fn info(&self, request: InfoRequest) -> Result<InfoResponse> {
        self.call.call::<storage::Info>(request).await
    }

    pub async fn list_column_rows(&self, request: ListColumnRowsRequest) -> Result<ListColumnRowsResponse> {
        self.call.call::<storage::ListColumnRows>(request).await
    }

    pub async fn list_directory_entries(&self, request: ListDirectoryEntriesRequest) -> Result<ListDirectoryEntriesResponse> {
        self.call.call::<storage::ListDirectoryEntries>(request).await
    }

    pub async fn list_file_productions(&self, request: ListFileProductionsRequest) -> Result<ListFileProductionsResponse> {
        self.call.call::<storage::ListFileProductions>(request).await
    }

    pub async fn list_index_entries(&self, request: ListIndexEntriesRequest) -> Result<ListIndexEntriesResponse> {
        self.call.call::<storage::ListIndexEntries>(request).await
    }

    pub async fn list_projection_rows(&self, request: ListProjectionRowsRequest) -> Result<ListProjectionRowsResponse> {
        self.call.call::<storage::ListProjectionRows>(request).await
    }

    pub async fn list_rows(&self, request: ListRowsRequest) -> Result<ListRowsResponse> {
        self.call.call::<storage::ListRows>(request).await
    }

    pub async fn mutate(&self, request: MutateRequest) -> Result<MutateResponse> {
        self.call.call::<storage::Mutate>(request).await
    }

    pub async fn publish_file_production(&self, request: PublishFileProductionRequest) -> Result<PublishFileProductionResponse> {
        self.call.call::<storage::PublishFileProduction>(request).await
    }

    pub async fn set_index_definition(&self, request: SetIndexDefinitionRequest) -> Result<SetIndexDefinitionResponse> {
        self.call.call::<storage::SetIndexDefinition>(request).await
    }

    pub async fn set_object(&self, request: SetObjectRequest) -> Result<SetObjectResponse> {
        self.call.call::<storage::SetObject>(request).await
    }

    pub async fn set_projection_definition(&self, request: SetProjectionDefinitionRequest) -> Result<SetProjectionDefinitionResponse> {
        self.call.call::<storage::SetProjectionDefinition>(request).await
    }

    pub async fn set_schema(&self, request: SetSchemaRequest) -> Result<SetSchemaResponse> {
        self.call.call::<storage::SetSchema>(request).await
    }

    pub fn watch_directory_entries(
        &self, request: WatchDirectoryEntriesRequest, output: <storage::WatchDirectoryEntries as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<storage::WatchDirectoryEntries>> {
        self.call.submit::<storage::WatchDirectoryEntries>(request, output)
    }

    pub fn watch_file_bytes(
        &self, request: WatchFileBytesRequest, output: <storage::WatchFileBytes as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<storage::WatchFileBytes>> {
        self.call.submit::<storage::WatchFileBytes>(request, output)
    }

    pub fn watch_file_production(
        &self, request: WatchFileProductionRequest, output: <storage::WatchFileProduction as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<storage::WatchFileProduction>> {
        self.call.submit::<storage::WatchFileProduction>(request, output)
    }

    pub fn watch_rows(
        &self, request: WatchRowsRequest, output: <storage::WatchRows as NativeOperation>::Endpoints,
    ) -> Result<NativeInvocation<storage::WatchRows>> {
        self.call.submit::<storage::WatchRows>(request, output)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
