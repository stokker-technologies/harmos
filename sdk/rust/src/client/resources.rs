// @generated from the protocol descriptor table; do not edit.
use harmos::client::{ClientTransport, Watching};
use prost_reflect::ReflectMessage;
#[derive(Clone)]
pub struct Root {
    pub catalog_entries: CatalogEntries,
    pub invocations: Invocations,
    pub storage: Storage,
    pub time: Time,
}
impl Root {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            catalog_entries: CatalogEntries::new(client.clone()),
            invocations: Invocations::new(client.clone()),
            storage: Storage::new(client.clone()),
            time: Time::new(client.clone()),
        }
    }
}
#[derive(Clone)]
pub struct CatalogEntries {
    client: ClientTransport,
}
impl CatalogEntries {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::v1::CatalogEntriesListRequest>) -> Result<tonic::Response<harmos_protocol::v1::CatalogEntriesListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.CatalogEntries/List"), request, harmos_protocol::v1::CatalogEntriesListResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Invocations {
    client: ClientTransport,
}
impl Invocations {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn invoke(&self, request: tonic::Request<harmos_protocol::v1::InvocationsInvokeRequest>) -> Result<tonic::Response<harmos_protocol::v1::InvocationsInvokeResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Invocations/Invoke"), request, harmos_protocol::v1::InvocationsInvokeResponse::default().descriptor()).await
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::InvocationsGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::InvocationsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Invocations/Get"), request, harmos_protocol::v1::InvocationsGetResponse::default().descriptor()).await
    }
    pub async fn cancel(&self, request: tonic::Request<harmos_protocol::v1::InvocationsCancelRequest>) -> Result<tonic::Response<harmos_protocol::v1::InvocationsCancelResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Invocations/Cancel"), request, harmos_protocol::v1::InvocationsCancelResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::v1::InvocationsWatchRequest>) -> Result<Watching<harmos_protocol::v1::InvocationsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Invocations/Watch"), request, harmos_protocol::v1::InvocationsWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Storage {
    client: ClientTransport,
}
impl Storage {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn create_file_production(&self, request: tonic::Request<harmos_protocol::v1::StorageCreateFileProductionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageCreateFileProductionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/CreateFileProduction"), request, harmos_protocol::v1::StorageCreateFileProductionResponse::default().descriptor()).await
    }
    pub async fn get_file_production(&self, request: tonic::Request<harmos_protocol::v1::StorageGetFileProductionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetFileProductionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetFileProduction"), request, harmos_protocol::v1::StorageGetFileProductionResponse::default().descriptor()).await
    }
    pub async fn append_file_bytes(&self, request: tonic::Request<harmos_protocol::v1::StorageAppendFileBytesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageAppendFileBytesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/AppendFileBytes"), request, harmos_protocol::v1::StorageAppendFileBytesResponse::default().descriptor()).await
    }
    pub async fn create_file_writer(&self, request: tonic::Request<harmos_protocol::v1::StorageCreateFileWriterRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageCreateFileWriterResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/CreateFileWriter"), request, harmos_protocol::v1::StorageCreateFileWriterResponse::default().descriptor()).await
    }
    pub async fn publish_file_production(&self, request: tonic::Request<harmos_protocol::v1::StoragePublishFileProductionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StoragePublishFileProductionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/PublishFileProduction"), request, harmos_protocol::v1::StoragePublishFileProductionResponse::default().descriptor()).await
    }
    pub async fn delete_file_production(&self, request: tonic::Request<harmos_protocol::v1::StorageDeleteFileProductionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageDeleteFileProductionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/DeleteFileProduction"), request, harmos_protocol::v1::StorageDeleteFileProductionResponse::default().descriptor()).await
    }
    pub async fn get_file_bytes(&self, request: tonic::Request<harmos_protocol::v1::StorageGetFileBytesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetFileBytesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetFileBytes"), request, harmos_protocol::v1::StorageGetFileBytesResponse::default().descriptor()).await
    }
    pub async fn capabilities(&self, request: tonic::Request<harmos_protocol::v1::StorageCapabilitiesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageCapabilitiesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/Capabilities"), request, harmos_protocol::v1::StorageCapabilitiesResponse::default().descriptor()).await
    }
    pub async fn create_snapshot(&self, request: tonic::Request<harmos_protocol::v1::StorageCreateSnapshotRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageCreateSnapshotResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/CreateSnapshot"), request, harmos_protocol::v1::StorageCreateSnapshotResponse::default().descriptor()).await
    }
    pub async fn delete_snapshot(&self, request: tonic::Request<harmos_protocol::v1::StorageDeleteSnapshotRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageDeleteSnapshotResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/DeleteSnapshot"), request, harmos_protocol::v1::StorageDeleteSnapshotResponse::default().descriptor()).await
    }
    pub async fn list_column_rows(&self, request: tonic::Request<harmos_protocol::v1::StorageListColumnRowsRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageListColumnRowsResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/ListColumnRows"), request, harmos_protocol::v1::StorageListColumnRowsResponse::default().descriptor()).await
    }
    pub async fn list_index_entries(&self, request: tonic::Request<harmos_protocol::v1::StorageListIndexEntriesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageListIndexEntriesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/ListIndexEntries"), request, harmos_protocol::v1::StorageListIndexEntriesResponse::default().descriptor()).await
    }
    pub async fn set_index_definition(&self, request: tonic::Request<harmos_protocol::v1::StorageSetIndexDefinitionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageSetIndexDefinitionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/SetIndexDefinition"), request, harmos_protocol::v1::StorageSetIndexDefinitionResponse::default().descriptor()).await
    }
    pub async fn set_projection_definition(&self, request: tonic::Request<harmos_protocol::v1::StorageSetProjectionDefinitionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageSetProjectionDefinitionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/SetProjectionDefinition"), request, harmos_protocol::v1::StorageSetProjectionDefinitionResponse::default().descriptor()).await
    }
    pub async fn advance_projection(&self, request: tonic::Request<harmos_protocol::v1::StorageAdvanceProjectionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageAdvanceProjectionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/AdvanceProjection"), request, harmos_protocol::v1::StorageAdvanceProjectionResponse::default().descriptor()).await
    }
    pub async fn list_projection_rows(&self, request: tonic::Request<harmos_protocol::v1::StorageListProjectionRowsRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageListProjectionRowsResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/ListProjectionRows"), request, harmos_protocol::v1::StorageListProjectionRowsResponse::default().descriptor()).await
    }
    pub async fn get_projection_status(&self, request: tonic::Request<harmos_protocol::v1::StorageGetProjectionStatusRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetProjectionStatusResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetProjectionStatus"), request, harmos_protocol::v1::StorageGetProjectionStatusResponse::default().descriptor()).await
    }
    pub async fn set_object(&self, request: tonic::Request<harmos_protocol::v1::StorageSetObjectRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageSetObjectResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/SetObject"), request, harmos_protocol::v1::StorageSetObjectResponse::default().descriptor()).await
    }
    pub async fn list_rows(&self, request: tonic::Request<harmos_protocol::v1::StorageListRowsRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageListRowsResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/ListRows"), request, harmos_protocol::v1::StorageListRowsResponse::default().descriptor()).await
    }
    pub async fn watch_rows(&self, request: tonic::Request<harmos_protocol::v1::StorageWatchRowsRequest>) -> Result<Watching<harmos_protocol::v1::StorageWatchRowsResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchRows"), request, harmos_protocol::v1::StorageWatchRowsResponse::default().descriptor()).await
    }
    pub async fn append_records(&self, request: tonic::Request<harmos_protocol::v1::StorageAppendRecordsRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageAppendRecordsResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/AppendRecords"), request, harmos_protocol::v1::StorageAppendRecordsResponse::default().descriptor()).await
    }
    pub async fn append_samples(&self, request: tonic::Request<harmos_protocol::v1::StorageAppendSamplesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageAppendSamplesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/AppendSamples"), request, harmos_protocol::v1::StorageAppendSamplesResponse::default().descriptor()).await
    }
    pub async fn append_object_revisions(&self, request: tonic::Request<harmos_protocol::v1::StorageAppendObjectRevisionsRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageAppendObjectRevisionsResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/AppendObjectRevisions"), request, harmos_protocol::v1::StorageAppendObjectRevisionsResponse::default().descriptor()).await
    }
    pub async fn get_file_hash(&self, request: tonic::Request<harmos_protocol::v1::StorageGetFileHashRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetFileHashResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetFileHash"), request, harmos_protocol::v1::StorageGetFileHashResponse::default().descriptor()).await
    }
    pub async fn capture_file(&self, request: tonic::Request<harmos_protocol::v1::StorageCaptureFileRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageCaptureFileResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/CaptureFile"), request, harmos_protocol::v1::StorageCaptureFileResponse::default().descriptor()).await
    }
    pub async fn watch_file_bytes(&self, request: tonic::Request<harmos_protocol::v1::StorageWatchFileBytesRequest>) -> Result<Watching<harmos_protocol::v1::StorageWatchFileBytesResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchFileBytes"), request, harmos_protocol::v1::StorageWatchFileBytesResponse::default().descriptor()).await
    }
    pub async fn watch_file_production(&self, request: tonic::Request<harmos_protocol::v1::StorageWatchFileProductionRequest>) -> Result<Watching<harmos_protocol::v1::StorageWatchFileProductionResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchFileProduction"), request, harmos_protocol::v1::StorageWatchFileProductionResponse::default().descriptor()).await
    }
    pub async fn list_directory_entries(&self, request: tonic::Request<harmos_protocol::v1::StorageListDirectoryEntriesRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageListDirectoryEntriesResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/ListDirectoryEntries"), request, harmos_protocol::v1::StorageListDirectoryEntriesResponse::default().descriptor()).await
    }
    pub async fn watch_directory_entries(&self, request: tonic::Request<harmos_protocol::v1::StorageWatchDirectoryEntriesRequest>) -> Result<Watching<harmos_protocol::v1::StorageWatchDirectoryEntriesResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchDirectoryEntries"), request, harmos_protocol::v1::StorageWatchDirectoryEntriesResponse::default().descriptor()).await
    }
    pub async fn get_index_definition(&self, request: tonic::Request<harmos_protocol::v1::StorageGetIndexDefinitionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetIndexDefinitionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetIndexDefinition"), request, harmos_protocol::v1::StorageGetIndexDefinitionResponse::default().descriptor()).await
    }
    pub async fn get_projection_definition(&self, request: tonic::Request<harmos_protocol::v1::StorageGetProjectionDefinitionRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetProjectionDefinitionResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetProjectionDefinition"), request, harmos_protocol::v1::StorageGetProjectionDefinitionResponse::default().descriptor()).await
    }
    pub async fn get_object(&self, request: tonic::Request<harmos_protocol::v1::StorageGetObjectRequest>) -> Result<tonic::Response<harmos_protocol::v1::StorageGetObjectResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/GetObject"), request, harmos_protocol::v1::StorageGetObjectResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Time {
    client: ClientTransport,
}
impl Time {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::TimeGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::TimeGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Time/Get"), request, harmos_protocol::v1::TimeGetResponse::default().descriptor()).await
    }
    pub async fn sleep(&self, request: tonic::Request<harmos_protocol::v1::TimeSleepRequest>) -> Result<tonic::Response<harmos_protocol::v1::TimeSleepResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Time/Sleep"), request, harmos_protocol::v1::TimeSleepResponse::default().descriptor()).await
    }
}
