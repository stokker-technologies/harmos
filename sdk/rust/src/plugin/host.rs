// @generated from protocol descriptors; do not edit.
use crate::{Context, Error, OperationPolicy, Plugin, Result};
use harmos_protocol::host::InvocationsCancelRequest;
use harmos_protocol::host::InvocationsCancelResponse;
use harmos_protocol::host::InvocationsGetRequest;
use harmos_protocol::host::InvocationsGetResponse;
use harmos_protocol::host::InvocationsWatchRequest;
use harmos_protocol::host::InvocationsWatchResponse;
use harmos_protocol::host::OperationsGetRequest;
use harmos_protocol::host::OperationsGetResponse;
use harmos_protocol::host::OperationsListRequest;
use harmos_protocol::host::OperationsListResponse;
use harmos_protocol::host::TimeGetRequest;
use harmos_protocol::host::TimeGetResponse;
use harmos_protocol::host::TimeSleepRequest;
use harmos_protocol::host::TimeSleepResponse;
use harmos_protocol::host::plugins::SettingsListRequest;
use harmos_protocol::host::plugins::SettingsListResponse;
use harmos_protocol::model::DiscoveredOperation;
use harmos_protocol::model::InstanceState;
use harmos_protocol::model::ReadCut;
use prost_types::Duration;
use prost_types::Timestamp;
use std::num::NonZeroUsize;
pub enum HostOperation {
    InvocationsGet,
    InvocationsCancel,
    InvocationsWatch,
    OperationsList,
    OperationsGet,
    PluginsSettingsList,
    TimeGet,
    TimeSleep,
}
impl Plugin {
    pub fn requires_host(self, operation: HostOperation) -> Self {
        match operation {
            HostOperation::InvocationsGet => self.requires::<InvocationsGetRequest>(),
            HostOperation::InvocationsCancel => self.requires::<InvocationsCancelRequest>(),
            HostOperation::InvocationsWatch => self.requires::<InvocationsWatchRequest>(),
            HostOperation::OperationsList => self.requires::<OperationsListRequest>(),
            HostOperation::OperationsGet => self.requires::<OperationsGetRequest>(),
            HostOperation::PluginsSettingsList => self.requires::<SettingsListRequest>(),
            HostOperation::TimeGet => self.requires::<TimeGetRequest>(),
            HostOperation::TimeSleep => self.requires::<TimeSleepRequest>(),
        }
    }
}
impl Context {
    pub fn host(&self) -> HostRoot {
        HostRoot::new(self.clone())
    }
}
#[derive(Clone)]
pub struct HostRoot {
    pub invocations: HostInvocationsResource,
    pub operations: HostOperationsResource,
    pub plugins: HostPluginsResource,
    pub time: HostTimeResource,
}
impl HostRoot {
    fn new(context: Context) -> Self {
        Self {
            invocations: HostInvocationsResource::new(context.clone()),
            operations: HostOperationsResource::new(context.clone()),
            plugins: HostPluginsResource::new(context.clone()),
            time: HostTimeResource::new(context.clone()),
        }
    }
}
#[derive(Default)]
pub struct HostInvocationsGetOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostInvocationsCancelOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostInvocationsWatchOptions {}
pub struct HostInvocationsWatch {
    incoming: crate::context::Watching<InvocationsWatchResponse>,
}
impl HostInvocationsWatch {
    pub async fn next(&mut self) -> Result<Option<InstanceState>> {
        let Some(response) = self.incoming.next().await? else {
            return Ok(None);
        };
        response.invocation.map(Some).ok_or_else(|| Error::Invalid("Read InvocationsWatch: Missing Payload".into()))
    }
}
#[derive(Default)]
pub struct HostOperationsListOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostOperationsGetOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostPluginsSettingsListOptions {
    pub name: Option<String>,
    pub at: Option<ReadCut>,
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostTimeGetOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Default)]
pub struct HostTimeSleepOptions {
    pub policy: Option<OperationPolicy>,
}
#[derive(Clone)]
pub struct HostInvocationsResource {
    context: Context,
}
impl HostInvocationsResource {
    fn new(context: Context) -> Self {
        Self { context }
    }
    pub async fn get(&self, instance: u64) -> Result<InstanceState> {
        self.get_with(instance, HostInvocationsGetOptions::default()).await
    }
    pub async fn get_with(&self, instance: u64, options: HostInvocationsGetOptions) -> Result<InstanceState> {
        let request = InvocationsGetRequest { instance };
        let response: InvocationsGetResponse = self.context.call_with(request, options.policy).await?;
        response.invocation.ok_or_else(|| Error::Invalid("Read InvocationsGet: Missing Payload".into()))
    }
    pub async fn cancel(&self, instance: u64, reason: String) -> Result<InstanceState> {
        self.cancel_with(instance, reason, HostInvocationsCancelOptions::default()).await
    }
    pub async fn cancel_with(&self, instance: u64, reason: String, options: HostInvocationsCancelOptions) -> Result<InstanceState> {
        let request = InvocationsCancelRequest { instance, reason };
        let response: InvocationsCancelResponse = self.context.call_with(request, options.policy).await?;
        response.invocation.ok_or_else(|| Error::Invalid("Read InvocationsCancel: Missing Payload".into()))
    }
    pub async fn watch(&self, instance: u64, max_item_bytes: NonZeroUsize) -> Result<HostInvocationsWatch> {
        self.watch_with(instance, max_item_bytes, HostInvocationsWatchOptions::default()).await
    }
    pub async fn watch_with(
        &self, instance: u64, max_item_bytes: NonZeroUsize, options: HostInvocationsWatchOptions,
    ) -> Result<HostInvocationsWatch> {
        let request = InvocationsWatchRequest { instance };
        let HostInvocationsWatchOptions {} = options;
        Ok(HostInvocationsWatch {
            incoming: self.context.watch(request, max_item_bytes).await?,
        })
    }
}
#[derive(Clone)]
pub struct HostOperationsResource {
    context: Context,
}
impl HostOperationsResource {
    fn new(context: Context) -> Self {
        Self { context }
    }
    pub async fn list(&self) -> Result<Vec<DiscoveredOperation>> {
        self.list_with(HostOperationsListOptions::default()).await
    }
    pub async fn list_with(&self, options: HostOperationsListOptions) -> Result<Vec<DiscoveredOperation>> {
        let request = OperationsListRequest {};
        let response: OperationsListResponse = self.context.call_with(request, options.policy).await?;
        Ok(response.operations)
    }
    pub async fn get(&self, id: String) -> Result<DiscoveredOperation> {
        self.get_with(id, HostOperationsGetOptions::default()).await
    }
    pub async fn get_with(&self, id: String, options: HostOperationsGetOptions) -> Result<DiscoveredOperation> {
        let request = OperationsGetRequest { id };
        let response: OperationsGetResponse = self.context.call_with(request, options.policy).await?;
        response.operation.ok_or_else(|| Error::Invalid("Read OperationsGet: Missing Payload".into()))
    }
}
#[derive(Clone)]
pub struct HostPluginsResource {
    pub settings: HostPluginsSettingsResource,
}
impl HostPluginsResource {
    fn new(context: Context) -> Self {
        Self {
            settings: HostPluginsSettingsResource::new(context.clone()),
        }
    }
}
#[derive(Clone)]
pub struct HostPluginsSettingsResource {
    context: Context,
}
impl HostPluginsSettingsResource {
    fn new(context: Context) -> Self {
        Self { context }
    }
    pub async fn list(&self) -> Result<SettingsListResponse> {
        self.list_with(HostPluginsSettingsListOptions::default()).await
    }
    pub async fn list_with(&self, options: HostPluginsSettingsListOptions) -> Result<SettingsListResponse> {
        let request = SettingsListRequest { name: options.name, at: options.at };
        let response: SettingsListResponse = self.context.call_with(request, options.policy).await?;
        Ok(response)
    }
}
#[derive(Clone)]
pub struct HostTimeResource {
    context: Context,
}
impl HostTimeResource {
    fn new(context: Context) -> Self {
        Self { context }
    }
    pub async fn get(&self) -> Result<Timestamp> {
        self.get_with(HostTimeGetOptions::default()).await
    }
    pub async fn get_with(&self, options: HostTimeGetOptions) -> Result<Timestamp> {
        let request = TimeGetRequest {};
        let response: TimeGetResponse = self.context.call_with(request, options.policy).await?;
        response.now.ok_or_else(|| Error::Invalid("Read TimeGet: Missing Payload".into()))
    }
    pub async fn sleep(&self, duration: Duration) -> Result<TimeSleepResponse> {
        self.sleep_with(duration, HostTimeSleepOptions::default()).await
    }
    pub async fn sleep_with(&self, duration: Duration, options: HostTimeSleepOptions) -> Result<TimeSleepResponse> {
        let request = TimeSleepRequest { duration: Some(duration) };
        let response: TimeSleepResponse = self.context.call_with(request, options.policy).await?;
        Ok(response)
    }
}
