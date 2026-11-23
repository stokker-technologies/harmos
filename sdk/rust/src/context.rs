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
#[cfg(target_arch = "wasm32")]
mod guest;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod native_output;
mod state;
mod subscription;

/* === Re-Exports === */

pub use state::StateHandle;
pub use subscription::{Subscription, SubscriptionItem, SubscriptionRow};

#[cfg(target_arch = "wasm32")]
use guest::Queue;
#[cfg(target_arch = "wasm32")]
pub use guest::Watching;
#[cfg(not(target_arch = "wasm32"))]
use native::Queue;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Watching;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native_output::ChildOutput;

/* === Imports === */

// Standard Library Imports
use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost::Message;
use prost_types::Timestamp;
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tonic::Status;

// Local Imports
use crate::plugin::HostPluginsSettingsResource;
use crate::{Error, OperationRequest, Result, message::request_fingerprint, operation::LocalFuture};
use harmos_protocol::host::{TimeGetRequest, TimeSleepRequest};
use harmos_protocol::model::Classification;
use harmos_protocol::model::Failure;
use harmos_protocol::model::OperationPolicy;
use harmos_protocol::plugin::ChildRequest;
use harmos_protocol::plugin::GuestValue;
use harmos_protocol::plugin::Request;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

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
    pub capturing_executions: Vec<String>,
    pub attempt: u32,
    pub(crate) scope: String,
    pub(crate) wind_down: Arc<AtomicBool>,
    pub(crate) operation: String,
    pub(crate) plugin: String,
    resources: Arc<Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
    pub(crate) queue: Queue,
    pub(crate) cancellation: watch::Sender<Option<String>>,
    pub(crate) requires: Arc<HashMap<String, Vec<u8>>>,
    pub(crate) sequence: u64,
    pub(crate) values: Vec<GuestValue>,
    pub(crate) samples: Option<mpsc::Sender<(Vec<u8>, Option<Timestamp>)>>,
    pub(crate) failure_mapper: Option<crate::error::FailureMapper>,
}

/* === Enums === */

/* === Traits === */

/// An Input Accepted by `Context::operation`.
pub trait OperationInput {
    type Response;

    fn invoke(self, context: &Context, policy: Option<OperationPolicy>) -> impl Future<Output = Result<Self::Response>>;
}

pub(crate) trait ChildTransport: Send + Sync + Any {
    fn call<'a>(&'a self, request: ChildRequest, values: Vec<GuestValue>, submit: bool) -> LocalFuture<'a, Result<Vec<u8>>>;
}

/* === Implementations === */

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

impl Context {
    pub(crate) fn new(request: Request, plugin: String, requires: Arc<HashMap<String, Vec<u8>>>) -> Self {
        Self {
            invocation: request.invocation,
            capturing_executions: request.capturing_executions,
            attempt: request.attempt,
            scope: request.scope,
            wind_down: Arc::new(AtomicBool::new(false)),
            operation: request.operation,
            plugin,
            resources: Arc::new(Mutex::new(HashMap::new())),
            queue: Queue::default(),
            values: Vec::new(),
            cancellation: watch::channel(None).0,
            requires,
            sequence: 0,
            samples: None,
            failure_mapper: None,
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
        context.queue.child = Some(transport.clone());

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
        context.queue.child = Some(Arc::new(StateTransport { value: Mutex::new(Default::default()) }));
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
