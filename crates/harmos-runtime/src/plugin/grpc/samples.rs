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

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use prost::Message;
use tokio::sync::{self, mpsc};
use tonic::{Status, Streaming};

// Local Imports
use super::{
    CallingPlugin, Error,
    invoke::{ChildReplies, failure, watch_child},
};
use crate::{RetryPolicy, operations::HostContext, streams::StreamWriter, supervision::instance::Instance};
use harmos_domain::{MemorySize, message};
use harmos_operations::clock::WindDown;
use harmos_operations::{
    OperationPolicy,
    execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError},
    operations::UNBOUNDED,
};
use harmos_protocol::plugin::{
    self, ChildResponse, CollectionBatch, InstanceSamplesRequest, InstanceSamplesResponse, SampleBatch, child_response,
    instance_samples_request, instance_samples_response,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Child = BoxFuture<'static, (u64, String, crate::Result<Vec<u8>>)>;
type Control = (Arc<Instance>, bool, Option<mpsc::Sender<()>>);

/* === Structs === */

pub(crate) struct Window;
#[derive(Clone)]
pub(crate) struct Request {
    pub(super) input: Arc<sync::Mutex<Streaming<InstanceSamplesResponse>>>,
    pub(super) output: mpsc::Sender<InstanceSamplesRequest>,
    pub(super) caller: Arc<CallingPlugin>,
    pub(super) seen: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) window: plugin::Window,
    pub(super) collection: BTreeSet<String>,
    pub(super) lease: Arc<StreamWriter>,
}
struct Children(BTreeMap<u64, Control>);

/* === Enums === */

pub(crate) enum Answer {
    Columns(Vec<SampleBatch>),
    Rows(Vec<CollectionBatch>),
}

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
        let members = message::btree_map_memory::<String, ()>(self.collection.len()).saturating_sub(mem::size_of::<BTreeSet<String>>());
        let captures = self
            .window
            .capturing_executions
            .iter()
            .fold(self.window.capturing_executions.capacity().saturating_mul(mem::size_of::<String>()), |bytes, execution| {
                bytes.saturating_add(execution.capacity())
            });
        let bytes =
            mem::size_of::<Self>().saturating_add(self.caller.description.encoded_len()).saturating_add(members).saturating_add(captures);
        self.collection.iter().fold(bytes, |bytes, member| bytes.saturating_add(member.capacity()))
    }
}
impl NativeOperation for Window {
    type Request = Request;
    type Response = Answer;
    type Endpoints = ();
    const ID: &'static str = "samples.window";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: UNBOUNDED,
        attempt_timeout: UNBOUNDED,
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];
    async fn run(context: ExecutionContext, request: Request, _: &()) -> Result<Self::Response, ServingError> {
        let host = context.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
        run(&context, &host, request).await.map_err(|source| ServingError::Native(Arc::new(source)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn run(context: &ExecutionContext, host: &HostContext, request: Request) -> Result<Answer, Error> {
    let mut children = FuturesUnordered::<Child>::new();
    let mut controls = Children(BTreeMap::new());
    let mut cancelled = BTreeSet::new();
    let mut ended = BTreeSet::new();
    let mut input = request.input.lock().await;
    let output = ChildReplies::Samples(request.output.clone());
    let granted = request.window.count;
    let stream = request.lease.stream().definition().id().to_owned();
    request
        .output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Window(request.window)),
        })
        .await
        .map_err(|_| invalid("Grant Sample Window: stream closed"))?;
    let mut parts = Vec::new();
    let mut ticks = Vec::new();
    let mut staged_rows = 0_u64;
    let mut staged_bytes = 0_u128;
    let mut received = 0_u64;
    let wind_down = host.borrow().context().get::<WindDown>().cloned().unwrap_or_default();
    let mut winding_down = false;
    let result = loop {
        let has_children = !children.is_empty();
        enum Event {
            Cancel,
            WindDown,
            Frame(Result<Option<InstanceSamplesResponse>, Status>),
            Child(u64, String, crate::Result<Vec<u8>>),
        }
        let waiting = controls.0.values().any(|(_, background, _)| !background);
        let next = async {
            tokio::select! {
                biased;
                () = context.cancellation.cancelled() => Event::Cancel,
                () = wind_down.requested(), if !winding_down => Event::WindDown,
                Some((call, operation, result)) = children.next(), if has_children => Event::Child(call, operation, result),
                frame = input.message() => Event::Frame(frame),
            }
        };
        let event = if waiting { crate::clock::waiting(next).await } else { next.await };
        match event {
            Event::WindDown => {
                winding_down = true;
                request
                    .output
                    .send(InstanceSamplesRequest {
                        frame: Some(instance_samples_request::Frame::WindDown(true)),
                    })
                    .await
                    .map_err(|_| invalid("Wind Down Sample Window: stream closed"))?;
            }
            Event::Cancel => break Err(invalid("Answer Sample Window: window cancelled")),
            Event::Frame(frame) => {
                // The Plugin's Status Fails This Stream's Window; Connection Loss Is Observed by the Connection Itself.
                let failed = |status: Status| Error::WindowFailed {
                    stream: stream.clone(),
                    status: status.code(),
                    message: status.message().to_owned(),
                };
                let Some(frame) = frame.map_err(failed)? else {
                    break Err(invalid("Answer Sample Window: stream ended"));
                };
                if !frame.samples.is_empty() {
                    break Err(invalid("Answer Sample Window: an interval stream answers with batches; send SampleBatch parts"));
                }
                let Some(frame) = frame.frame else {
                    break Err(invalid("Answer Sample Window: response has no frame"));
                };
                if let instance_samples_response::Frame::Collection(part) = frame {
                    if request.collection.is_empty()
                        || part.offset != received
                        || received >= granted
                        || part.last != (received + 1 == granted)
                    {
                        break Err(invalid(
                            "Answer Collection Window: send one consecutive batch per tick and mark only the final tick last",
                        ));
                    }
                    let mut members = BTreeSet::new();
                    for rows in &part.streams {
                        if !request.collection.contains(&rows.stream) || !members.insert(&rows.stream) {
                            return Err(invalid("Answer Collection Window: publish each declared member at most once per tick batch"));
                        }
                        staged_rows = staged_rows.saturating_add(rows.rows.len() as u64);
                    }
                    // Empty Ticks Also Consume Staging Space Until the Whole Window Succeeds.
                    let bytes = part.streams.iter().fold(
                        part.streams.capacity() as u128 * mem::size_of::<plugin::CollectionRows>() as u128,
                        |bytes, group| {
                            let headers =
                                group.stream.capacity() as u128 + group.rows.capacity() as u128 * mem::size_of::<Vec<u8>>() as u128;
                            group.rows.iter().fold(bytes.saturating_add(headers), |bytes, row| bytes.saturating_add(row.capacity() as u128))
                        },
                    );
                    staged_bytes = staged_bytes.saturating_add(bytes);
                    let (_, fits) = crate::streams::claim(staged_rows.max(ticks.len() as u64 + 1), staged_bytes);
                    if !fits {
                        break Err(invalid(
                            "Answer Collection Window: reduce published rows or the configured window to fit the publication pool",
                        ));
                    }
                    received += 1;
                    let last = part.last;
                    ticks.push(part);
                    if last {
                        break Ok(Answer::Rows(mem::take(&mut ticks)));
                    }
                    continue;
                }
                if let instance_samples_response::Frame::Batch(part) = frame {
                    if !request.collection.is_empty() {
                        break Err(invalid("Answer Collection Window: send collection tick batches instead of dense columns"));
                    }
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
                        break Ok(Answer::Columns(mem::take(&mut parts)));
                    }
                    continue;
                }
                if !request.lease.is_active() {
                    break Err(invalid("Call Sample Child: activation lease has been revoked"));
                }
                if let instance_samples_response::Frame::Child(child) = frame.clone() {
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
                if let instance_samples_response::Frame::ChildCancel(child) = frame.clone() {
                    if let Some((control, _, _)) = controls.0.get(&child.call) {
                        cancelled.insert(child.call);
                        control.request_cancel();
                    } else if !request.seen.lock().expect("sample child identity lock is healthy").contains(&child.call) {
                        break Err(invalid("Cancel Sample Child: unknown child identity"));
                    }
                }
                if let instance_samples_response::Frame::ChildDemand(demand) = frame {
                    if ended.contains(&demand.call) {
                        continue;
                    }
                    let Some((_, _, Some(credits))) = controls.0.get(&demand.call) else {
                        break Err(invalid("Demand Sample Child: unknown watch identity"));
                    };
                    if credits.try_send(()).is_err() {
                        break Err(invalid("Demand Sample Child: duplicate demand"));
                    }
                } else if matches!(frame, instance_samples_response::Frame::Demand(_)) {
                    break Err(invalid("Answer Sample Window: change collection demand only between windows"));
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
