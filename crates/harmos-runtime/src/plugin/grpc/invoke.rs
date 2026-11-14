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
use harmos_protocol::plugin::{instance_invoke_request, instance_invoke_response};
use harmos_protocol::v1::{self};
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
    pub(crate) contract: v1::OperationContract,
    pub(crate) caller: super::CallingPlugin,
    connection: Arc<Connection>,
}

struct Invocation {
    binding: Binding,
    payload: Vec<u8>,
}

/* === Enums === */

#[derive(Clone)]
pub(super) enum ChildReplies {
    Invoke(mpsc::Sender<harmos_protocol::plugin::InstanceInvokeRequest>),
    Samples(mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>),
}

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("{0}")]
    Remote(#[source] Box<harmos_operations::Failure>),

    #[error("plugin transport failed: {0}")]
    Transport(tonic::Status),

    #[error("plugin protocol failed: {0}")]
    Protocol(String),
}

/* === Traits === */

/* === Implementations === */

impl ChildReplies {
    pub(super) async fn send(&self, child: v1::ChildResponse) -> crate::Result<()> {
        let result = match self {
            Self::Invoke(output) => output
                .send(harmos_protocol::plugin::InstanceInvokeRequest {
                    frame: Some(instance_invoke_request::Frame::Child(child)),
                })
                .await
                .map_err(|_| ()),
            Self::Samples(output) => output
                .send(harmos_protocol::plugin::InstanceSamplesRequest {
                    frame: Some(harmos_protocol::plugin::instance_samples_request::Frame::Child(child)),
                })
                .await
                .map_err(|_| ()),
        };
        result.map_err(|()| crate::Error::Closing { domain: "child response carrier" })
    }
}

impl Binding {
    pub(crate) fn new(activation: &Activation, contract: v1::OperationContract) -> Self {
        Self {
            contract,
            caller: super::CallingPlugin {
                name: activation.entry_name.clone(),
                description: activation.description.clone(),
            },
            connection: activation.connection.clone(),
        }
    }

    pub(crate) fn prepare(&self, payload: Vec<u8>) -> Box<dyn NativeInstance> {
        Box::new(Invocation { binding: self.clone(), payload })
    }
}

impl CallError {
    pub fn classification(&self) -> v1::Classification {
        match self {
            Self::Remote(failure) => v1::Failure::from(failure.as_ref()).classification(),
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

    fn initialize(&mut self, _: harmos_operations::execution::ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn run(&mut self, context: harmos_operations::execution::ExecutionContext) -> ServiceFuture<'_, ErasedResponse> {
        Box::pin(async move {
            let host = context.resource::<HostContext>().map_err(|_| harmos_operations::execution::ServingError::Catalog)?;
            let result = invoke(&self.binding, self.payload.clone(), &context, &host).await;
            result
                .map(|bytes| crate::operations::Response::from(bytes).into_erased())
                .map_err(|error| harmos_operations::execution::ServingError::Native(Arc::new(error)))
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
    binding: &Binding, payload: Vec<u8>, context: &harmos_operations::execution::ExecutionContext, host: &HostContext,
) -> Result<Vec<u8>, CallError> {
    let connection = &binding.connection;
    let closed = connection.closed.subscribe();
    let work = async {
        let (output, input) = mpsc::channel(1);
        let request = v1::Request {
            operation: binding.contract.id.clone(),
            payload,
            invocation: host.borrow().id().to_string(),
            attempt: host.borrow().attempt() as u32 + 1,
            scope: host.borrow().context().get::<crate::Scope>().map(|scope| scope.as_str().to_owned()).unwrap_or_default(),
        };
        output
            .send(harmos_protocol::plugin::InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Request(request)),
            })
            .await
            .unwrap();
        let input = stream::unfold(input, |mut input| async { input.recv().await.map(|frame| (frame, input)) });
        let mut client = connection.client();
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
            Frame(Result<Option<harmos_protocol::plugin::InstanceInvokeResponse>, tonic::Status>),
        }
        loop {
            let has_children = !children.is_empty();
            let waiting = controls
                .values()
                .any(|(_, background, _): &(Arc<crate::supervision::instance::Instance>, bool, Option<mpsc::Sender<()>>)| !background);
            let next = async {
                tokio::select! {
                    biased;
                    () = context.cancellation.cancelled(), if !cancelled => Event::Cancel,
                    Some((call, operation, result)) = children.next(), if has_children => Event::Child(call, operation, result),
                    frame = input.message() => Event::Frame(frame),
                }
            };
            let event = if waiting { crate::clock::waiting(next).await } else { next.await };
            match event {
                Event::Cancel => {
                    cancelled = true;
                    let cancel = v1::Cancel {
                        reason: format!("Operation {} Cancelled", binding.contract.id),
                    };
                    let _ = output
                        .send(harmos_protocol::plugin::InstanceInvokeRequest {
                            frame: Some(instance_invoke_request::Frame::Cancel(cancel)),
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
                        .send(harmos_protocol::plugin::InstanceInvokeRequest {
                            frame: Some(instance_invoke_request::Frame::Child(child)),
                        })
                        .await;
                }
                Event::Frame(frame) => match frame.map_err(CallError::Transport)?.and_then(|frame| frame.frame) {
                    Some(instance_invoke_response::Frame::Response(response)) => {
                        if input.message().await.map_err(CallError::Transport)?.is_some() {
                            return Err(CallError::Protocol("Frame After Terminal Response".into()));
                        }
                        return match response.result {
                            Some(v1::response::Result::Payload(payload)) => Ok(payload),
                            Some(v1::response::Result::Failure(failure)) => Err(CallError::Remote(Box::new(failure.into()))),
                            None => Err(CallError::Protocol("Missing Response Result".into())),
                        };
                    }
                    Some(instance_invoke_response::Frame::Child(child)) => {
                        if !seen.insert(child.call) {
                            return Err(CallError::Protocol(format!("Duplicate Child ID {}", child.call)));
                        }
                        let call = child.call;
                        let operation = child.operation.clone();
                        let background = child.background;
                        let invocation = host.borrow().submit_grpc_child(&binding.caller, child);
                        let (demand, credits) = mpsc::channel(1);
                        if let Ok(child) = &invocation {
                            controls.insert(call, (child.invocation.control.clone(), background, child.streams.as_ref().map(|_| demand)));
                        }
                        let output = output.clone();
                        children.push(Box::pin(async move {
                            let result = match invocation {
                                Ok(child) => match child.streams {
                                    Some(streams) => {
                                        watch_child(call, child.invocation, streams, credits, ChildReplies::Invoke(output)).await
                                    }
                                    None => child.invocation.await,
                                },
                                Err(error) => Err(error),
                            };
                            (call, operation, result)
                        }));
                    }
                    Some(instance_invoke_response::Frame::ChildCancel(child)) => {
                        if let Some((control, _, _)) = controls.get(&child.call) {
                            cancelled_children.insert(child.call);
                            control.request_cancel();
                        } else if !seen.contains(&child.call) {
                            return Err(CallError::Protocol(format!("Unknown Child ID {}", child.call)));
                        }
                    }
                    Some(instance_invoke_response::Frame::ChildDemand(demand)) => {
                        let Some((_, _, Some(credits))) = controls.get(&demand.call) else {
                            return Err(CallError::Protocol(format!("Watch {}: Unknown Child Demand", demand.call)));
                        };
                        credits.try_send(()).map_err(|_| CallError::Protocol(format!("Watch {}: Duplicate Demand", demand.call)))?;
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

pub(super) async fn watch_child(
    call: u64, mut invocation: crate::operations::EncodedInvocation, streams: harmos_operations::execution::operations::WireStreams,
    mut credits: mpsc::Receiver<()>, output: ChildReplies,
) -> crate::Result<Vec<u8>> {
    let mut response = None;
    loop {
        // Completion Is Observed Even While the Consumer Is Not Requesting an Item.
        tokio::select! {
            result = &mut invocation, if response.is_none() => { response = Some(result?); continue; }
            credit = credits.recv() => if credit.is_none() { invocation.request_cancel(); return invocation.await; }
        }
        let item = loop {
            tokio::select! {
                result = &mut invocation, if response.is_none() => response = Some(result?),
                item = streams.receive_current() => break item.map_err(|source| crate::Error::OperationFailed {
                    operation: "Watch child".into(), source: Arc::new(source),
                })?,
            }
        };
        let Some(payload) = item else {
            return match response {
                Some(response) => Ok(response),
                None => invocation.await,
            };
        };
        output
            .send(v1::ChildResponse {
                call,
                result: Some(v1::child_response::Result::Item(payload)),
            })
            .await?;
    }
}

pub(crate) fn failure(error: &crate::Error, operation: &str, plugin: &str) -> v1::Failure {
    if let crate::Error::OperationFailed { source, .. } = error {
        if let Some(CallError::Remote(failure)) = source.downcast_ref::<CallError>() {
            return v1::Failure::from(failure.as_ref());
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
    if let Some(failure) = error.failure() {
        return v1::Failure::from(failure);
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
