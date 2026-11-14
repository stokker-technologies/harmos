/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Tick-Scoped Sample Child Calls

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{
    CallingPlugin,
    invoke::{ChildReplies, failure, watch_child},
};
use crate::operations::HostContext;
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use harmos_operations::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_protocol::{
    plugin::{InstanceSamplesRequest, InstanceSamplesResponse},
    v1::{self, ChildResponse, child_response},
};
use prost::Message;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, mpsc};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Child = BoxFuture<'static, (u64, String, crate::Result<Vec<u8>>)>;
type Control = (Arc<crate::supervision::instance::Instance>, bool, Option<mpsc::Sender<()>>);

/* === Structs === */

pub(crate) struct Tick;
#[derive(Clone)]
pub(crate) struct Request {
    pub(super) input: Arc<AsyncMutex<tonic::Streaming<InstanceSamplesResponse>>>,
    pub(super) output: mpsc::Sender<InstanceSamplesRequest>,
    pub(super) caller: Arc<CallingPlugin>,
    pub(super) seen: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) at: prost_types::Timestamp,
}
struct Children(BTreeMap<u64, Control>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Children {
    fn drop(&mut self) {
        for (control, _, _) in self.0.values() {
            control.request_cancel();
        }
    }
}
impl harmos_domain::MemorySize for Request {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.caller.description.encoded_len())
    }
}
impl NativeOperation for Tick {
    type Request = Request;
    type Response = InstanceSamplesResponse;
    type Endpoints = ();
    const ID: &'static str = "samples.tick";
    const POLICY: harmos_operations::OperationPolicy = harmos_operations::OperationPolicy {
        timeout: harmos_operations::operations::UNBOUNDED,
        attempt_timeout: harmos_operations::operations::UNBOUNDED,
        cancellation_timeout: std::time::Duration::ZERO,
        retries: crate::RetryPolicy::Never,
    };
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];
    async fn run(context: ExecutionContext, request: Request, _: &()) -> Result<Self::Response, ServingError> {
        let host = context.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
        run(&context, &host, request).await.map_err(|source| ServingError::Native(Arc::new(source)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn run(context: &ExecutionContext, host: &HostContext, request: Request) -> Result<InstanceSamplesResponse, super::Error> {
    let mut children = FuturesUnordered::<Child>::new();
    let mut controls = Children(BTreeMap::new());
    let mut cancelled = BTreeSet::new();
    let mut input = request.input.lock().await;
    let output = ChildReplies::Samples(request.output.clone());
    request
        .output
        .send(InstanceSamplesRequest {
            frame: Some(harmos_protocol::plugin::instance_samples_request::Frame::Tick(v1::Tick { at: Some(request.at) })),
        })
        .await
        .map_err(|_| invalid("Send Sample Tick: stream closed"))?;
    let result = loop {
        let has_children = !children.is_empty();
        enum Event {
            Cancel,
            Frame(Result<Option<InstanceSamplesResponse>, tonic::Status>),
            Child(u64, String, crate::Result<Vec<u8>>),
        }
        let waiting = controls.0.values().any(|(_, background, _)| !background);
        let next = async {
            tokio::select! {
                biased;
                () = context.cancellation.cancelled() => Event::Cancel,
                Some((call, operation, result)) = children.next(), if has_children => Event::Child(call, operation, result),
                frame = input.message() => Event::Frame(frame),
            }
        };
        let event = if waiting { crate::clock::waiting(next).await } else { next.await };
        match event {
            Event::Cancel => break Err(invalid("Answer Sample Tick: tick cancelled")),
            Event::Frame(frame) => {
                let Some(frame) = frame.map_err(|error| super::Error::Transport(error.to_string()))? else {
                    break Err(invalid("Answer Sample Tick: stream ended"));
                };
                if !frame.samples.is_empty() {
                    if frame.frame.is_some() {
                        break Err(invalid("Answer Sample Tick: sample and child traffic must be separate"));
                    }
                    break Ok(frame);
                }
                let Some(frame) = frame.frame else {
                    break Err(invalid("Answer Sample Tick: child-only response has no frame"));
                };
                if let harmos_protocol::plugin::instance_samples_response::Frame::Child(child) = frame.clone() {
                    let call = child.call;
                    if !request.seen.lock().expect("sample child identity lock is healthy").insert(call) {
                        break Err(invalid("Call Sample Child: duplicate child identity across ticks"));
                    }
                    let operation = child.operation.clone();
                    let background = child.background;
                    let invocation = host.borrow().submit_grpc_child(&request.caller, child);
                    let (demand, credits) = mpsc::channel(1);
                    if let Ok(child) = &invocation {
                        controls.0.insert(call, (child.invocation.control.clone(), background, child.streams.as_ref().map(|_| demand)));
                    }
                    let output = output.clone();
                    children.push(Box::pin(async move {
                        let result = match invocation {
                            Ok(child) => match child.streams {
                                Some(streams) => watch_child(call, child.invocation, streams, credits, output).await,
                                None => child.invocation.await,
                            },
                            Err(error) => Err(error),
                        };
                        (call, operation, result)
                    }));
                }
                if let harmos_protocol::plugin::instance_samples_response::Frame::ChildCancel(child) = frame.clone() {
                    if let Some((control, _, _)) = controls.0.get(&child.call) {
                        cancelled.insert(child.call);
                        control.request_cancel();
                    } else if !request.seen.lock().expect("sample child identity lock is healthy").contains(&child.call) {
                        break Err(invalid("Cancel Sample Child: unknown child identity"));
                    }
                }
                if let harmos_protocol::plugin::instance_samples_response::Frame::ChildDemand(demand) = frame {
                    let Some((_, _, Some(credits))) = controls.0.get(&demand.call) else {
                        break Err(invalid("Demand Sample Child: unknown watch identity"));
                    };
                    if credits.try_send(()).is_err() {
                        break Err(invalid("Demand Sample Child: duplicate demand"));
                    }
                }
            }
            Event::Child(call, operation, result) => {
                controls.0.remove(&call);
                if cancelled.remove(&call) {
                    continue;
                }
                let result = match result {
                    Ok(payload) => child_response::Result::Payload(payload),
                    Err(error) => child_response::Result::Failure(failure(&error, &operation, "host")),
                };
                output.send(ChildResponse { call, result: Some(result) }).await.map_err(|error| invalid(error.to_string()))?;
            }
        }
    };
    drop(controls);
    while children.next().await.is_some() {}
    result
}
fn invalid(message: impl Into<String>) -> super::Error {
    super::Error::Invalid(message.into())
}

/* === Tests === */
