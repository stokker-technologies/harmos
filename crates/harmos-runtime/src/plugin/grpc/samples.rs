/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Interval Window: Grant, Window-Scoped Child Calls and the Columnar Answer

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::plugin::{SampleBatch, Window as WireWindow};

use super::{
    CallingPlugin, Error,
    invoke::{ChildReplies, failure, watch_child},
};
use crate::operations::HostContext;
use crate::streams::StreamWriter;
use crate::supervision::instance::Instance;
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use harmos_domain::MemorySize;
use harmos_operations::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_operations::{OperationPolicy, operations::UNBOUNDED};
use harmos_protocol::plugin::ChildResponse;
use harmos_protocol::plugin::InstanceSamplesRequest;
use harmos_protocol::plugin::InstanceSamplesResponse;
use harmos_protocol::plugin::child_response;
use harmos_protocol::plugin::instance_samples_request::Frame as RequestFrame;
use harmos_protocol::plugin::instance_samples_response::Frame as ResponseFrame;
use prost::Message;
use std::time::Duration;
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
type Control = (Arc<Instance>, bool, Option<mpsc::Sender<()>>);

/* === Structs === */

pub(crate) struct Window;
#[derive(Clone)]
pub(crate) struct Request {
    pub(super) input: Arc<AsyncMutex<tonic::Streaming<InstanceSamplesResponse>>>,
    pub(super) output: mpsc::Sender<InstanceSamplesRequest>,
    pub(super) caller: Arc<CallingPlugin>,
    pub(super) seen: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) window: WireWindow,
    pub(super) lease: Arc<StreamWriter>,
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
impl MemorySize for Request {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.caller.description.encoded_len())
    }
}
impl NativeOperation for Window {
    type Request = Request;
    type Response = Vec<SampleBatch>;
    type Endpoints = ();
    const ID: &'static str = "samples.window";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: UNBOUNDED,
        attempt_timeout: UNBOUNDED,
        cancellation_timeout: Duration::ZERO,
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

async fn run(context: &ExecutionContext, host: &HostContext, request: Request) -> Result<Vec<SampleBatch>, Error> {
    let mut children = FuturesUnordered::<Child>::new();
    let mut controls = Children(BTreeMap::new());
    let mut cancelled = BTreeSet::new();
    let mut ended = BTreeSet::new();
    let mut input = request.input.lock().await;
    let output = ChildReplies::Samples(request.output.clone());
    let granted = request.window.count;
    request
        .output
        .send(InstanceSamplesRequest {
            frame: Some(RequestFrame::Window(request.window)),
        })
        .await
        .map_err(|_| invalid("Grant Sample Window: stream closed"))?;
    let mut parts = Vec::new();
    let mut received = 0_u64;
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
            Event::Cancel => break Err(invalid("Answer Sample Window: window cancelled")),
            Event::Frame(frame) => {
                let Some(frame) = frame.map_err(|error| Error::Transport(error.to_string()))? else {
                    break Err(invalid("Answer Sample Window: stream ended"));
                };
                if !frame.samples.is_empty() {
                    break Err(invalid("Answer Sample Window: an interval stream answers with batches; send SampleBatch parts"));
                }
                let Some(frame) = frame.frame else {
                    break Err(invalid("Answer Sample Window: response has no frame"));
                };
                if let ResponseFrame::Batch(part) = frame {
                    if part.offset != received {
                        break Err(invalid("Answer Sample Window: a part must start where the previous part ended"));
                    }
                    let end = part.offset.checked_add(part.count).filter(|end| part.count > 0 && *end <= granted);
                    let Some(end) = end else {
                        break Err(invalid("Answer Sample Window: a part must hold at least one sample and stay within the window"));
                    };
                    if part.last != (end == granted) {
                        break Err(invalid("Answer Sample Window: only the part that completes the window sets last"));
                    }
                    received = end;
                    let last = part.last;
                    parts.push(part);
                    if last {
                        break Ok(std::mem::take(&mut parts));
                    }
                    continue;
                }
                if !request.lease.is_active() {
                    break Err(invalid("Call Sample Child: activation lease has been revoked"));
                }
                if let ResponseFrame::Child(child) = frame.clone() {
                    let call = child.call;
                    if !request.seen.lock().expect("sample child identity lock is healthy").insert(call) {
                        break Err(invalid("Call Sample Child: duplicate child identity across windows"));
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
                if let ResponseFrame::ChildCancel(child) = frame.clone() {
                    if let Some((control, _, _)) = controls.0.get(&child.call) {
                        cancelled.insert(child.call);
                        control.request_cancel();
                    } else if !request.seen.lock().expect("sample child identity lock is healthy").contains(&child.call) {
                        break Err(invalid("Cancel Sample Child: unknown child identity"));
                    }
                }
                if let ResponseFrame::ChildDemand(demand) = frame {
                    if ended.contains(&demand.call) {
                        continue;
                    }
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
                ended.insert(call);
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
    // Carrier sends may be backpressured after the peer has answered its window.
    // The operation scope awaits cancelled descendants without retaining those sends.
    drop(children);
    result
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/* === Tests === */
