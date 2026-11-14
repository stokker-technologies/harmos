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
#[cfg(target_arch = "wasm32")]
mod wasm_output;

/* === Re-Exports === */

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native_output::ChildOutput;
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_output::ChildOutput;

/* === Imports === */

// Standard Library Imports
use std::{
    any::type_name,
    any::{Any, TypeId},
    collections::{BTreeMap, BTreeSet, HashMap},
    marker::PhantomData,
    mem,
    num::NonZeroUsize,
    result::Result as StdResult,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use prost::Message as _;
use prost_types::{Duration as ProtoDuration, Timestamp};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tonic::Status;

// Local Imports
use crate::plugin::HostPluginsSettingsResource;
use crate::{Error, Message, Operation, OperationRequest, Result, message::request_fingerprint, operation::LocalFuture};
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
use harmos_protocol::policy::RetryPolicy;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type ChildResult = StdResult<Vec<u8>, Error>;
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

/// Resources belong to this execution, including its child calls; never another invocation.
/// Virtual time cannot observe background work started independently by a handler.
#[derive(Clone)]
pub struct Context {
    pub invocation: String,
    pub attempt: u32,
    pub(crate) scope: String,
    pub(crate) operation: String,
    pub(crate) plugin: String,
    resources: Arc<Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
    pub(crate) calls: Arc<Mutex<Calls>>,
    pub(crate) output: Option<ChildOutput>,
    pub(crate) cancellation: watch::Sender<Option<String>>,
    pub(crate) requires: Arc<HashMap<String, Vec<u8>>>,
    pub(crate) local: Arc<HashMap<String, Arc<Operation>>>,
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
            local: Arc::new(HashMap::new()),
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

    pub async fn call<Q: OperationRequest>(&self, request: Q) -> Result<Q::Response> {
        self.call_with(request, None).await
    }

    pub async fn call_with<Q: OperationRequest>(&self, request: Q, policy: Option<OperationPolicy>) -> Result<Q::Response> {
        if Q::WATCH {
            return Err(Error::Invalid(format!("Call {}: Use Watch for a Streaming Contract", Q::ID)));
        }
        if self.output.is_none() && self.child.is_none() {
            return Err(Error::Invalid(format!("Call {}: Child Carrier Required", Q::ID)));
        }
        if let Some(output) = &self.output {
            output.check()?;
        }
        if let Some(operation) = self.local.get(Q::ID) {
            if operation.contract.request != Q::name() || operation.contract.response != Q::Response::name() {
                return Err(Error::Invalid(format!("Call {}: Local Contract Differs", Q::ID)));
            }
            let payload = self.invoke_local(operation, request.encode_to_vec(), policy).await?;
            return Q::Response::decode(payload.as_slice()).map_err(|error| Status::invalid_argument(error.to_string()).into());
        }
        let fingerprint = request_fingerprint::<Q>()?;
        if self.requires.get(Q::ID) != Some(&fingerprint) {
            return Err(Error::Invalid(format!("Undeclared Requirement: {}", Q::ID)));
        }
        let payload = self.send_call(Q::ID, request.encode_to_vec(), fingerprint, policy).await?;
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

    async fn invoke_local(&self, operation: &Operation, payload: Vec<u8>, override_policy: Option<OperationPolicy>) -> Result<Vec<u8>> {
        use harmos_protocol::policy::{FailureKind, OperationPolicy, ReplaySafety, UNBOUNDED};
        let policy = OperationPolicy::try_from(override_policy.as_ref().or(operation.contract.policy.as_ref()))
            .map_err(|error| Error::Invalid(format!("Invoke {}: {error}", operation.contract.id)))?;
        let replay = if operation.contract.replay_safe {
            ReplaySafety::Idempotent
        } else {
            ReplaySafety::Unknown
        };
        if replay == ReplaySafety::Unknown && !matches!(policy.retries, RetryPolicy::Never) {
            return Err(Error::Invalid(format!("Invoke {}: Retry Requires Replay Safety", operation.contract.id)));
        }
        let mut context = self.clone();
        context.operation = operation.contract.id.clone();
        context.group = Arc::new(());
        context.resources = Arc::new(Mutex::new(HashMap::new()));
        context.cancellation = watch::channel(None).0;
        let mut guard = self.clone();
        guard.background = true;
        guard.group = Arc::new(());
        let started = if policy.timeout < UNBOUNDED {
            Some(self.now().await?)
        } else {
            None
        };
        let mut attempt = 0;
        loop {
            if let Some(reason) = self.cancellation.borrow().clone() {
                return Err(context.cancel_failure(reason));
            }
            context.attempt = attempt + 1;
            context.resources = Arc::new(Mutex::new(HashMap::new()));
            context.cancellation = watch::channel(None).0;
            let remaining = if let Some(started) = &started {
                let now = self.now().await?;
                let elapsed = elapsed(started, &now);
                policy.timeout.saturating_sub(elapsed)
            } else {
                UNBOUNDED
            };
            if remaining.is_zero() {
                return Err(Error::failure(
                    Classification::TimedOut,
                    "deadline",
                    format!("Invoke {}: Deadline Reached", operation.contract.id),
                    "Invoke Again If Safe",
                ));
            }
            let bound = remaining.min(policy.attempt_timeout);
            let result = Box::pin(async {
                let executing = operation.handler.execute(&context, payload.clone());
                tokio::pin!(executing);
                let deadline = guard.sleep(bound);
                tokio::pin!(deadline);
                let (classification, reason) = tokio::select! {
                    result = &mut executing => return result,
                    reason = self.cancelled() => (Classification::Cancelled, reason),
                    result = &mut deadline, if bound < UNBOUNDED => {
                        result?;
                        (Classification::TimedOut, "Deadline Reached".into())
                    }
                };
                context.cancellation.send_replace(Some(reason.clone()));
                context.cancel_calls(true, &reason, false).await?;
                if classification == Classification::Cancelled && !policy.cancellation_timeout.is_zero() {
                    tokio::select! {
                        result = &mut executing => return result,
                        _ = guard.sleep(policy.cancellation_timeout) => {},
                        _ = &mut deadline, if bound < UNBOUNDED => {},
                    }
                }
                Err(Error::failure(
                    classification,
                    "interrupted",
                    format!("Invoke {}: {reason}", operation.contract.id),
                    "Invoke Again If Safe",
                ))
            })
            .await;
            guard.cancel_calls(true, "Policy Guard Settled", false).await?;
            context.cancel_calls(true, "Local Attempt Settled", false).await?;
            let kind = match &result {
                Err(Error::Failure(failure)) if failure.classification() == Classification::Transient => FailureKind::Transient,
                _ => return result,
            };
            let Some(mut delay) = policy.retries.retry_after(attempt as usize, kind, replay, 0) else {
                return result;
            };
            if let Some(started) = &started {
                let now = self.now().await?;
                let elapsed = elapsed(started, &now);
                delay = delay.min(policy.timeout.saturating_sub(elapsed));
            }
            if !delay.is_zero() {
                self.sleep(delay).await?;
            }
            attempt += 1;
        }
    }

    /// Every wait is a child call to the host. No SDK timer advances scenario time.
    /// Declare `.requires_host(HostOperation::TimeSleep)` when composing the plugin.
    pub async fn sleep(&self, duration: Duration) -> Result<()> {
        let duration = ProtoDuration::try_from(duration).map_err(|error| Error::Invalid(error.to_string()))?;
        self.clock_call(TimeSleepRequest { duration: Some(duration) }).await?;
        Ok(())
    }

    /// Read evidence time from the host; declare `.requires_host(HostOperation::TimeGet)`.
    pub async fn now(&self) -> Result<Timestamp> {
        self.clock_call(TimeGetRequest {}).await?.now.ok_or_else(|| Status::invalid_argument("Get Host Time: Missing Payload").into())
    }

    /// Access the calling plugin entry's effective settings.
    pub fn settings(&self) -> HostPluginsSettingsResource {
        self.host().plugins.settings
    }

    // Local Operation Policies Depend on the Clock, so Clock Calls Go Straight to the Host.
    async fn clock_call<Q: OperationRequest>(&self, request: Q) -> Result<Q::Response> {
        let fingerprint = self.requires.get(Q::ID).ok_or_else(|| Error::Invalid(format!("Undeclared Requirement: {}", Q::ID)))?.clone();
        let payload = self.send_call(Q::ID, request.encode_to_vec(), fingerprint, None).await?;
        Q::Response::decode(payload.as_slice()).map_err(|error| Status::invalid_argument(error.to_string()).into())
    }

    async fn send_call(&self, operation: &str, payload: Vec<u8>, fingerprint: Vec<u8>, policy: Option<OperationPolicy>) -> Result<Vec<u8>> {
        if let Some(child) = &self.child {
            return child
                .call(
                    ChildRequest {
                        call: 0,
                        operation: operation.into(),
                        payload,
                        fingerprint,
                        policy,
                        background: self.background,
                        ..Default::default()
                    },
                    self.values.clone(),
                    false,
                )
                .await;
        }
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
        let child = ChildRequest {
            call,
            operation: operation.into(),
            payload,
            fingerprint,
            policy,
            background: self.background,
            ..Default::default()
        };
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
pub struct Watching<R: Message> {
    context: Context,
    call: u64,
    receiver: mpsc::Receiver<WatchResult>,
    closed: bool,
    response: PhantomData<fn() -> R>,
}
impl<R: Message> Watching<R> {
    pub async fn next(&mut self) -> Result<Option<R>> {
        if self.closed {
            return Ok(None);
        }
        let output = self.context.output.as_ref().expect("Native Watch");
        output.send(instance_invoke_response::Frame::ChildDemand(ChildDemand { call: self.call })).await?;
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
impl<R: Message> Drop for Watching<R> {
    fn drop(&mut self) {
        let mut calls = self.context.calls.lock().expect("Child Calls Poisoned");
        if let Some(watch) = calls.watches.remove(&self.call)
            && watch.sent
        {
            calls.cancelled.insert(self.call);
            calls.cancel_queue.insert(self.call);
            self.context.call_changes.notify_one();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn elapsed(started: &Timestamp, now: &Timestamp) -> Duration {
    let nanos = ((i128::from(now.seconds) - i128::from(started.seconds)) * 1_000_000_000 + i128::from(now.nanos)
        - i128::from(started.nanos))
    .max(0) as u128;
    Duration::new((nanos / 1_000_000_000).min(u128::from(u64::MAX)) as u64, (nanos % 1_000_000_000) as u32)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

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
        let declarations = Plugin::new("caller", "1.0.0").requires_host(HostOperation::PluginsSettingsList).declarations().unwrap();
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

    #[test]
    fn policy_clock_spans_keep_large_durations_and_fractional_seconds() {
        let started = Timestamp {
            seconds: -30_000_000_000,
            nanos: 900_000_000,
        };
        let now = Timestamp {
            seconds: 10_000_000_001,
            nanos: 100_000_000,
        };
        let span = elapsed(&started, &now);
        assert_eq!(span, Duration::new(40_000_000_000, 200_000_000));
        assert_eq!(Duration::from_secs(40_000_000_000).saturating_sub(span), Duration::ZERO);
        assert_eq!(elapsed(&now, &started), Duration::ZERO);
    }
}
