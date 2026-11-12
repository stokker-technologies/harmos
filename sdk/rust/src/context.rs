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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use prost::Message as _;
use tokio::sync::{mpsc, oneshot, watch};

// Local Imports
use crate::{Error, OperationRequest, Result, message::fingerprint};
use harmos_protocol::plugin::instance_invoke_response;
use harmos_protocol::v1::{self};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type ChildResult = std::result::Result<Vec<u8>, Error>;
pub(crate) type Pending = BTreeMap<u64, oneshot::Sender<ChildResult>>;

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
    pub(crate) output: Option<mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceInvokeResponse, tonic::Status>>>,
    pub(crate) cancellation: watch::Sender<Option<String>>,
    pub(crate) requires: Arc<HashMap<String, Vec<u8>>>,
    pub(crate) sequence: u64,
    pub(crate) child: Option<Arc<dyn ChildTransport>>,
    pub(crate) values: Vec<v1::GuestValue>,
    pub(crate) samples: Option<mpsc::Sender<(Vec<u8>, Option<prost_types::Timestamp>)>>,
}

pub(crate) struct Calls {
    next: u64,
    pub pending: Pending,
}

/* === Enums === */

/* === Traits === */

pub(crate) trait ChildTransport: Send + Sync + Any {
    fn call<'a>(
        &'a self, request: v1::ChildRequest, values: Vec<v1::GuestValue>, submit: bool,
    ) -> crate::operation::LocalFuture<'a, Result<Vec<u8>>>;
}

/* === Implementations === */

impl Context {
    pub(crate) fn new(request: v1::Request, plugin: String, requires: Arc<HashMap<String, Vec<u8>>>) -> Self {
        Self {
            invocation: request.invocation,
            attempt: request.attempt,
            scope: request.scope,
            operation: request.operation,
            plugin,
            resources: Arc::new(Mutex::new(HashMap::new())),
            calls: Arc::new(Mutex::new(Calls { next: 0, pending: BTreeMap::new() })),
            output: None,
            child: None,
            values: Vec::new(),
            cancellation: watch::channel(None).0,
            requires,
            sequence: 0,
            samples: None,
        }
    }

    /// Require a Caller-Bound Scope for This Invocation.
    pub fn scope(&self) -> Result<&str> {
        if self.scope.is_empty() {
            return Err(v1::Failure {
                classification: v1::Classification::Refused as i32,
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
            .ok_or_else(|| Error::Invalid(format!("Missing Resource: {}", std::any::type_name::<T>())))
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

    pub async fn call_with<Q: OperationRequest>(&self, request: Q, policy: Option<v1::OperationPolicy>) -> Result<Q::Response> {
        let fingerprint = fingerprint::<Q, Q::Response>(Q::ID)?;
        if self.requires.get(Q::ID) != Some(&fingerprint) {
            return Err(Error::Invalid(format!("Undeclared Requirement: {}", Q::ID)));
        }
        let payload = self.send_call(Q::ID, request.encode_to_vec(), fingerprint, policy).await?;
        Q::Response::decode(payload.as_slice()).map_err(|error| tonic::Status::invalid_argument(error.to_string()).into())
    }

    /// Every wait is a child call to the host. No SDK timer advances scenario time.
    /// Declare `.requires::<harmos::clock::TimeSleepRequest>()` when composing the plugin.
    pub async fn sleep(&self, duration: Duration) -> Result<()> {
        let duration = prost_types::Duration::try_from(duration).map_err(|error| Error::Invalid(error.to_string()))?;
        let request = v1::TimeSleepRequest { duration: Some(duration) };
        let payload = self.clock_call("harmos.time.sleep", request.encode_to_vec()).await?;
        v1::TimeSleepResponse::decode(payload.as_slice()).map_err(|error| tonic::Status::invalid_argument(error.to_string()))?;
        Ok(())
    }

    /// Read evidence time from the host; declare `.requires::<harmos::clock::TimeGetRequest>()`.
    pub async fn now(&self) -> Result<prost_types::Timestamp> {
        let payload = self.clock_call("harmos.time.get", v1::TimeGetRequest {}.encode_to_vec()).await?;
        v1::TimeGetResponse::decode(payload.as_slice())
            .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?
            .now
            .ok_or_else(|| tonic::Status::invalid_argument("Host Time Missing").into())
    }

    async fn clock_call(&self, id: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        let fingerprint = self.requires.get(id).ok_or_else(|| Error::Invalid(format!("Undeclared Requirement: {id}")))?.clone();
        self.send_call(id, payload, fingerprint, None).await
    }

    async fn send_call(
        &self, operation: &str, payload: Vec<u8>, fingerprint: Vec<u8>, policy: Option<v1::OperationPolicy>,
    ) -> Result<Vec<u8>> {
        if let Some(child) = &self.child {
            return child
                .call(
                    v1::ChildRequest {
                        call: 0,
                        operation: operation.into(),
                        payload,
                        fingerprint,
                        policy,
                    },
                    self.values.clone(),
                    false,
                )
                .await;
        }
        let output = self.output.as_ref().ok_or_else(|| Error::Invalid("Observers Cannot Call Operations or Wait".into()))?;
        if let Some(reason) = self.cancellation.borrow().clone() {
            return Err(self.cancel_failure(reason));
        }
        let (send, receive) = oneshot::channel();
        let call = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            calls.next += 1;
            let call = calls.next;
            calls.pending.insert(call, send);
            call
        };
        let child = v1::ChildRequest {
            call,
            operation: operation.into(),
            payload,
            fingerprint,
            policy,
        };
        output
            .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                frame: Some(instance_invoke_response::Frame::Child(child)),
            }))
            .await
            .map_err(|_| tonic::Status::unavailable("Host Closed"))?;
        receive.await.map_err(|_| tonic::Status::unavailable("Invocation Closed"))?
    }

    pub(crate) fn cancel_failure(&self, reason: String) -> Error {
        v1::Failure {
            classification: v1::Classification::Cancelled as i32,
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
