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
