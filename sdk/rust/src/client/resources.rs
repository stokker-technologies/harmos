// @generated from the protocol descriptor table; do not edit.
use harmos::client::{ClientTransport, Watching};
use prost_reflect::ReflectMessage;
#[derive(Clone)]
pub struct Root {
    pub access: Access,
    pub catalog_entries: CatalogEntries,
    pub invocations: Invocations,
    pub plugins: Plugins,
    pub status: Status,
    pub storage: Storage,
    pub time: Time,
}
impl Root {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            access: Access::new(client.clone()),
            catalog_entries: CatalogEntries::new(client.clone()),
            invocations: Invocations::new(client.clone()),
            plugins: Plugins::new(client.clone()),
            status: Status::new(client.clone()),
            storage: Storage::new(client.clone()),
            time: Time::new(client.clone()),
        }
    }
}
#[derive(Clone)]
pub struct Access {
    pub certificates: AccessCertificates,
    pub listener: AccessListener,
    pub pairings: AccessPairings,
}
impl Access {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            certificates: AccessCertificates::new(client.clone()),
            listener: AccessListener::new(client.clone()),
            pairings: AccessPairings::new(client.clone()),
        }
    }
}
#[derive(Clone)]
pub struct AccessCertificates {
    client: ClientTransport,
}
impl AccessCertificates {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn create(&self, request: tonic::Request<harmos_protocol::v1::access::CertificatesCreateRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::CertificatesCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Certificates/Create"), request, harmos_protocol::v1::access::CertificatesCreateResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct AccessListener {
    client: ClientTransport,
}
impl AccessListener {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::access::ListenerGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::ListenerGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Listener/Get"), request, harmos_protocol::v1::access::ListenerGetResponse::default().descriptor()).await
    }
    pub async fn set(&self, request: tonic::Request<harmos_protocol::v1::access::ListenerSetRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::ListenerSetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Listener/Set"), request, harmos_protocol::v1::access::ListenerSetResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct AccessPairings {
    client: ClientTransport,
}
impl AccessPairings {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn create(&self, request: tonic::Request<harmos_protocol::v1::access::PairingsCreateRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::PairingsCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Pairings/Create"), request, harmos_protocol::v1::access::PairingsCreateResponse::default().descriptor()).await
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::v1::access::PairingsListRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::PairingsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Pairings/List"), request, harmos_protocol::v1::access::PairingsListResponse::default().descriptor()).await
    }
    pub async fn delete(&self, request: tonic::Request<harmos_protocol::v1::access::PairingsDeleteRequest>) -> Result<tonic::Response<harmos_protocol::v1::access::PairingsDeleteResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.access.Pairings/Delete"), request, harmos_protocol::v1::access::PairingsDeleteResponse::default().descriptor()).await
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
pub struct Plugins {
    client: ClientTransport,
}
impl Plugins {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::PluginsGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Get"), request, harmos_protocol::v1::PluginsGetResponse::default().descriptor()).await
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::v1::PluginsListRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/List"), request, harmos_protocol::v1::PluginsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::v1::PluginsWatchRequest>) -> Result<Watching<harmos_protocol::v1::PluginsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Watch"), request, harmos_protocol::v1::PluginsWatchResponse::default().descriptor()).await
    }
    pub async fn create(&self, request: tonic::Request<harmos_protocol::v1::PluginsCreateRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Create"), request, harmos_protocol::v1::PluginsCreateResponse::default().descriptor()).await
    }
    pub async fn set(&self, request: tonic::Request<harmos_protocol::v1::PluginsSetRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsSetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Set"), request, harmos_protocol::v1::PluginsSetResponse::default().descriptor()).await
    }
    pub async fn delete(&self, request: tonic::Request<harmos_protocol::v1::PluginsDeleteRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsDeleteResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Delete"), request, harmos_protocol::v1::PluginsDeleteResponse::default().descriptor()).await
    }
    pub async fn restart(&self, request: tonic::Request<harmos_protocol::v1::PluginsRestartRequest>) -> Result<tonic::Response<harmos_protocol::v1::PluginsRestartResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Plugins/Restart"), request, harmos_protocol::v1::PluginsRestartResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Status {
    client: ClientTransport,
    pub diagnostics: StatusDiagnostics,
    pub logs: StatusLogs,
    pub trace_events: StatusTraceEvents,
}
impl Status {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            diagnostics: StatusDiagnostics::new(client.clone()),
            logs: StatusLogs::new(client.clone()),
            trace_events: StatusTraceEvents::new(client.clone()),
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::StatusGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::StatusGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Status/Get"), request, harmos_protocol::v1::StatusGetResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::v1::StatusWatchRequest>) -> Result<Watching<harmos_protocol::v1::StatusWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Status/Watch"), request, harmos_protocol::v1::StatusWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct StatusDiagnostics {
    client: ClientTransport,
}
impl StatusDiagnostics {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::v1::status::DiagnosticsGetRequest>) -> Result<tonic::Response<harmos_protocol::v1::status::DiagnosticsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.status.Diagnostics/Get"), request, harmos_protocol::v1::status::DiagnosticsGetResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct StatusLogs {
    client: ClientTransport,
}
impl StatusLogs {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::v1::status::LogsListRequest>) -> Result<tonic::Response<harmos_protocol::v1::status::LogsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.status.Logs/List"), request, harmos_protocol::v1::status::LogsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::v1::status::LogsWatchRequest>) -> Result<Watching<harmos_protocol::v1::status::LogsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.status.Logs/Watch"), request, harmos_protocol::v1::status::LogsWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct StatusTraceEvents {
    client: ClientTransport,
}
impl StatusTraceEvents {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::v1::status::TraceEventsListRequest>) -> Result<tonic::Response<harmos_protocol::v1::status::TraceEventsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.status.TraceEvents/List"), request, harmos_protocol::v1::status::TraceEventsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::v1::status::TraceEventsWatchRequest>) -> Result<Watching<harmos_protocol::v1::status::TraceEventsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.status.TraceEvents/Watch"), request, harmos_protocol::v1::status::TraceEventsWatchResponse::default().descriptor()).await
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
