/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

#[rustfmt::skip]
mod contracts;
#[cfg(not(target_arch = "wasm32"))]
mod native_output;
mod state;
mod subscription;
#[cfg(target_arch = "wasm32")]
mod wasm_output;

/* === Re-Exports === */

pub use state::StateHandle;
pub use subscription::{Subscription, SubscriptionItem, SubscriptionRow};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native_output::ChildOutput;
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_output::ChildOutput;

/* === Imports === */

// Standard Library Imports
use {
    std::any::Any,
    std::any::TypeId,
    std::any::type_name,
    std::collections::BTreeMap,
    std::collections::BTreeSet,
    std::collections::HashMap,
    std::future::Future,
    std::marker::PhantomData,
    std::mem,
    std::num::NonZeroUsize,
    std::result,
    std::sync::Arc,
    std::sync::Mutex,
    std::sync::atomic::{AtomicBool, Ordering},
    std::time::Duration,
};

// Third Party Imports
use prost::Message;
use prost_types::Timestamp;
use serde_json::Value;
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tonic::Status;

// Local Imports
use crate::message;
use crate::plugin::HostPluginsSettingsResource;
use crate::{Error, OperationRequest, Result, message::request_fingerprint, operation::LocalFuture};
use harmos_protocol::host::{TimeGetRequest, TimeSleepRequest};
use harmos_protocol::model::Classification;
use harmos_protocol::model::Failure;
use harmos_protocol::model::OperationPolicy;
use harmos_protocol::plugin::ChildCancel;
use harmos_protocol::plugin::ChildDemand;
use harmos_protocol::plugin::ChildRequest;
use harmos_protocol::plugin::GuestValue;
use harmos_protocol::plugin::Request;
use harmos_protocol::plugin::instance_invoke_response;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type ChildResult = result::Result<Vec<u8>, Error>;
type WatchResult = Result<Option<Vec<u8>>>;
pub(crate) type Pending = BTreeMap<u64, PendingCall>;
pub(crate) struct PendingCall {
    pub sender: oneshot::Sender<ChildResult>,
    pub group: Arc<()>,
    pub background: bool,
    pub sent: bool,
}
#[derive(Clone)]
pub(crate) struct WatchCall {
    pub sender: mpsc::Sender<WatchResult>,
    pub limit: usize,
    pub group: Arc<()>,
    pub sent: bool,
}

struct PendingDrop {
    context: Context,
    call: u64,
}
impl Drop for PendingDrop {
    fn drop(&mut self) {
        let mut calls = self.context.calls.lock().expect("Child Calls Poisoned");
        if let Some(pending) = calls.pending.remove(&self.call)
            && pending.sent
        {
            calls.cancelled.insert(self.call);
            calls.cancel_queue.insert(self.call);
            self.context.call_changes.notify_one();
        }
    }
}

/* === Structs === */

/// An operation identifier and JSON payload, resolved against the host's served contract.
pub struct Untyped {
    operation: String,
    payload: Value,
}

/// Resources belong to this execution, including its child calls; never another invocation.
/// Virtual time cannot observe background work started independently by a handler.
#[derive(Clone)]
pub struct Context {
    pub invocation: String,
    pub attempt: u32,
    pub(crate) scope: String,
    pub(crate) wind_down: Arc<AtomicBool>,
    pub(crate) operation: String,
    pub(crate) plugin: String,
    resources: Arc<Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
    pub(crate) calls: Arc<Mutex<Calls>>,
    pub(crate) output: Option<ChildOutput>,
    pub(crate) cancellation: watch::Sender<Option<String>>,
    pub(crate) requires: Arc<HashMap<String, Vec<u8>>>,
    pub(crate) sequence: u64,
    background: bool,
    group: Arc<()>,
    pub(crate) call_changes: Arc<Notify>,
    pub(crate) child: Option<Arc<dyn ChildTransport>>,
    pub(crate) values: Vec<GuestValue>,
    pub(crate) samples: Option<mpsc::Sender<(Vec<u8>, Option<Timestamp>)>>,
}

pub(crate) struct Calls {
    next: u64,
    pub pending: Pending,
    pub watches: BTreeMap<u64, WatchCall>,
    pub cancelled: BTreeSet<u64>,
    pub cancel_queue: BTreeSet<u64>,
}

/* === Enums === */

/* === Traits === */

/// An Input Accepted by `Context::operation`.
pub trait OperationInput {
    type Response;

    fn invoke(self, context: &Context, policy: Option<OperationPolicy>) -> impl Future<Output = Result<Self::Response>>;
}

impl<Q: OperationRequest> OperationInput for Q {
    type Response = Q::Response;

    async fn invoke(self, context: &Context, policy: Option<OperationPolicy>) -> Result<Self::Response> {
        context.typed_operation_with(self, policy).await
    }
}

impl OperationInput for Untyped {
    type Response = Value;

    async fn invoke(self, context: &Context, policy: Option<OperationPolicy>) -> Result<Value> {
        context.json_operation_with(self, policy).await
    }
}

pub(crate) trait ChildTransport: Send + Sync + Any {
    fn call<'a>(&'a self, request: ChildRequest, values: Vec<GuestValue>, submit: bool) -> LocalFuture<'a, Result<Vec<u8>>>;
}

/* === Implementations === */

impl Context {
    pub(crate) fn new(request: Request, plugin: String, requires: Arc<HashMap<String, Vec<u8>>>) -> Self {
        Self {
            invocation: request.invocation,
            attempt: request.attempt,
            scope: request.scope,
            wind_down: Arc::new(AtomicBool::new(false)),
            operation: request.operation,
            plugin,
            resources: Arc::new(Mutex::new(HashMap::new())),
            calls: Arc::new(Mutex::new(Calls {
                next: 0,
                pending: BTreeMap::new(),
                watches: BTreeMap::new(),
                cancelled: BTreeSet::new(),
                cancel_queue: BTreeSet::new(),
            })),
            output: None,
            child: None,
            values: Vec::new(),
            cancellation: watch::channel(None).0,
            requires,
            sequence: 0,
            background: false,
            group: Arc::new(()),
            call_changes: Arc::new(Notify::new()),
            samples: None,
        }
    }

    /// Require a Caller-Bound Scope for This Invocation.
    pub fn scope(&self) -> Result<&str> {
        if self.scope.is_empty() {
            return Err(Failure {
                classification: Classification::Refused as i32,
                code: "scope_required".into(),
                message: "Invocation Scope Is Required: the caller must bind a scope".into(),
                ..Default::default()
            }
            .into());
        }
        Ok(&self.scope)
    }

    /// The Host Requested Normal Completion of This Scope's Work.
    pub fn wind_down_requested(&self) -> bool {
        self.wind_down.load(Ordering::Acquire)
    }

    pub(crate) fn request_wind_down(&self) {
        self.wind_down.store(true, Ordering::Release);
    }

    pub fn set<T: Send + Sync + 'static>(&self, value: T) {
        self.resources.lock().expect("Resource Store Poisoned").insert(TypeId::of::<T>(), Arc::new(value));
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Result<Arc<T>> {
        self.resources
            .lock()
            .expect("Resource Store Poisoned")
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|value| value.downcast().ok())
            .ok_or_else(|| Error::Invalid(format!("Missing Resource: {}", type_name::<T>())))
    }

    /// The sequence of the sample being requested on this publication call.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub async fn cancelled(&self) -> String {
        let mut cancellation = self.cancellation.subscribe();
        loop {
            if let Some(reason) = cancellation.borrow_and_update().clone() {
                return reason;
            }
            if cancellation.changed().await.is_err() {
                return "Connection Closed".into();
            }
        }
    }

    /// Invoke a Typed Request or an Identifier With a JSON Object Through the Host.
    pub async fn operation<Q: OperationInput>(&self, request: Q) -> Result<Q::Response> {
        self.operation_with(request, None).await
    }

    pub async fn operation_with<Q: OperationInput>(&self, request: Q, policy: Option<OperationPolicy>) -> Result<Q::Response> {
        request.invoke(self, policy).await
    }

    /// Call a Served Operation With JSON; Conversion Belongs to the Host.
    async fn json_operation_with(&self, request: Untyped, policy: Option<OperationPolicy>) -> Result<Value> {
        let json = serde_json::to_string(&request.payload)
            .map_err(|error| Error::Invalid(format!("Call {}: Encode JSON Payload: {error}", request.operation)))?;
        let child = ChildRequest {
            operation: request.operation,
            json: Some(json),
            policy,
            ..Default::default()
        };
        let payload = self.send_call(child).await?;
        serde_json::from_slice(&payload).map_err(|error| Error::Invalid(format!("Read Operation Reply: Host Must Send JSON: {error}")))
    }

    async fn typed_operation_with<Q: OperationRequest>(&self, request: Q, policy: Option<OperationPolicy>) -> Result<Q::Response> {
        if Q::WATCH {
            return Err(Error::Invalid(format!("Call {}: Use Watch for a Streaming Contract", Q::ID)));
        }
        if self.output.is_none() && self.child.is_none() {
            return Err(Error::Invalid(format!("Call {}: Child Carrier Required", Q::ID)));
        }
        if let Some(output) = &self.output {
            output.check()?;
        }
        let fingerprint = request_fingerprint::<Q>()?;
        let payload = self
            .send_call(ChildRequest {
                operation: Q::ID.into(),
                payload: request.encode_to_vec(),
                fingerprint,
                policy,
                ..Default::default()
            })
            .await?;
        Q::Response::decode(payload.as_slice()).map_err(|error| Status::invalid_argument(error.to_string()).into())
    }

    /// Open a declared context Watch with a consumer-owned maximum encoded item size.
    pub async fn watch<Q: OperationRequest>(&self, request: Q, max_item_bytes: NonZeroUsize) -> Result<Watching<Q::Response>> {
        if !Q::WATCH {
            return Err(Error::Invalid(format!("Watch {}: Server-Streaming Contract Required", Q::ID)));
        }
        let output = self.output.as_ref().ok_or_else(|| Error::Invalid(format!("Watch {}: Child Carrier Required", Q::ID)))?;
        output.check()?;
        let fingerprint = request_fingerprint::<Q>()?;
        if self.requires.get(Q::ID) != Some(&fingerprint) {
            return Err(Error::Invalid(format!("Watch {}: Undeclared Requirement", Q::ID)));
        }
        let (sender, receiver) = mpsc::channel(1);
        let call = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            calls.next += 1;
            let call = calls.next;
            calls.watches.insert(
                call,
                WatchCall {
                    sender,
                    limit: max_item_bytes.get(),
                    group: self.group.clone(),
                    sent: false,
                },
            );
            call
        };
        let watching = Watching {
            context: self.clone(),
            call,
            receiver,
            closed: false,
            response: PhantomData,
        };
        let limit = self.calls.lock().expect("Child Calls Poisoned").watches[&call].limit;
        output
            .send(instance_invoke_response::Frame::Child(ChildRequest {
                call,
                operation: Q::ID.into(),
                payload: request.encode_to_vec(),
                fingerprint,
                watch: true,
                max_response_bytes: limit as u64,
                ..Default::default()
            }))
            .await?;
        if let Some(watch) = self.calls.lock().expect("Child Calls Poisoned").watches.get_mut(&call) {
            watch.sent = true;
        }
        Ok(watching)
    }

    /// Every wait is a child call to the host. No SDK timer advances scenario time.
    /// Register `HostOperation::TimeSleep` during the plugin register phase.
    pub async fn sleep(&self, duration: Duration) -> Result<()> {
        let duration = prost_types::Duration::try_from(duration).map_err(|error| Error::Invalid(error.to_string()))?;
        self.operation(TimeSleepRequest { duration: Some(duration) }).await?;
        Ok(())
    }

    /// Read evidence time from the host; register `HostOperation::TimeGet`.
    pub async fn now(&self) -> Result<Timestamp> {
        self.operation(TimeGetRequest {}).await?.now.ok_or_else(|| Status::invalid_argument("Get Host Time: Missing Payload").into())
    }

    /// Access the calling plugin entry's effective settings.
    pub fn settings(&self) -> HostPluginsSettingsResource {
        self.host().plugins.settings
    }

    async fn send_call(&self, mut child: ChildRequest) -> Result<Vec<u8>> {
        child.background = self.background;
        if let Some(transport) = &self.child {
            return transport.call(child, self.values.clone(), false).await;
        }
        let operation = &child.operation;
        let output = self.output.as_ref().ok_or_else(|| Error::Invalid(format!("Call {operation}: Child Carrier Required")))?;
        output.check()?;
        if !self.background
            && let Some(reason) = self.cancellation.borrow().clone()
        {
            return Err(self.cancel_failure(reason));
        }
        self.flush_cancels().await?;
        let (send, receive) = oneshot::channel();
        let call = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            calls.next += 1;
            let call = calls.next;
            calls.pending.insert(
                call,
                PendingCall {
                    sender: send,
                    group: self.group.clone(),
                    background: self.background,
                    sent: false,
                },
            );
            call
        };
        let _pending = PendingDrop { context: self.clone(), call };
        child.call = call;
        output.send(instance_invoke_response::Frame::Child(child)).await?;
        if let Some(pending) = self.calls.lock().expect("Child Calls Poisoned").pending.get_mut(&call) {
            pending.sent = true;
        }
        receive.await.map_err(|_| Status::unavailable("Invocation Closed"))?
    }

    pub(crate) async fn flush_cancels(&self) -> Result<()> {
        let cancelled = mem::take(&mut self.calls.lock().expect("Child Calls Poisoned").cancel_queue);
        if let Some(output) = &self.output {
            for call in cancelled {
                output.send(instance_invoke_response::Frame::ChildCancel(ChildCancel { call })).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn cancel_calls(&self, owned: bool, reason: &str, keep_guards: bool) -> Result<()> {
        let (pending, watches) = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            let ids = calls
                .pending
                .iter()
                .filter(|(_, call)| (!owned || Arc::ptr_eq(&call.group, &self.group)) && !(keep_guards && call.background))
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            let mut pending = Vec::new();
            for id in ids {
                let call = calls.pending.remove(&id).expect("selected child");
                if call.sent {
                    calls.cancelled.insert(id);
                    calls.cancel_queue.insert(id);
                }
                pending.push(call);
            }
            let ids = calls
                .watches
                .iter()
                .filter(|(_, call)| !owned || Arc::ptr_eq(&call.group, &self.group))
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            let mut watches = Vec::new();
            for id in ids {
                let watch = calls.watches.remove(&id).expect("selected watch");
                if watch.sent {
                    calls.cancelled.insert(id);
                    calls.cancel_queue.insert(id);
                }
                watches.push(watch.sender);
            }
            (pending, watches)
        };
        for call in pending {
            let _ = call.sender.send(Err(self.cancel_failure(reason.into())));
        }
        for watch in watches {
            let _ = watch.try_send(Err(self.cancel_failure(reason.into())));
        }
        self.flush_cancels().await
    }

    pub(crate) fn cancel_failure(&self, reason: String) -> Error {
        Failure {
            classification: Classification::Cancelled as i32,
            code: "cancelled".into(),
            message: reason,
            action: "Retry If Needed".into(),
            operation: self.operation.clone(),
            plugin: self.plugin.clone(),
            ..Default::default()
        }
        .into()
    }
}

/// A Child Watch Holds at Most One Demanded Item; Dropping It Cancels Its Demand.
#[must_use]
pub struct Watching<R: message::Message> {
    context: Context,
    call: u64,
    receiver: mpsc::Receiver<WatchResult>,
    closed: bool,
    response: PhantomData<fn() -> R>,
}
impl<R: message::Message> Watching<R> {
    pub fn close(&mut self) {
        self.closed = true;
        let mut calls = self.context.calls.lock().expect("Child Calls Poisoned");
        if let Some(watch) = calls.watches.remove(&self.call)
            && watch.sent
        {
            calls.cancelled.insert(self.call);
            calls.cancel_queue.insert(self.call);
            self.context.call_changes.notify_one();
        }
    }

    pub async fn next(&mut self) -> Result<Option<R>> {
        if self.closed {
            return Ok(None);
        }
        let output = self.context.output.as_ref().expect("Native Watch");
        let active = self.context.calls.lock().expect("Child Calls Poisoned").watches.contains_key(&self.call);
        if active {
            output.send(instance_invoke_response::Frame::ChildDemand(ChildDemand { call: self.call })).await?;
        }
        match self.receiver.recv().await {
            Some(Ok(Some(bytes))) => R::decode(bytes.as_slice()).map(Some).map_err(|error| Error::Invalid(format!("Watch: {error}"))),
            Some(Ok(None)) => {
                self.closed = true;
                Ok(None)
            }
            Some(Err(error)) => {
                self.closed = true;
                Err(error)
            }
            None => Err(Status::unavailable("Watch: Invocation Closed").into()),
        }
    }
}
impl<R: message::Message> Drop for Watching<R> {
    fn drop(&mut self) {
        self.close();
    }
}

/* === Public Functions === */

/// Call an operation by identifier with a standard Protobuf JSON payload.
pub fn untyped(operation: impl Into<String>, payload: Value) -> Untyped {
    Untyped { operation: operation.into(), payload }
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    use crate::{
        Plugin, SettingValues,
        plugin::{HostOperation, HostPluginsSettingsListOptions},
    };
    use harmos_protocol::host::plugins::{SettingsListRequest, SettingsListResponse};

    struct SettingsTransport {
        requests: Mutex<Vec<ChildRequest>>,
        response: SettingsListResponse,
    }

    impl ChildTransport for SettingsTransport {
        fn call<'a>(&'a self, request: ChildRequest, _: Vec<GuestValue>, _: bool) -> LocalFuture<'a, Result<Vec<u8>>> {
            self.requests.lock().unwrap().push(request);
            Box::pin(async { Ok(self.response.encode_to_vec()) })
        }
    }

    #[tokio::test]
    async fn settings_convenience_and_host_resource_share_operation_and_payload() {
        // Admit the Shared Settings Requirement.
        let declarations = Plugin::new("caller", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::PluginsSettingsList)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
            .declarations()
            .unwrap();
        let requires = declarations.requires.into_iter().map(|requirement| (requirement.id, requirement.fingerprint)).collect();
        let mut context = Context::new(Request::default(), "caller".into(), Arc::new(requires));
        let response = SettingsListResponse {
            value: Some(SettingValues::default()),
            revision: vec![1, 2, 3],
            ..Default::default()
        };
        let transport = Arc::new(SettingsTransport {
            requests: Mutex::new(Vec::new()),
            response: response.clone(),
        });
        context.child = Some(transport.clone());

        // Read Through Both Public Paths and Preserve the Full Reply.
        assert_eq!(context.settings().list().await.unwrap(), response);
        let options = HostPluginsSettingsListOptions {
            name: Some("caller".into()),
            ..Default::default()
        };
        assert_eq!(context.host().plugins.settings.list_with(options).await.unwrap(), response);

        // Preserve the Optional Caller Selector on One Shared Operation.
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].operation, requests[1].operation);
        assert_eq!(requests[0].fingerprint, requests[1].fingerprint);
        assert_eq!(SettingsListRequest::decode(requests[0].payload.as_slice()).unwrap().name, None);
        assert_eq!(SettingsListRequest::decode(requests[1].payload.as_slice()).unwrap().name.as_deref(), Some("caller"));
    }

    #[derive(Clone, PartialEq, prost::Message)]
    struct OpaqueState {
        #[prost(bytes = "vec", tag = "1")]
        effects: Vec<u8>,
    }

    struct StateTransport {
        value: Mutex<harmos_protocol::host::plugins::StateGetResponse>,
    }

    impl ChildTransport for StateTransport {
        fn call<'a>(&'a self, request: ChildRequest, _: Vec<GuestValue>, _: bool) -> LocalFuture<'a, Result<Vec<u8>>> {
            Box::pin(async move {
                let mut saved = self.value.lock().unwrap();
                if request.operation.ends_with("/Get") {
                    return Ok(saved.encode_to_vec());
                }
                let request = harmos_protocol::host::plugins::StatePutRequest::decode(request.payload.as_slice()).unwrap();
                if request.expected_revision != saved.revision {
                    return Err(Error::Invalid("state_revision_conflict".into()));
                }
                saved.revision += 1;
                saved.value = request.value;
                Ok(harmos_protocol::host::plugins::StatePutResponse { revision: saved.revision }.encode_to_vec())
            })
        }
    }

    #[tokio::test]
    async fn typed_state_get_put_clear_share_revision_and_preserve_conflicts() {
        let declarations = Plugin::new("caller", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::PluginsStateGet)?;
                ctx.register(HostOperation::PluginsStatePut)?;
                Ok(())
            })
            .unwrap()
            .declarations()
            .unwrap();
        let requires = declarations.requires.into_iter().map(|requirement| (requirement.id, requirement.fingerprint)).collect();
        let mut context = Context::new(Request::default(), "caller".into(), Arc::new(requires));
        context.child = Some(Arc::new(StateTransport { value: Mutex::new(Default::default()) }));
        let state = context.state();
        assert_eq!(state.get::<OpaqueState>().await.unwrap(), OpaqueState::default());
        let value = OpaqueState { effects: vec![0, 255, 7] };
        assert_eq!(state.put(&value).await.unwrap(), 1);
        let retained = state.clone();
        assert_eq!(retained.get::<OpaqueState>().await.unwrap(), value);
        assert!(context.state().put(&value).await.is_err());
        assert_eq!(retained.clear().await.unwrap(), 2);
        assert_eq!(state.get::<OpaqueState>().await.unwrap(), OpaqueState::default());
    }

}
