// @generated from the protocol descriptor table; do not edit.
use harmos::client::{ClientTransport, Watching};
use prost_reflect::ReflectMessage;
#[derive(Clone)]
pub struct Root {
    pub access: Access,
    pub invocations: Invocations,
    pub jobs: Jobs,
    pub operations: Operations,
    pub plugins: Plugins,
    pub schemas: Schemas,
    pub status: Status,
    pub streams: Streams,
    pub time: Time,
}
impl Root {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            access: Access::new(client.clone()),
            invocations: Invocations::new(client.clone()),
            jobs: Jobs::new(client.clone()),
            operations: Operations::new(client.clone()),
            plugins: Plugins::new(client.clone()),
            schemas: Schemas::new(client.clone()),
            status: Status::new(client.clone()),
            streams: Streams::new(client.clone()),
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
    pub async fn create(&self, request: tonic::Request<harmos_protocol::client::access::CertificatesCreateRequest>) -> Result<tonic::Response<harmos_protocol::client::access::CertificatesCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Certificates/Create"), request, harmos_protocol::client::access::CertificatesCreateResponse::default().descriptor()).await
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
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::access::ListenerGetRequest>) -> Result<tonic::Response<harmos_protocol::client::access::ListenerGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Listener/Get"), request, harmos_protocol::client::access::ListenerGetResponse::default().descriptor()).await
    }
    pub async fn set(&self, request: tonic::Request<harmos_protocol::client::access::ListenerSetRequest>) -> Result<tonic::Response<harmos_protocol::client::access::ListenerSetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Listener/Set"), request, harmos_protocol::client::access::ListenerSetResponse::default().descriptor()).await
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
    pub async fn create(&self, request: tonic::Request<harmos_protocol::client::access::PairingsCreateRequest>) -> Result<tonic::Response<harmos_protocol::client::access::PairingsCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Pairings/Create"), request, harmos_protocol::client::access::PairingsCreateResponse::default().descriptor()).await
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::access::PairingsListRequest>) -> Result<tonic::Response<harmos_protocol::client::access::PairingsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Pairings/List"), request, harmos_protocol::client::access::PairingsListResponse::default().descriptor()).await
    }
    pub async fn delete(&self, request: tonic::Request<harmos_protocol::client::access::PairingsDeleteRequest>) -> Result<tonic::Response<harmos_protocol::client::access::PairingsDeleteResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.access.Pairings/Delete"), request, harmos_protocol::client::access::PairingsDeleteResponse::default().descriptor()).await
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
    pub async fn invoke(&self, request: tonic::Request<harmos_protocol::client::InvocationsInvokeRequest>) -> Result<tonic::Response<harmos_protocol::client::InvocationsInvokeResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Invocations/Invoke"), request, harmos_protocol::client::InvocationsInvokeResponse::default().descriptor()).await
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::InvocationsGetRequest>) -> Result<tonic::Response<harmos_protocol::client::InvocationsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Invocations/Get"), request, harmos_protocol::client::InvocationsGetResponse::default().descriptor()).await
    }
    pub async fn cancel(&self, request: tonic::Request<harmos_protocol::client::InvocationsCancelRequest>) -> Result<tonic::Response<harmos_protocol::client::InvocationsCancelResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Invocations/Cancel"), request, harmos_protocol::client::InvocationsCancelResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::client::InvocationsWatchRequest>) -> Result<Watching<harmos_protocol::client::InvocationsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Invocations/Watch"), request, harmos_protocol::client::InvocationsWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Jobs {
    client: ClientTransport,
    pub progress: JobsProgress,
}
impl Jobs {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            progress: JobsProgress::new(client.clone()),
            client,
        }
    }
    pub async fn start(&self, request: tonic::Request<application_protocol::v1::JobsStartRequest>) -> Result<tonic::Response<application_protocol::v1::JobsStartResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/application.v1.Jobs/Start"), request, application_protocol::v1::JobsStartResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct JobsProgress {
    client: ClientTransport,
}
impl JobsProgress {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn watch(&self, request: tonic::Request<application_protocol::v1::jobs::ProgressWatchRequest>) -> Result<Watching<application_protocol::v1::jobs::ProgressWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/application.v1.jobs.Progress/Watch"), request, application_protocol::v1::jobs::ProgressWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Operations {
    client: ClientTransport,
}
impl Operations {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::OperationsListRequest>) -> Result<tonic::Response<harmos_protocol::client::OperationsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Operations/List"), request, harmos_protocol::client::OperationsListResponse::default().descriptor()).await
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::OperationsGetRequest>) -> Result<tonic::Response<harmos_protocol::client::OperationsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Operations/Get"), request, harmos_protocol::client::OperationsGetResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Plugins {
    client: ClientTransport,
    pub annotations: PluginsAnnotations,
    pub settings: PluginsSettings,
}
impl Plugins {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            annotations: PluginsAnnotations::new(client.clone()),
            settings: PluginsSettings::new(client.clone()),
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::PluginsGetRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Get"), request, harmos_protocol::client::PluginsGetResponse::default().descriptor()).await
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::PluginsListRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/List"), request, harmos_protocol::client::PluginsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::client::PluginsWatchRequest>) -> Result<Watching<harmos_protocol::client::PluginsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Watch"), request, harmos_protocol::client::PluginsWatchResponse::default().descriptor()).await
    }
    pub async fn create(&self, request: tonic::Request<harmos_protocol::client::PluginsCreateRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsCreateResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Create"), request, harmos_protocol::client::PluginsCreateResponse::default().descriptor()).await
    }
    pub async fn set(&self, request: tonic::Request<harmos_protocol::client::PluginsSetRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsSetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Set"), request, harmos_protocol::client::PluginsSetResponse::default().descriptor()).await
    }
    pub async fn delete(&self, request: tonic::Request<harmos_protocol::client::PluginsDeleteRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsDeleteResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Delete"), request, harmos_protocol::client::PluginsDeleteResponse::default().descriptor()).await
    }
    pub async fn restart(&self, request: tonic::Request<harmos_protocol::client::PluginsRestartRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsRestartResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Restart"), request, harmos_protocol::client::PluginsRestartResponse::default().descriptor()).await
    }
    pub async fn replace(&self, request: tonic::Request<harmos_protocol::client::PluginsReplaceRequest>) -> Result<tonic::Response<harmos_protocol::client::PluginsReplaceResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Plugins/Replace"), request, harmos_protocol::client::PluginsReplaceResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct PluginsAnnotations {
    client: ClientTransport,
}
impl PluginsAnnotations {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<application_protocol::v1::plugins::AnnotationsGetRequest>) -> Result<tonic::Response<application_protocol::v1::plugins::AnnotationsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/application.v1.plugins.Annotations/Get"), request, application_protocol::v1::plugins::AnnotationsGetResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct PluginsSettings {
    client: ClientTransport,
}
impl PluginsSettings {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::plugins::SettingsListRequest>) -> Result<tonic::Response<harmos_protocol::client::plugins::SettingsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.plugins.Settings/List"), request, harmos_protocol::client::plugins::SettingsListResponse::default().descriptor()).await
    }
    pub async fn set(&self, request: tonic::Request<harmos_protocol::client::plugins::SettingsSetRequest>) -> Result<tonic::Response<harmos_protocol::client::plugins::SettingsSetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.plugins.Settings/Set"), request, harmos_protocol::client::plugins::SettingsSetResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Schemas {
    client: ClientTransport,
}
impl Schemas {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::SchemasListRequest>) -> Result<tonic::Response<harmos_protocol::client::SchemasListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Schemas/List"), request, harmos_protocol::client::SchemasListResponse::default().descriptor()).await
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
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::StatusGetRequest>) -> Result<tonic::Response<harmos_protocol::client::StatusGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Status/Get"), request, harmos_protocol::client::StatusGetResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::client::StatusWatchRequest>) -> Result<Watching<harmos_protocol::client::StatusWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Status/Watch"), request, harmos_protocol::client::StatusWatchResponse::default().descriptor()).await
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
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::status::DiagnosticsGetRequest>) -> Result<tonic::Response<harmos_protocol::client::status::DiagnosticsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.status.Diagnostics/Get"), request, harmos_protocol::client::status::DiagnosticsGetResponse::default().descriptor()).await
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
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::status::LogsListRequest>) -> Result<tonic::Response<harmos_protocol::client::status::LogsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.status.Logs/List"), request, harmos_protocol::client::status::LogsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::client::status::LogsWatchRequest>) -> Result<Watching<harmos_protocol::client::status::LogsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.status.Logs/Watch"), request, harmos_protocol::client::status::LogsWatchResponse::default().descriptor()).await
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
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::status::TraceEventsListRequest>) -> Result<tonic::Response<harmos_protocol::client::status::TraceEventsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.status.TraceEvents/List"), request, harmos_protocol::client::status::TraceEventsListResponse::default().descriptor()).await
    }
    pub async fn watch(&self, request: tonic::Request<harmos_protocol::client::status::TraceEventsWatchRequest>) -> Result<Watching<harmos_protocol::client::status::TraceEventsWatchResponse>, tonic::Status> {
        self.client.watch_known(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.status.TraceEvents/Watch"), request, harmos_protocol::client::status::TraceEventsWatchResponse::default().descriptor()).await
    }
}
#[derive(Clone)]
pub struct Streams {
    client: ClientTransport,
}
impl Streams {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            client,
        }
    }
    pub async fn list(&self, request: tonic::Request<harmos_protocol::client::StreamsListRequest>) -> Result<tonic::Response<harmos_protocol::client::StreamsListResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Streams/List"), request, harmos_protocol::client::StreamsListResponse::default().descriptor()).await
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
    pub async fn get(&self, request: tonic::Request<harmos_protocol::client::TimeGetRequest>) -> Result<tonic::Response<harmos_protocol::client::TimeGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Time/Get"), request, harmos_protocol::client::TimeGetResponse::default().descriptor()).await
    }
    pub async fn sleep(&self, request: tonic::Request<harmos_protocol::client::TimeSleepRequest>) -> Result<tonic::Response<harmos_protocol::client::TimeSleepResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.client.v1.Time/Sleep"), request, harmos_protocol::client::TimeSleepResponse::default().descriptor()).await
    }
}
