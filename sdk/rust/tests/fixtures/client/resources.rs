// @generated from protocol descriptors; do not edit.
use application_protocol::v1::JobsStartRequest;
use application_protocol::v1::JobsStartResponse;
use application_protocol::v1::jobs::ProgressWatchRequest;
use application_protocol::v1::jobs::ProgressWatchResponse;
use application_protocol::v1::plugins::AnnotationsGetRequest;
use application_protocol::v1::plugins::AnnotationsGetResponse;
use harmos::client::{CallOptions, ClientTransport, Watching, action_error, request_id};
use harmos_protocol::client::InvocationsCancelRequest;
use harmos_protocol::client::InvocationsCancelResponse;
use harmos_protocol::client::InvocationsGetRequest;
use harmos_protocol::client::InvocationsGetResponse;
use harmos_protocol::client::InvocationsInvokeRequest;
use harmos_protocol::client::InvocationsInvokeResponse;
use harmos_protocol::client::InvocationsWatchRequest;
use harmos_protocol::client::InvocationsWatchResponse;
use harmos_protocol::client::OperationsGetRequest;
use harmos_protocol::client::OperationsGetResponse;
use harmos_protocol::client::OperationsListRequest;
use harmos_protocol::client::OperationsListResponse;
use harmos_protocol::client::PluginEntry;
use harmos_protocol::client::PluginPlacement;
use harmos_protocol::client::PluginValue;
use harmos_protocol::client::PluginsCreateRequest;
use harmos_protocol::client::PluginsCreateResponse;
use harmos_protocol::client::PluginsDeleteRequest;
use harmos_protocol::client::PluginsDeleteResponse;
use harmos_protocol::client::PluginsGetRequest;
use harmos_protocol::client::PluginsGetResponse;
use harmos_protocol::client::PluginsListRequest;
use harmos_protocol::client::PluginsListResponse;
use harmos_protocol::client::PluginsReplaceRequest;
use harmos_protocol::client::PluginsReplaceResponse;
use harmos_protocol::client::PluginsRestartRequest;
use harmos_protocol::client::PluginsRestartResponse;
use harmos_protocol::client::PluginsSetRequest;
use harmos_protocol::client::PluginsSetResponse;
use harmos_protocol::client::PluginsWatchRequest;
use harmos_protocol::client::PluginsWatchResponse;
use harmos_protocol::client::SchemaDescription;
use harmos_protocol::client::SchemasListRequest;
use harmos_protocol::client::SchemasListResponse;
use harmos_protocol::client::StatusGetRequest;
use harmos_protocol::client::StatusGetResponse;
use harmos_protocol::client::StatusWatchRequest;
use harmos_protocol::client::StatusWatchResponse;
use harmos_protocol::client::StreamIdentity;
use harmos_protocol::client::StreamsListRequest;
use harmos_protocol::client::StreamsListResponse;
use harmos_protocol::client::TimeGetRequest;
use harmos_protocol::client::TimeGetResponse;
use harmos_protocol::client::TimeSleepRequest;
use harmos_protocol::client::TimeSleepResponse;
use harmos_protocol::client::access::CertificatesCreateRequest;
use harmos_protocol::client::access::CertificatesCreateResponse;
use harmos_protocol::client::access::ListenerGetRequest;
use harmos_protocol::client::access::ListenerGetResponse;
use harmos_protocol::client::access::ListenerSetRequest;
use harmos_protocol::client::access::ListenerSetResponse;
use harmos_protocol::client::access::ListenerValue;
use harmos_protocol::client::access::PairingsCreateRequest;
use harmos_protocol::client::access::PairingsCreateResponse;
use harmos_protocol::client::access::PairingsDeleteRequest;
use harmos_protocol::client::access::PairingsDeleteResponse;
use harmos_protocol::client::access::PairingsListRequest;
use harmos_protocol::client::access::PairingsListResponse;
use harmos_protocol::client::plugins::SettingsListRequest;
use harmos_protocol::client::plugins::SettingsListResponse;
use harmos_protocol::client::plugins::SettingsSetRequest;
use harmos_protocol::client::plugins::SettingsSetResponse;
use harmos_protocol::client::status::DiagnosticsGetRequest;
use harmos_protocol::client::status::DiagnosticsGetResponse;
use harmos_protocol::client::status::LogsListRequest;
use harmos_protocol::client::status::LogsListResponse;
use harmos_protocol::client::status::LogsWatchRequest;
use harmos_protocol::client::status::LogsWatchResponse;
use harmos_protocol::client::status::TraceEventsListRequest;
use harmos_protocol::client::status::TraceEventsListResponse;
use harmos_protocol::client::status::TraceEventsWatchRequest;
use harmos_protocol::client::status::TraceEventsWatchResponse;
use harmos_protocol::v1::DiscoveredOperation;
use harmos_protocol::v1::InstanceState;
use harmos_protocol::v1::PageRequest;
use harmos_protocol::v1::Position;
use harmos_protocol::v1::ReadCut;
use harmos_protocol::v1::Receipt;
use harmos_protocol::v1::SettingValues;
use prost_reflect::ReflectMessage;
use prost_types::Duration;
use prost_types::Timestamp;
use tonic::Status as RpcStatus;
use tonic::codegen::http::uri::PathAndQuery;
#[derive(Default)]
pub struct AccessCertificatesCreateOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct AccessListenerGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct AccessListenerSetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct AccessPairingsCreateOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct AccessPairingsListOptions {
    pub at: Option<Position>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct AccessPairingsDeleteOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct InvocationsInvokeOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct InvocationsGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct InvocationsCancelOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct InvocationsWatchOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct OperationsListOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct OperationsGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsSettingsListOptions {
    pub at: Option<ReadCut>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsSettingsSetOptions {
    pub request_id: Option<String>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsGetOptions {
    pub at: Option<ReadCut>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsListOptions {
    pub at: Option<ReadCut>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsWatchOptions {
    pub name: Option<String>,
    pub at: Option<ReadCut>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsCreateOptions {
    pub request_id: Option<String>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsSetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsDeleteOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsRestartOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsReplaceOptions {
    pub request_id: Option<String>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct SchemasListOptions {
    pub schema: Option<String>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusDiagnosticsGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusLogsListOptions {
    pub at: Option<Position>,
    pub page: Option<PageRequest>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusLogsWatchOptions {
    pub at: Option<Position>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusTraceEventsListOptions {
    pub at: Option<Position>,
    pub page: Option<PageRequest>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusTraceEventsWatchOptions {
    pub at: Option<Position>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StatusWatchOptions {
    pub at: Option<Position>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct StreamsListOptions {
    pub schema: Option<String>,
    pub context: CallOptions,
}
#[derive(Default)]
pub struct TimeGetOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct TimeSleepOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct JobsStartOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct JobsProgressWatchOptions {
    pub context: CallOptions,
}
#[derive(Default)]
pub struct PluginsAnnotationsGetOptions {
    pub context: CallOptions,
}
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
        Self { client }
    }
    pub async fn create(&self) -> Result<String, RpcStatus> {
        self.create_with(AccessCertificatesCreateOptions::default()).await
    }
    pub async fn create_with(&self, options: AccessCertificatesCreateOptions) -> Result<String, RpcStatus> {
        let value = CertificatesCreateRequest {};
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<CertificatesCreateRequest, CertificatesCreateResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Certificates/Create"),
                request,
                CertificatesCreateResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.certificates.create", error))?;
        let response = response.into_inner();
        Ok(response.certificate_pem)
    }
}
#[derive(Clone)]
pub struct AccessListener {
    client: ClientTransport,
}
impl AccessListener {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn get(&self) -> Result<ListenerGetResponse, RpcStatus> {
        self.get_with(AccessListenerGetOptions::default()).await
    }
    pub async fn get_with(&self, options: AccessListenerGetOptions) -> Result<ListenerGetResponse, RpcStatus> {
        let value = ListenerGetRequest {};
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<ListenerGetRequest, ListenerGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Listener/Get"),
                request,
                ListenerGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.listener.get", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn set(&self, value: ListenerValue, expected_revision: Vec<u8>) -> Result<ListenerSetResponse, RpcStatus> {
        self.set_with(value, expected_revision, AccessListenerSetOptions::default()).await
    }
    pub async fn set_with(
        &self, value: ListenerValue, expected_revision: Vec<u8>, options: AccessListenerSetOptions,
    ) -> Result<ListenerSetResponse, RpcStatus> {
        let value = ListenerSetRequest {
            value: Some(value),
            expected_revision: expected_revision,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<ListenerSetRequest, ListenerSetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Listener/Set"),
                request,
                ListenerSetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.listener.set", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
#[derive(Clone)]
pub struct AccessPairings {
    client: ClientTransport,
}
impl AccessPairings {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn create(&self, label: impl Into<String>) -> Result<PairingsCreateResponse, RpcStatus> {
        self.create_with(label, AccessPairingsCreateOptions::default()).await
    }
    pub async fn create_with(
        &self, label: impl Into<String>, options: AccessPairingsCreateOptions,
    ) -> Result<PairingsCreateResponse, RpcStatus> {
        let value = PairingsCreateRequest { label: label.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PairingsCreateRequest, PairingsCreateResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Pairings/Create"),
                request,
                PairingsCreateResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.pairings.create", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn list(&self) -> Result<PairingsListResponse, RpcStatus> {
        self.list_with(AccessPairingsListOptions::default()).await
    }
    pub async fn list_with(&self, options: AccessPairingsListOptions) -> Result<PairingsListResponse, RpcStatus> {
        let value = PairingsListRequest { at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PairingsListRequest, PairingsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Pairings/List"),
                request,
                PairingsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.pairings.list", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn delete(&self, name: impl Into<String>) -> Result<PairingsDeleteResponse, RpcStatus> {
        self.delete_with(name, AccessPairingsDeleteOptions::default()).await
    }
    pub async fn delete_with(
        &self, name: impl Into<String>, options: AccessPairingsDeleteOptions,
    ) -> Result<PairingsDeleteResponse, RpcStatus> {
        let value = PairingsDeleteRequest { name: name.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PairingsDeleteRequest, PairingsDeleteResponse>(
                PathAndQuery::from_static("/harmos.client.v1.access.Pairings/Delete"),
                request,
                PairingsDeleteResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("access.pairings.delete", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
#[derive(Clone)]
pub struct Invocations {
    client: ClientTransport,
}
impl Invocations {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn invoke(&self, operation: impl Into<String>, fingerprint: Vec<u8>, payload: Vec<u8>) -> Result<Vec<u8>, RpcStatus> {
        self.invoke_with(operation, fingerprint, payload, InvocationsInvokeOptions::default()).await
    }
    pub async fn invoke_with(
        &self, operation: impl Into<String>, fingerprint: Vec<u8>, payload: Vec<u8>, options: InvocationsInvokeOptions,
    ) -> Result<Vec<u8>, RpcStatus> {
        let value = InvocationsInvokeRequest {
            operation: operation.into(),
            fingerprint: fingerprint,
            payload: payload,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<InvocationsInvokeRequest, InvocationsInvokeResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Invocations/Invoke"),
                request,
                InvocationsInvokeResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("invocations.invoke", error))?;
        let response = response.into_inner();
        Ok(response.payload)
    }
    pub async fn get(&self, instance: u64) -> Result<InstanceState, RpcStatus> {
        self.get_with(instance, InvocationsGetOptions::default()).await
    }
    pub async fn get_with(&self, instance: u64, options: InvocationsGetOptions) -> Result<InstanceState, RpcStatus> {
        let value = InvocationsGetRequest { instance: instance };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<InvocationsGetRequest, InvocationsGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Invocations/Get"),
                request,
                InvocationsGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("invocations.get", error))?;
        let response = response.into_inner();
        response.invocation.ok_or_else(|| RpcStatus::data_loss("Read Invocations.Get: Missing Payload"))
    }
    pub async fn cancel(&self, instance: u64, reason: impl Into<String>) -> Result<InstanceState, RpcStatus> {
        self.cancel_with(instance, reason, InvocationsCancelOptions::default()).await
    }
    pub async fn cancel_with(
        &self, instance: u64, reason: impl Into<String>, options: InvocationsCancelOptions,
    ) -> Result<InstanceState, RpcStatus> {
        let value = InvocationsCancelRequest {
            instance: instance,
            reason: reason.into(),
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<InvocationsCancelRequest, InvocationsCancelResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Invocations/Cancel"),
                request,
                InvocationsCancelResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("invocations.cancel", error))?;
        let response = response.into_inner();
        response.invocation.ok_or_else(|| RpcStatus::data_loss("Read Invocations.Cancel: Missing Payload"))
    }
    pub async fn watch(&self, instance: u64) -> Result<Watching<InstanceState>, RpcStatus> {
        self.watch_with(instance, InvocationsWatchOptions::default()).await
    }
    pub async fn watch_with(&self, instance: u64, options: InvocationsWatchOptions) -> Result<Watching<InstanceState>, RpcStatus> {
        let value = InvocationsWatchRequest { instance: instance };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<InvocationsWatchRequest, InvocationsWatchResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Invocations/Watch"),
                request,
                InvocationsWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("invocations.watch", error))?;
        let response = response.with_action("invocations.watch");
        Ok(response.map(|response| response.invocation.ok_or_else(|| RpcStatus::data_loss("Read Invocations.Watch: Missing Payload"))))
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
    pub async fn start(&self, value: impl Into<String>) -> Result<String, RpcStatus> {
        self.start_with(value, JobsStartOptions::default()).await
    }
    pub async fn start_with(&self, value: impl Into<String>, options: JobsStartOptions) -> Result<String, RpcStatus> {
        let value = JobsStartRequest { value: value.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<JobsStartRequest, JobsStartResponse>(
                PathAndQuery::from_static("/application.v1.Jobs/Start"),
                request,
                JobsStartResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("jobs.start", error))?;
        let response = response.into_inner();
        Ok(response.value)
    }
}
#[derive(Clone)]
pub struct JobsProgress {
    client: ClientTransport,
}
impl JobsProgress {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn watch(&self, value: impl Into<String>) -> Result<Watching<String>, RpcStatus> {
        self.watch_with(value, JobsProgressWatchOptions::default()).await
    }
    pub async fn watch_with(&self, value: impl Into<String>, options: JobsProgressWatchOptions) -> Result<Watching<String>, RpcStatus> {
        let value = ProgressWatchRequest { value: value.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<ProgressWatchRequest, ProgressWatchResponse>(
                PathAndQuery::from_static("/application.v1.jobs.Progress/Watch"),
                request,
                ProgressWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("jobs.progress.watch", error))?;
        let response = response.with_action("jobs.progress.watch");
        Ok(response.map(|response| Ok(response.value)))
    }
}
#[derive(Clone)]
pub struct Operations {
    client: ClientTransport,
}
impl Operations {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self) -> Result<Vec<DiscoveredOperation>, RpcStatus> {
        self.list_with(OperationsListOptions::default()).await
    }
    pub async fn list_with(&self, options: OperationsListOptions) -> Result<Vec<DiscoveredOperation>, RpcStatus> {
        let value = OperationsListRequest {};
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<OperationsListRequest, OperationsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Operations/List"),
                request,
                OperationsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("operations.list", error))?;
        let response = response.into_inner();
        Ok(response.operations)
    }
    pub async fn get(&self, id: impl Into<String>) -> Result<DiscoveredOperation, RpcStatus> {
        self.get_with(id, OperationsGetOptions::default()).await
    }
    pub async fn get_with(&self, id: impl Into<String>, options: OperationsGetOptions) -> Result<DiscoveredOperation, RpcStatus> {
        let value = OperationsGetRequest { id: id.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<OperationsGetRequest, OperationsGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Operations/Get"),
                request,
                OperationsGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("operations.get", error))?;
        let response = response.into_inner();
        response.operation.ok_or_else(|| RpcStatus::data_loss("Read Operations.Get: Missing Payload"))
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
    pub async fn get(&self, name: impl Into<String>) -> Result<PluginsGetResponse, RpcStatus> {
        self.get_with(name, PluginsGetOptions::default()).await
    }
    pub async fn get_with(&self, name: impl Into<String>, options: PluginsGetOptions) -> Result<PluginsGetResponse, RpcStatus> {
        let value = PluginsGetRequest { name: name.into(), at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsGetRequest, PluginsGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Get"),
                request,
                PluginsGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.get", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn list(&self) -> Result<PluginsListResponse, RpcStatus> {
        self.list_with(PluginsListOptions::default()).await
    }
    pub async fn list_with(&self, options: PluginsListOptions) -> Result<PluginsListResponse, RpcStatus> {
        let value = PluginsListRequest { at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsListRequest, PluginsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/List"),
                request,
                PluginsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.list", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn watch(&self) -> Result<Watching<PluginsWatchResponse>, RpcStatus> {
        self.watch_with(PluginsWatchOptions::default()).await
    }
    pub async fn watch_with(&self, options: PluginsWatchOptions) -> Result<Watching<PluginsWatchResponse>, RpcStatus> {
        let value = PluginsWatchRequest { name: options.name, at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<PluginsWatchRequest, PluginsWatchResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Watch"),
                request,
                PluginsWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.watch", error))?;
        let response = response.with_action("plugins.watch");
        Ok(response)
    }
    pub async fn create(
        &self, name: impl Into<String>, placement: PluginPlacement, value: PluginValue,
    ) -> Result<PluginsCreateResponse, RpcStatus> {
        self.create_with(name, placement, value, PluginsCreateOptions::default()).await
    }
    pub async fn create_with(
        &self, name: impl Into<String>, placement: PluginPlacement, value: PluginValue, options: PluginsCreateOptions,
    ) -> Result<PluginsCreateResponse, RpcStatus> {
        let value = PluginsCreateRequest {
            request_id: match options.request_id {
                Some(value) => value,
                None => request_id()?,
            },
            name: name.into(),
            placement: Some(placement),
            value: Some(value),
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsCreateRequest, PluginsCreateResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Create"),
                request,
                PluginsCreateResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.create", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn set(
        &self, name: impl Into<String>, value: PluginValue, expected_revision: Vec<u8>,
    ) -> Result<PluginsSetResponse, RpcStatus> {
        self.set_with(name, value, expected_revision, PluginsSetOptions::default()).await
    }
    pub async fn set_with(
        &self, name: impl Into<String>, value: PluginValue, expected_revision: Vec<u8>, options: PluginsSetOptions,
    ) -> Result<PluginsSetResponse, RpcStatus> {
        let value = PluginsSetRequest {
            name: name.into(),
            value: Some(value),
            expected_revision: expected_revision,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsSetRequest, PluginsSetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Set"),
                request,
                PluginsSetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.set", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn delete(&self, name: impl Into<String>, expected_revision: Vec<u8>) -> Result<Receipt, RpcStatus> {
        self.delete_with(name, expected_revision, PluginsDeleteOptions::default()).await
    }
    pub async fn delete_with(
        &self, name: impl Into<String>, expected_revision: Vec<u8>, options: PluginsDeleteOptions,
    ) -> Result<Receipt, RpcStatus> {
        let value = PluginsDeleteRequest {
            name: name.into(),
            expected_revision: expected_revision,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsDeleteRequest, PluginsDeleteResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Delete"),
                request,
                PluginsDeleteResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.delete", error))?;
        let response = response.into_inner();
        response.receipt.ok_or_else(|| RpcStatus::data_loss("Read Plugins.Delete: Missing Payload"))
    }
    pub async fn restart(&self, name: impl Into<String>, expected_revision: Vec<u8>) -> Result<PluginEntry, RpcStatus> {
        self.restart_with(name, expected_revision, PluginsRestartOptions::default()).await
    }
    pub async fn restart_with(
        &self, name: impl Into<String>, expected_revision: Vec<u8>, options: PluginsRestartOptions,
    ) -> Result<PluginEntry, RpcStatus> {
        let value = PluginsRestartRequest {
            name: name.into(),
            expected_revision: expected_revision,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsRestartRequest, PluginsRestartResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Restart"),
                request,
                PluginsRestartResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.restart", error))?;
        let response = response.into_inner();
        response.entry.ok_or_else(|| RpcStatus::data_loss("Read Plugins.Restart: Missing Payload"))
    }
    pub async fn replace(
        &self, name: impl Into<String>, placement: PluginPlacement, expected_revision: Vec<u8>,
    ) -> Result<PluginsReplaceResponse, RpcStatus> {
        self.replace_with(name, placement, expected_revision, PluginsReplaceOptions::default()).await
    }
    pub async fn replace_with(
        &self, name: impl Into<String>, placement: PluginPlacement, expected_revision: Vec<u8>, options: PluginsReplaceOptions,
    ) -> Result<PluginsReplaceResponse, RpcStatus> {
        let value = PluginsReplaceRequest {
            name: name.into(),
            placement: Some(placement),
            expected_revision: expected_revision,
            request_id: match options.request_id {
                Some(value) => value,
                None => request_id()?,
            },
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<PluginsReplaceRequest, PluginsReplaceResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Plugins/Replace"),
                request,
                PluginsReplaceResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.replace", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
#[derive(Clone)]
pub struct PluginsAnnotations {
    client: ClientTransport,
}
impl PluginsAnnotations {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn get(&self, name: impl Into<String>) -> Result<String, RpcStatus> {
        self.get_with(name, PluginsAnnotationsGetOptions::default()).await
    }
    pub async fn get_with(&self, name: impl Into<String>, options: PluginsAnnotationsGetOptions) -> Result<String, RpcStatus> {
        let value = AnnotationsGetRequest { name: name.into() };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<AnnotationsGetRequest, AnnotationsGetResponse>(
                PathAndQuery::from_static("/application.v1.plugins.Annotations/Get"),
                request,
                AnnotationsGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.annotations.get", error))?;
        let response = response.into_inner();
        Ok(response.value)
    }
}
#[derive(Clone)]
pub struct PluginsSettings {
    client: ClientTransport,
}
impl PluginsSettings {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self, name: impl Into<String>) -> Result<SettingsListResponse, RpcStatus> {
        self.list_with(name, PluginsSettingsListOptions::default()).await
    }
    pub async fn list_with(&self, name: impl Into<String>, options: PluginsSettingsListOptions) -> Result<SettingsListResponse, RpcStatus> {
        let value = SettingsListRequest { name: name.into(), at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<SettingsListRequest, SettingsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.plugins.Settings/List"),
                request,
                SettingsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.settings.list", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn set(
        &self, name: impl Into<String>, value: SettingValues, expected_revision: Vec<u8>,
    ) -> Result<SettingsSetResponse, RpcStatus> {
        self.set_with(name, value, expected_revision, PluginsSettingsSetOptions::default()).await
    }
    pub async fn set_with(
        &self, name: impl Into<String>, value: SettingValues, expected_revision: Vec<u8>, options: PluginsSettingsSetOptions,
    ) -> Result<SettingsSetResponse, RpcStatus> {
        let value = SettingsSetRequest {
            name: name.into(),
            value: Some(value),
            expected_revision: expected_revision,
            request_id: match options.request_id {
                Some(value) => value,
                None => request_id()?,
            },
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<SettingsSetRequest, SettingsSetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.plugins.Settings/Set"),
                request,
                SettingsSetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("plugins.settings.set", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
#[derive(Clone)]
pub struct Schemas {
    client: ClientTransport,
}
impl Schemas {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self) -> Result<Vec<SchemaDescription>, RpcStatus> {
        self.list_with(SchemasListOptions::default()).await
    }
    pub async fn list_with(&self, options: SchemasListOptions) -> Result<Vec<SchemaDescription>, RpcStatus> {
        let value = SchemasListRequest { schema: options.schema };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<SchemasListRequest, SchemasListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Schemas/List"),
                request,
                SchemasListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("schemas.list", error))?;
        let response = response.into_inner();
        Ok(response.schemas)
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
    pub async fn get(&self) -> Result<StatusGetResponse, RpcStatus> {
        self.get_with(StatusGetOptions::default()).await
    }
    pub async fn get_with(&self, options: StatusGetOptions) -> Result<StatusGetResponse, RpcStatus> {
        let value = StatusGetRequest {};
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<StatusGetRequest, StatusGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Status/Get"),
                request,
                StatusGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.get", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn watch(&self) -> Result<Watching<StatusWatchResponse>, RpcStatus> {
        self.watch_with(StatusWatchOptions::default()).await
    }
    pub async fn watch_with(&self, options: StatusWatchOptions) -> Result<Watching<StatusWatchResponse>, RpcStatus> {
        let value = StatusWatchRequest { at: options.at };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<StatusWatchRequest, StatusWatchResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Status/Watch"),
                request,
                StatusWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.watch", error))?;
        let response = response.with_action("status.watch");
        Ok(response)
    }
}
#[derive(Clone)]
pub struct StatusDiagnostics {
    client: ClientTransport,
}
impl StatusDiagnostics {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn get(&self, name: impl Into<String>, offset: u64, max_bytes: u64) -> Result<DiagnosticsGetResponse, RpcStatus> {
        self.get_with(name, offset, max_bytes, StatusDiagnosticsGetOptions::default()).await
    }
    pub async fn get_with(
        &self, name: impl Into<String>, offset: u64, max_bytes: u64, options: StatusDiagnosticsGetOptions,
    ) -> Result<DiagnosticsGetResponse, RpcStatus> {
        let value = DiagnosticsGetRequest {
            name: name.into(),
            offset: offset,
            max_bytes: max_bytes,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<DiagnosticsGetRequest, DiagnosticsGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.status.Diagnostics/Get"),
                request,
                DiagnosticsGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.diagnostics.get", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
#[derive(Clone)]
pub struct StatusLogs {
    client: ClientTransport,
}
impl StatusLogs {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self) -> Result<LogsListResponse, RpcStatus> {
        self.list_with(StatusLogsListOptions::default()).await
    }
    pub async fn list_with(&self, options: StatusLogsListOptions) -> Result<LogsListResponse, RpcStatus> {
        let value = LogsListRequest { at: options.at, page: options.page };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<LogsListRequest, LogsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.status.Logs/List"),
                request,
                LogsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.logs.list", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn watch(&self, max_bytes: u64, max_events: u32) -> Result<Watching<LogsWatchResponse>, RpcStatus> {
        self.watch_with(max_bytes, max_events, StatusLogsWatchOptions::default()).await
    }
    pub async fn watch_with(
        &self, max_bytes: u64, max_events: u32, options: StatusLogsWatchOptions,
    ) -> Result<Watching<LogsWatchResponse>, RpcStatus> {
        let value = LogsWatchRequest {
            at: options.at,
            max_bytes: max_bytes,
            max_events: max_events,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<LogsWatchRequest, LogsWatchResponse>(
                PathAndQuery::from_static("/harmos.client.v1.status.Logs/Watch"),
                request,
                LogsWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.logs.watch", error))?;
        let response = response.with_action("status.logs.watch");
        Ok(response)
    }
}
#[derive(Clone)]
pub struct StatusTraceEvents {
    client: ClientTransport,
}
impl StatusTraceEvents {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self) -> Result<TraceEventsListResponse, RpcStatus> {
        self.list_with(StatusTraceEventsListOptions::default()).await
    }
    pub async fn list_with(&self, options: StatusTraceEventsListOptions) -> Result<TraceEventsListResponse, RpcStatus> {
        let value = TraceEventsListRequest { at: options.at, page: options.page };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<TraceEventsListRequest, TraceEventsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.status.TraceEvents/List"),
                request,
                TraceEventsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.trace_events.list", error))?;
        let response = response.into_inner();
        Ok(response)
    }
    pub async fn watch(&self, max_bytes: u64, max_events: u32) -> Result<Watching<TraceEventsWatchResponse>, RpcStatus> {
        self.watch_with(max_bytes, max_events, StatusTraceEventsWatchOptions::default()).await
    }
    pub async fn watch_with(
        &self, max_bytes: u64, max_events: u32, options: StatusTraceEventsWatchOptions,
    ) -> Result<Watching<TraceEventsWatchResponse>, RpcStatus> {
        let value = TraceEventsWatchRequest {
            at: options.at,
            max_bytes: max_bytes,
            max_events: max_events,
        };
        let request = options.context.request(value);
        let response = self
            .client
            .watch_known::<TraceEventsWatchRequest, TraceEventsWatchResponse>(
                PathAndQuery::from_static("/harmos.client.v1.status.TraceEvents/Watch"),
                request,
                TraceEventsWatchResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("status.trace_events.watch", error))?;
        let response = response.with_action("status.trace_events.watch");
        Ok(response)
    }
}
#[derive(Clone)]
pub struct Streams {
    client: ClientTransport,
}
impl Streams {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn list(&self) -> Result<Vec<StreamIdentity>, RpcStatus> {
        self.list_with(StreamsListOptions::default()).await
    }
    pub async fn list_with(&self, options: StreamsListOptions) -> Result<Vec<StreamIdentity>, RpcStatus> {
        let value = StreamsListRequest { schema: options.schema };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<StreamsListRequest, StreamsListResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Streams/List"),
                request,
                StreamsListResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("streams.list", error))?;
        let response = response.into_inner();
        Ok(response.streams)
    }
}
#[derive(Clone)]
pub struct Time {
    client: ClientTransport,
}
impl Time {
    pub fn new(client: ClientTransport) -> Self {
        Self { client }
    }
    pub async fn get(&self) -> Result<Timestamp, RpcStatus> {
        self.get_with(TimeGetOptions::default()).await
    }
    pub async fn get_with(&self, options: TimeGetOptions) -> Result<Timestamp, RpcStatus> {
        let value = TimeGetRequest {};
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<TimeGetRequest, TimeGetResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Time/Get"),
                request,
                TimeGetResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("time.get", error))?;
        let response = response.into_inner();
        response.now.ok_or_else(|| RpcStatus::data_loss("Read Time.Get: Missing Payload"))
    }
    pub async fn sleep(&self, duration: Duration) -> Result<TimeSleepResponse, RpcStatus> {
        self.sleep_with(duration, TimeSleepOptions::default()).await
    }
    pub async fn sleep_with(&self, duration: Duration, options: TimeSleepOptions) -> Result<TimeSleepResponse, RpcStatus> {
        let value = TimeSleepRequest { duration: Some(duration) };
        let request = options.context.request(value);
        let response = self
            .client
            .unary::<TimeSleepRequest, TimeSleepResponse>(
                PathAndQuery::from_static("/harmos.client.v1.Time/Sleep"),
                request,
                TimeSleepResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("time.sleep", error))?;
        let response = response.into_inner();
        Ok(response)
    }
}
