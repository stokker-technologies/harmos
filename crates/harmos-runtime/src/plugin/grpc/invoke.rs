/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned Plugin Invocations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

// Third Party Imports
use futures_util::{StreamExt, stream};
use harmos_protocol::v1::{self, host_frame, plugin_frame};
use tokio::sync::mpsc;

// Local Imports
use super::{Activation, connection::Connection};
use crate::operations::{ErasedResponse, FailureKind, HostContext, NativeInstance, ServiceFuture};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub(crate) struct Binding {
    pub(crate) owner: String,
    pub(crate) contract: v1::OperationContract,
    pub(crate) requires: Vec<v1::Requirement>,
    target: Target,
}

struct Invocation {
    binding: Binding,
    payload: Vec<u8>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("{}", .0.message)]
    Remote(v1::Failure),

    #[error("plugin transport failed: {0}")]
    Transport(tonic::Status),

    #[error("plugin protocol failed: {0}")]
    Protocol(String),
}

#[derive(Clone)]
enum Target {
    Plugin(Arc<Connection>),
    Clock(crate::clock::Clock),
}

/* === Traits === */

/* === Implementations === */

impl Binding {
    pub(crate) fn new(activation: &Activation, contract: v1::OperationContract) -> Self {
        Self {
            owner: activation.description.id.clone(),
            contract,
            requires: activation.description.catalog.as_ref().unwrap().requires.clone(),
            target: Target::Plugin(activation.connection.clone()),
        }
    }

    pub(crate) fn clock(contract: v1::OperationContract, clock: crate::clock::Clock) -> Self {
        Self {
            owner: "host".into(),
            contract,
            requires: Vec::new(),
            target: Target::Clock(clock),
        }
    }

    pub(crate) fn prepare(&self, payload: Vec<u8>) -> Box<dyn NativeInstance> {
        Box::new(Invocation { binding: self.clone(), payload })
    }
}

impl CallError {
    pub fn classification(&self) -> v1::Classification {
        match self {
            Self::Remote(failure) => failure.classification(),
            Self::Transport(status) => match status.code() {
                tonic::Code::Cancelled => v1::Classification::Cancelled,
                tonic::Code::DeadlineExceeded => v1::Classification::TimedOut,
                tonic::Code::Unavailable | tonic::Code::Unknown | tonic::Code::Internal => v1::Classification::Transient,
                tonic::Code::NotFound | tonic::Code::PermissionDenied | tonic::Code::FailedPrecondition => v1::Classification::Refused,
                _ => v1::Classification::Permanent,
            },
            Self::Protocol(_) => v1::Classification::Permanent,
        }
    }
}

impl NativeInstance for Invocation {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        match error.downcast_ref::<CallError>().map(CallError::classification) {
            Some(v1::Classification::Transient) => FailureKind::Transient,
            Some(v1::Classification::Cancelled) => FailureKind::Cancelled,
            Some(v1::Classification::Refused) => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn initialize(&mut self, _: harmos_sdk::ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn run(&mut self, context: harmos_sdk::ExecutionContext) -> ServiceFuture<'_, ErasedResponse> {
        Box::pin(async move {
            let host = context.resource::<HostContext>().map_err(|_| harmos_sdk::ServingError::Catalog)?;
            let result = match &self.binding.target {
                Target::Plugin(_) => invoke(&self.binding, self.payload.clone(), &context, &host).await,
                Target::Clock(clock) => super::clock::run(clock, &self.binding.contract.id, &self.payload).await,
            };
            result
                .map(|bytes| crate::operations::Response::from(bytes).into_erased())
                .map_err(|error| harmos_sdk::ServingError::Native(Arc::new(error)))
        })
    }

    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn transferred(&self) -> bool {
        false
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        Vec::new()
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn invoke(
    binding: &Binding, payload: Vec<u8>, context: &harmos_sdk::ExecutionContext, host: &HostContext,
) -> Result<Vec<u8>, CallError> {
    let Target::Plugin(connection) = &binding.target else { unreachable!() };
    let closed = connection.closed.subscribe();
    let work = async {
        let (output, input) = mpsc::channel(1);
        let request = v1::Request {
            operation: binding.contract.id.clone(),
            payload,
            invocation: host.borrow().id().to_string(),
            attempt: host.borrow().attempt() as u32 + 1,
            scope: String::new(),
        };
        output
            .send(v1::HostFrame {
                frame: Some(host_frame::Frame::Request(request)),
            })
            .await
            .unwrap();
        let input = stream::unfold(input, |mut input| async { input.recv().await.map(|frame| (frame, input)) });
        let mut client = v1::plugin_client::PluginClient::new(connection.channel.clone());
        let mut input = client.invoke(input).await.map_err(CallError::Transport)?.into_inner();
        let mut children = futures_util::stream::FuturesUnordered::<
            futures_util::future::BoxFuture<'static, (u64, String, crate::Result<Vec<u8>>)>,
        >::new();
        let mut controls = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut cancelled = false;
        let mut cancelled_children = BTreeSet::new();
        enum Event {
            Cancel,
            Child(u64, String, crate::Result<Vec<u8>>),
            Frame(Result<Option<v1::PluginFrame>, tonic::Status>),
        }
        loop {
            let has_children = !children.is_empty();
            let next = async {
                tokio::select! {
                    biased;
                    () = context.cancellation.cancelled(), if !cancelled => Event::Cancel,
                    Some((call, operation, result)) = children.next(), if has_children => Event::Child(call, operation, result),
                    frame = input.message() => Event::Frame(frame),
                }
            };
            let event = if has_children { crate::clock::waiting(next).await } else { next.await };
            match event {
                Event::Cancel => {
                    cancelled = true;
                    let cancel = v1::Cancel {
                        reason: format!("Operation {} Cancelled", binding.contract.id),
                    };
                    let _ = output
                        .send(v1::HostFrame {
                            frame: Some(host_frame::Frame::Cancel(cancel)),
                        })
                        .await;
                }
                Event::Child(call, operation, result) => {
                    controls.remove(&call);
                    if cancelled_children.remove(&call) {
                        continue;
                    }
                    let result = match result {
                        Ok(payload) => v1::child_response::Result::Payload(payload),
                        Err(error) => v1::child_response::Result::Failure(failure(&error, &operation, "host")),
                    };
                    let child = v1::ChildResponse { call, result: Some(result) };
                    let _ = output
                        .send(v1::HostFrame {
                            frame: Some(host_frame::Frame::Child(child)),
                        })
                        .await;
                }
                Event::Frame(frame) => match frame.map_err(CallError::Transport)?.and_then(|frame| frame.frame) {
                    Some(plugin_frame::Frame::Response(response)) => {
                        if input.message().await.map_err(CallError::Transport)?.is_some() {
                            return Err(CallError::Protocol("Frame After Terminal Response".into()));
                        }
                        return match response.result {
                            Some(v1::response::Result::Payload(payload)) => Ok(payload),
                            Some(v1::response::Result::Failure(failure)) => Err(CallError::Remote(failure)),
                            None => Err(CallError::Protocol("Missing Response Result".into())),
                        };
                    }
                    Some(plugin_frame::Frame::Child(child)) => {
                        if !seen.insert(child.call) {
                            return Err(CallError::Protocol(format!("Duplicate Child ID {}", child.call)));
                        }
                        let call = child.call;
                        let operation = child.operation.clone();
                        let invocation = host.borrow().submit_grpc_child(&binding.owner, &binding.requires, child);
                        if let Ok(invocation) = &invocation {
                            controls.insert(call, invocation.control.clone());
                        }
                        children.push(Box::pin(async move {
                            let result = match invocation {
                                Ok(invocation) => invocation.await,
                                Err(error) => Err(error),
                            };
                            (call, operation, result)
                        }));
                    }
                    Some(plugin_frame::Frame::ChildCancel(child)) => {
                        if let Some(control) = controls.get(&child.call) {
                            cancelled_children.insert(child.call);
                            control.request_cancel();
                        } else if !seen.contains(&child.call) {
                            return Err(CallError::Protocol(format!("Unknown Child ID {}", child.call)));
                        }
                    }
                    None => return Err(CallError::Protocol("Invoke Ended Without Response".into())),
                },
            }
        }
    };
    tokio::select! {
        biased;
        _ = super::disconnected(closed) => Err(CallError::Transport(tonic::Status::unavailable("Plugin Connection Closed"))),
        result = work => result,
    }
}

pub(crate) fn failure(error: &crate::Error, operation: &str, plugin: &str) -> v1::Failure {
    if let crate::Error::OperationFailed { source, .. } = error {
        if let Some(CallError::Remote(failure)) = source.downcast_ref::<CallError>() {
            return failure.clone();
        }
        if let Some(error) = source.downcast_ref::<CallError>() {
            return v1::Failure {
                classification: error.classification() as i32,
                code: "transport".into(),
                message: error.to_string(),
                action: "Retry the Operation".into(),
                operation: operation.into(),
                plugin: plugin.into(),
                ..Default::default()
            };
        }
    }
    let classification = match error {
        crate::Error::NotRegistered { .. }
        | crate::Error::OperationUnauthorized { .. }
        | crate::Error::PermissionDenied
        | crate::Error::InvalidInput { .. } => v1::Classification::Refused,
        crate::Error::OperationRefused(crate::Refusal::Cancelled { .. }) => v1::Classification::Cancelled,
        crate::Error::OperationRefused(crate::Refusal::TimedOut { .. }) => v1::Classification::TimedOut,
        crate::Error::OperationRefused(crate::Refusal::Unavailable) => v1::Classification::Transient,
        _ => v1::Classification::Permanent,
    };
    v1::Failure {
        classification: classification as i32,
        code: "refused".into(),
        message: error.to_string(),
        action: "Check the Operation Contract".into(),
        operation: operation.into(),
        plugin: plugin.into(),
        ..Default::default()
    }
}

/* === Tests === */
