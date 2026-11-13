// @generated from the protocol descriptor table; do not edit.
use harmos::client::{ClientTransport, Watching};
use prost_reflect::ReflectMessage;
#[derive(Clone)]
pub struct Root {
    pub jobs: Jobs,
    pub plugins: Plugins,
}
impl Root {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            jobs: Jobs::new(client.clone()),
            plugins: Plugins::new(client.clone()),
        }
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
pub struct Plugins {
    client: ClientTransport,
    pub settings: PluginsSettings,
}
impl Plugins {
    pub fn new(client: ClientTransport) -> Self {
        Self {
            settings: PluginsSettings::new(client.clone()),
            client,
        }
    }
    pub async fn get(&self, request: tonic::Request<library_protocol::v1::PluginsGetRequest>) -> Result<tonic::Response<library_protocol::v1::PluginsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/library.v1.Plugins/Get"), request, library_protocol::v1::PluginsGetResponse::default().descriptor()).await
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
    pub async fn get(&self, request: tonic::Request<library_protocol::v1::plugins::SettingsGetRequest>) -> Result<tonic::Response<library_protocol::v1::plugins::SettingsGetResponse>, tonic::Status> {
        self.client.unary(tonic::codegen::http::uri::PathAndQuery::from_static("/library.v1.plugins.Settings/Get"), request, library_protocol::v1::plugins::SettingsGetResponse::default().descriptor()).await
    }
}
