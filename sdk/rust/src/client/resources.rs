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
    pub async fn capabilities(&self, request: tonic::Request<harmos_protocol::storage::Capabilities>) -> Result<tonic::Response<harmos_protocol::storage::StorageCapabilities>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/Capabilities"), request, harmos_protocol::storage::StorageCapabilities::default().descriptor()).await
    }
    pub async fn open_snapshot(&self, request: tonic::Request<harmos_protocol::storage::OpenSnapshot>) -> Result<tonic::Response<harmos_protocol::storage::SavedSnapshot>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/OpenSnapshot"), request, harmos_protocol::storage::SavedSnapshot::default().descriptor()).await
    }
    pub async fn release_snapshot(&self, request: tonic::Request<harmos_protocol::storage::ReleaseSnapshot>) -> Result<tonic::Response<harmos_protocol::storage::Empty>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/ReleaseSnapshot"), request, harmos_protocol::storage::Empty::default().descriptor()).await
    }
    pub async fn scan_arrow(&self, request: tonic::Request<harmos_protocol::storage::ScanArrow>) -> Result<tonic::Response<harmos_protocol::storage::ArrowPage>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/ScanArrow"), request, harmos_protocol::storage::ArrowPage::default().descriptor()).await
    }
    pub async fn scan_index(&self, request: tonic::Request<harmos_protocol::storage::ScanIndex>) -> Result<tonic::Response<harmos_protocol::storage::IndexPage>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/ScanIndex"), request, harmos_protocol::storage::IndexPage::default().descriptor()).await
    }
    pub async fn register_index(&self, request: tonic::Request<harmos_protocol::storage::RegisterIndex>) -> Result<tonic::Response<harmos_protocol::storage::DefinitionRevision>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/RegisterIndex"), request, harmos_protocol::storage::DefinitionRevision::default().descriptor()).await
    }
    pub async fn register_projection(&self, request: tonic::Request<harmos_protocol::storage::RegisterProjection>) -> Result<tonic::Response<harmos_protocol::storage::DefinitionRevision>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/RegisterProjection"), request, harmos_protocol::storage::DefinitionRevision::default().descriptor()).await
    }
    pub async fn advance_projection(&self, request: tonic::Request<harmos_protocol::storage::AdvanceProjection>) -> Result<tonic::Response<harmos_protocol::storage::DefinitionRevision>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/AdvanceProjection"), request, harmos_protocol::storage::DefinitionRevision::default().descriptor()).await
    }
    pub async fn read_projection(&self, request: tonic::Request<harmos_protocol::storage::ReadProjection>) -> Result<tonic::Response<harmos_protocol::storage::ProjectionPage>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/ReadProjection"), request, harmos_protocol::storage::ProjectionPage::default().descriptor()).await
    }
    pub async fn inspect_projection(&self, request: tonic::Request<harmos_protocol::storage::InspectProjection>) -> Result<tonic::Response<harmos_protocol::storage::ProjectionState>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/InspectProjection"), request, harmos_protocol::storage::ProjectionState::default().descriptor()).await
    }
    pub async fn compare_exchange_object(&self, request: tonic::Request<harmos_protocol::storage::CompareExchangeObject>) -> Result<tonic::Response<harmos_protocol::storage::CompareExchangeOutcome>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/CompareExchangeObject"), request, harmos_protocol::storage::CompareExchangeOutcome::default().descriptor()).await
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
