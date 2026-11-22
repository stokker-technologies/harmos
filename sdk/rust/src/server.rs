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

#[cfg(test)]
mod tests;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    error,
    future::{self, Future},
    net,
    pin::{Pin, pin},
    result,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    task::{self, Poll},
    time::SystemTime,
};

// Third Party Imports
use futures_core::Stream;
use futures_util::{future::poll_fn, stream};
use prost::Message;
use prost_types::Timestamp;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
};
use tonic::{Request, Response, Status, Streaming, transport};

// Local Imports
use crate::{
    Context, Error, Plugin, Result, columns,
    context::ChildOutput,
    operation::Handler,
    stream::{Lifecycle, Observer},
};
use harmos_protocol::{
    model::Declarations,
    plugin::{
        self, InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
        InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse, Sample, instance_invoke_request, instance_invoke_response,
        instance_samples_request,
        instance_server::{self, InstanceServer},
        response,
    },
    transfer,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = result::Result<T, Status>> + Send>>;

/* === Structs === */

struct Completion {
    result: oneshot::Receiver<Result<Vec<u8>>>,
    settled: oneshot::Receiver<()>,
}

struct Settled(Option<oneshot::Sender<()>>);

struct Server {
    description: InstanceGetResponse,
    failure_mapper: Option<crate::error::FailureMapper>,
    operations: HashMap<String, Arc<dyn Handler>>,
    streams: HashMap<String, Publication>,
    requires: Arc<HashMap<String, Vec<u8>>>,
    jobs: mpsc::Sender<Job>,
    cleanup: Arc<Cleanup>,
    shutdown: Option<watch::Sender<bool>>,

    /// The Transport's Maximum Message Size; Window Parts Are Split to Fit It.
    message_bytes: usize,
}

pub(crate) struct Cleanup {
    pub(crate) hooks: Vec<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    pub(crate) result: OnceLock<result::Result<(), String>>,
}

struct Publication {
    observer: Option<Observer>,
    producer: Option<Arc<dyn Handler>>,
}

struct Job {
    handler: Arc<dyn Handler>,
    context: Context,
    payload: Vec<u8>,
    started: oneshot::Sender<()>,
    complete: oneshot::Sender<Result<Vec<u8>>>,
    settled: oneshot::Sender<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Cleanup {
    pub(crate) fn run(&self) -> Result<()> {
        let result = self.result.get_or_init(|| {
            let mut failure = None;
            for hook in &self.hooks {
                if let Err(error) = hook() {
                    failure.get_or_insert_with(|| error.to_string());
                }
            }
            failure.map_or(Ok(()), Err)
        });
        result.clone().map_err(Error::Invalid)
    }
}

impl Future for Completion {
    type Output = result::Result<Result<Vec<u8>>, oneshot::error::RecvError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.result).poll(cx)
    }
}

impl Completion {
    async fn cancel(mut self) {
        self.result.close();
        let _ = (&mut self.settled).await;
    }
}

impl Drop for Settled {
    fn drop(&mut self) {
        if let Some(settled) = self.0.take() {
            let _ = settled.send(());
        }
    }
}

#[tonic::async_trait]
impl instance_server::Instance for Server {
    async fn get(&self, request: Request<InstanceGetRequest>) -> result::Result<Response<InstanceGetResponse>, Status> {
        if request.into_inner().revision == 0 {
            return Err(Status::invalid_argument("Revision Must Be at Least One"));
        }
        Ok(Response::new(self.description.clone()))
    }

    type InvokeStream = Output<InstanceInvokeResponse>;

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> result::Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let Some(instance_invoke_request::Frame::Request(request)) = input.message().await?.and_then(|frame| frame.frame) else {
            return Err(Status::invalid_argument("Request Must Be First"));
        };
        if request.invocation.is_empty() || request.attempt == 0 {
            return Err(Status::invalid_argument("Invocation and Attempt Are Required"));
        }
        let handler = self.operations.get(&request.operation).ok_or_else(|| Status::not_found("Unknown Operation"))?.clone();
        handler.validate(&request.payload)?;
        let payload = request.payload.clone();
        let mut context = Context::new(request, self.description.id.clone(), self.requires.clone());
        context.failure_mapper = self.failure_mapper.clone();
        let (output, receiver) = mpsc::channel(1);
        context.output = Some(ChildOutput::Invoke(output.clone()));
        let jobs = self.jobs.clone();
        tokio::spawn(async move {
            let result = invoke(jobs, handler, context, payload, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(output_stream(receiver)))
    }

    type SamplesStream = Output<InstanceSamplesResponse>;

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> result::Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let Some(instance_samples_request::Frame::Open(open)) = input.message().await?.and_then(|frame| frame.frame) else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        let publication = self.streams.get(&open.stream).ok_or_else(|| Status::not_found("Unknown Stream"))?;
        let observer = publication.observer.clone();
        let producer = publication.producer.clone();
        let message_bytes = self.message_bytes;
        let mut context = Context::new(
            plugin::Request {
                scope: open.scope,
                operation: open.stream,
                ..Default::default()
            },
            self.description.id.clone(),
            self.requires.clone(),
        );
        context.failure_mapper = self.failure_mapper.clone();
        let (output, receiver) = mpsc::channel(2);
        let jobs = self.jobs.clone();
        tokio::spawn(async move {
            let result = samples(jobs, observer, producer, context, &mut input, &output, message_bytes).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(output_stream(receiver)))
    }

    async fn stop(&self, _: Request<InstanceStopRequest>) -> result::Result<Response<InstanceStopResponse>, Status> {
        self.cleanup.run().map_err(|error| Status::internal(error.to_string()))?;
        tracing::info!(plugin = %self.description.id, "Plugin Cleanup Completed");
        if let Some(shutdown) = &self.shutdown {
            shutdown.send_replace(true);
        }
        Ok(Response::new(InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub(crate) async fn run(
    plugin: Plugin, declarations: Declarations, listener: net::TcpListener, shutdown: watch::Sender<bool>,
    mut stopped: watch::Receiver<bool>, ready: oneshot::Sender<()>, cleanup: Arc<Cleanup>,
) -> Result<()> {
    let started_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|error| Error::Invalid(error.to_string()))?;
    transfer::message_bytes(&plugin.transfer).filter(|bytes| bytes.get() <= u32::MAX as usize).ok_or_else(|| {
        Error::Invalid("Start Plugin: MessageSettings.message_bytes cannot carry StorageTransferSettings.chunk_bytes plus framing".into())
    })?;
    let message_bytes = transfer::default_message_bytes().get();
    let listener = TcpListener::from_std(listener)?;
    let (jobs, receiver) = mpsc::channel(1);
    let executor = tokio::task::spawn_local(execute(receiver));
    let requires = Plugin::requirements(&declarations);
    let streams = plugin
        .streams
        .into_iter()
        .map(|stream| {
            // Declarations Refused Every Observer Without Columns Before Serving.
            let observer = plugin.observers.iter().find(|observer| observer.stream == stream.declaration.id).cloned();
            (stream.declaration.id, Publication { observer, producer: stream.producer })
        })
        .collect();
    let stopping = cleanup.clone();
    let server = Server {
        failure_mapper: plugin.failure_mapper,
        description: InstanceGetResponse {
            id: plugin.id,
            version: plugin.version,
            started_at: Some(Timestamp {
                seconds: started_at.as_secs() as i64,
                nanos: started_at.subsec_nanos() as i32,
            }),
            declarations: Some(declarations),
            revision: 1,
            sdk: "harmos_rust".into(),
            sdk_version: env!("CARGO_PKG_VERSION").into(),
            message_bytes: message_bytes as u64,
        },
        operations: plugin.operations.into_iter().map(|operation| (operation.contract.id, operation.handler)).collect(),
        streams,
        requires,
        message_bytes,
        jobs,
        cleanup,
        shutdown: (!plugin.remote).then_some(shutdown),
    };
    let incoming = stream::unfold(listener, |listener| async {
        let accepted = listener.accept().await.map(|(socket, _)| socket);
        Some((accepted, listener))
    });
    let _ = ready.send(());
    let result = transport::Server::builder()
        .add_service(InstanceServer::new(server).max_decoding_message_size(message_bytes).max_encoding_message_size(message_bytes))
        .serve_with_incoming_shutdown(incoming, async move {
            let _ = stopped.wait_for(|stopped| *stopped).await;
            // Stop Hooks Precede Graceful Drain on Both Local and Protocol Shutdown.
            let _ = stopping.run();
        })
        .await;
    executor.abort();
    let _ = executor.await;
    result?;
    Ok(())
}

/* === Private Functions === */

fn output_stream<T: Send + 'static>(receiver: mpsc::Receiver<result::Result<T, Status>>) -> Output<T> {
    Box::pin(stream::unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|item| (item, receiver)) }))
}

// A Local Executor Accepts Ordinary Borrowing Async Functions Without Author-Side Boxing.
async fn execute(mut jobs: mpsc::Receiver<Job>) {
    while let Some(job) = jobs.recv().await {
        tokio::task::spawn_local(async move {
            let Job {
                handler,
                context,
                payload,
                started,
                mut complete,
                settled,
            } = job;
            let _settled = Settled(Some(settled));
            if complete.is_closed() {
                return;
            }
            let result = {
                let future = handler.execute(&context, payload);
                let mut future = pin!(future);
                let first = poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await;
                let _ = started.send(());
                match first {
                    Poll::Ready(result) => result,
                    Poll::Pending => tokio::select! {
                        _ = complete.closed() => return,
                        result = &mut future => result,
                    },
                }
            };
            let result = result.map_err(|error| {
                if matches!(&error, Error::Failure(_)) {
                    return error;
                }
                let source: &(dyn error::Error + 'static) = match &error {
                    Error::Other(source) => source.as_ref(),
                    _ => &error,
                };
                context.failure_mapper.as_ref().and_then(|mapper| mapper(source)).unwrap_or(error)
            });
            let _ = complete.send(result);
        });
    }
}

async fn dispatch(
    jobs: &mpsc::Sender<Job>, handler: Arc<dyn Handler>, context: Context, payload: Vec<u8>,
) -> result::Result<Completion, Status> {
    let (complete, receiver) = oneshot::channel();
    let (started, ready) = oneshot::channel();
    let (settled, settlement) = oneshot::channel();
    jobs.send(Job {
        handler,
        context,
        payload,
        complete,
        started,
        settled,
    })
    .await
    .map_err(|_| Status::unavailable("Plugin Stopped"))?;
    ready.await.map_err(|_| Status::internal("Handler Panicked"))?;
    Ok(Completion { result: receiver, settled: settlement })
}

async fn invoke(
    jobs: mpsc::Sender<Job>, handler: Arc<dyn Handler>, context: Context, payload: Vec<u8>, input: &mut Streaming<InstanceInvokeRequest>,
    output: &mpsc::Sender<result::Result<InstanceInvokeResponse, Status>>,
) -> result::Result<(), Status> {
    let mut complete = dispatch(&jobs, handler, context.clone(), payload).await?;
    let mut eof = false;
    let result = loop {
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            _ = context.call_changes.notified() => context.flush_cancels().await.map_err(|error| Status::unavailable(error.to_string()))?,
            result = &mut complete => break result.map_err(|_| Status::internal("Handler Panicked"))?,
            frame = input.message(), if !eof => match frame? {
                Some(InstanceInvokeRequest { frame: Some(instance_invoke_request::Frame::Child(child)) }) => context.reply(child).await?,
                Some(InstanceInvokeRequest { frame: Some(instance_invoke_request::Frame::Cancel(cancel)) }) => context.cancel(cancel.reason).await.map_err(|error| Status::unavailable(error.to_string()))?,
                Some(InstanceInvokeRequest { frame: Some(instance_invoke_request::Frame::WindDown(true)) }) => context.request_wind_down(),
                None => { eof = true; if !context.calls.lock().expect("Child Calls Poisoned").pending.is_empty() { return Err(Status::unavailable("Host Closed Before Child Replies")); } },
                _ => return Err(Status::invalid_argument("Repeated Request")),
            }
        }
    };
    context.cancel_calls("Invocation Settled").await.map_err(|error| Status::unavailable(error.to_string()))?;
    let result = match result {
        Ok(payload) => response::Result::Payload(payload),
        Err(error) => error.response(&context.operation, &context.plugin)?,
    };
    output
        .send(Ok(InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Response(plugin::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn samples(
    jobs: mpsc::Sender<Job>, observer: Option<Observer>, producer: Option<Arc<dyn Handler>>, mut context: Context,
    input: &mut Streaming<InstanceSamplesRequest>, output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>,
    message_bytes: usize,
) -> result::Result<(), Status> {
    let (samples, mut rows) = mpsc::channel(1);
    context.samples = Some(samples);
    let mut producing = match producer {
        Some(producer) => Some(dispatch(&jobs, producer, context.clone(), Vec::new()).await?),
        None => None,
    };
    let mut observing: Option<(u64, Completion)> = None;
    let mut sample = None;
    // Parts of the Answered Window Still to Send, in Order.
    let mut parts = VecDeque::new();
    let mut sequence = 0;
    let collection = observer.as_ref().is_some_and(|observer| observer.columns.is_none());
    let mut active = false;
    let result = async {
    loop {
        // A Final Tick Answers the Window Only After Its Handler Has Completed Successfully.
        let can_send_part = parts.front().is_some_and(|part: &InstanceSamplesResponse| {
            observing.is_none() || !matches!(&part.frame, Some(plugin::instance_samples_response::Frame::Collection(batch)) if batch.last)
        });
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            _ = context.call_changes.notified(), if observing.is_some() => {
                context.flush_cancels().await.map_err(|error| Status::unavailable(error.to_string()))?;
            }
            frame = input.message() => {
                let Some(frame) = frame? else { return Ok(()); };
                match frame.frame {
                    Some(instance_samples_request::Frame::Demand(demand)) if observer.is_some() => {
                        let observer = observer.as_ref().expect("sampling observer");
                        let members = observer.members.lock().expect("collection members lock is healthy").clone();
                        let mut selected = BTreeSet::new();
                        for stream in &demand.streams {
                            let declared = if collection { members.contains_key(stream) } else { stream == &observer.stream };
                            if !declared || !selected.insert(stream) {
                                return Err(Status::invalid_argument("Change Collection Demand: select each declared member at most once"));
                            }
                        }
                        let wanted = !demand.streams.is_empty();
                        if (observing.is_some() || !parts.is_empty()) && wanted {
                            return Err(Status::invalid_argument("Change Collection Demand: wait for the current window to finish"));
                        }
                        if let Some((_, complete)) = observing.take() {
                            if let Some(ChildOutput::Samples { active, .. }) = &context.output { active.store(false, Ordering::Release); }
                            context.cancellation.send_replace(Some("Collection Demand Ended".into()));
                            context.cancel_calls("Collection Demand Ended").await.map_err(|error| Status::unavailable(error.to_string()))?;
                            complete.cancel().await;
                        }
                        parts.clear();
                        while rows.try_recv().is_ok() {}
                        if active != wanted {
                            if !wanted { active = false; }
                            transition(&jobs, &observer.lifecycle, &context, wanted, Some(&mut *input), output).await?;
                            active = wanted;
                        }
                        output.send(Ok(InstanceSamplesResponse {
                            samples: Vec::new(), frame: Some(plugin::instance_samples_response::Frame::Demand(plugin::CollectionDemandResponse {})),
                        })).await.map_err(|_| Status::cancelled("Collection Demand Closed"))?;
                    }
                    Some(instance_samples_request::Frame::Window(window)) => {
                        let Some(observer) = &observer else {
                            return Err(Status::invalid_argument("Sample Window: a device stream takes no windows"));
                        };
                        if collection && !active {
                            return Err(Status::invalid_argument("Sample Collection Window: acquire listener demand before granting a window"));
                        }
                        if observing.is_some() || !parts.is_empty() {
                            return Err(Status::invalid_argument("Sample Window: the previous window is unanswered"));
                        }
                        let start = window.start.ok_or_else(|| Status::invalid_argument("Sample Window: start time required"))?;
                        crate::stream::validate_timestamp(start)?;
                        let positive = window.period.is_some_and(|period| {
                            period.seconds >= 0 && (0..1_000_000_000).contains(&period.nanos) && (period.seconds > 0 || period.nanos > 0)
                        });
                        if !positive {
                            return Err(Status::invalid_argument("Sample Window: a positive period is required"));
                        }
                        if window.count == 0 {
                            return Err(Status::invalid_argument("Sample Window: a window holds at least one sample"));
                        }
                        let mut window_context = Context::new(plugin::Request {
                            operation: context.operation.clone(), scope: context.scope.clone(), ..Default::default()
                        }, context.plugin.clone(), context.requires.clone());
                        window_context.calls = context.calls.clone();
                        window_context.call_changes = context.call_changes.clone();
                        window_context.samples = context.samples.clone();
                        window_context.sequence = window.sequence;
                        window_context.capturing_executions = window.capturing_executions.clone();
                        window_context.failure_mapper = context.failure_mapper.clone();
                        window_context.output = Some(ChildOutput::Samples {
                            sender: output.clone(), active: Arc::new(AtomicBool::new(true)),
                        });
                        context = window_context;
                        let payload = Message::encode_to_vec(&window);
                        observing = Some((window.count, dispatch(&jobs, observer.handler.clone(), context.clone(), payload).await?));
                    }
                    Some(instance_samples_request::Frame::WindDown(true)) => context.request_wind_down(),
                    Some(instance_samples_request::Frame::Child(child)) if observing.is_some() => context.reply(child).await?,
                    _ => return Err(Status::invalid_argument("Receive Samples: Expected a Window or an Active Child Reply")),
                }
            }
            permit = output.reserve(), if sample.is_some() => {
                let permit = permit.map_err(|_| Status::cancelled("Demand Closed"))?;
                sequence += 1;
                let (row, at) = sample.take().expect("Sample Ready");
                permit.send(Ok(InstanceSamplesResponse { samples: vec![Sample { at, row, sequence }], frame: None }));
            }
            permit = output.reserve(), if can_send_part => {
                let permit = permit.map_err(|_| Status::cancelled("Demand Closed"))?;
                permit.send(Ok(parts.pop_front().expect("Part Ready")));
            }
            result = async { let (_, complete) = observing.as_mut().expect("Observer Active"); complete.await }, if observing.is_some() => {
                let (count, _) = observing.take().expect("Observer Active");
                if let Some(ChildOutput::Samples { active, .. }) = &context.output { active.store(false, Ordering::Release); }
                context.cancellation.send_replace(Some("Samples Window Answered".into()));
                context.cancel_calls("Samples Window Answered").await.map_err(|error| Status::unavailable(error.to_string()))?;
                let columns = result.map_err(|_| Status::internal("Observer Panicked"))?.map_err(|error| match error {
                    Error::Transport(status) if collection => status,
                    error => Status::internal(error.to_string()),
                })?;
                if let Some(layout) = &observer.as_ref().expect("Validated Window Observer").columns {
                    let layout = layout.as_ref().expect("validated observer columns");
                    parts.extend(columns::parts(layout, columns, count, message_bytes)?);
                } else {
                    // A Completed Handler May Still Own the Final Tick in Its Bounded Output Channel.
                    while let Ok((bytes, _)) = rows.try_recv() {
                        parts.push_back(collection_part(&bytes, message_bytes)?);
                    }
                }
            }
            row = rows.recv(), if (observer.is_none() || collection && observing.is_some()) && sample.is_none() => {
                let Some(row) = row else { return Ok(()); };
                if collection {
                    parts.push_back(collection_part(&row.0, message_bytes)?);
                } else {
                    sample = Some(row);
                }
            }
            result = async { producing.as_mut().expect("Producer Active").await }, if producing.is_some() => {
                result.map_err(|_| Status::internal("Producer Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
                producing = None;
            }
        }
    }
    }.await;
    if let Some(ChildOutput::Samples { active, .. }) = &context.output {
        active.store(false, Ordering::Release);
    }
    context.cancellation.send_replace(Some("Samples Stream Ended".into()));
    context.output = None;
    let cancellation = context.cancel_calls("Samples Stream Ended").await;
    if let Some((_, complete)) = observing {
        complete.cancel().await;
    }
    if let Some(complete) = producing {
        complete.cancel().await;
    }
    let stopped = if active {
        transition(&jobs, &observer.as_ref().expect("collection observer").lifecycle, &context, false, None, output).await
    } else {
        Ok(())
    };
    if let Err(error) = &stopped {
        tracing::warn!(%error, "Collection Listener Cleanup Failed");
    }
    result?;
    cancellation.map_err(|error| Status::unavailable(error.to_string()))?;
    stopped
}

async fn transition(
    jobs: &mpsc::Sender<Job>, lifecycle: &Lifecycle, context: &Context, start: bool, input: Option<&mut Streaming<InstanceSamplesRequest>>,
    output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>,
) -> result::Result<(), Status> {
    let mut active = lifecycle.active.lock().await;
    let hook = if start {
        if *active == 0 {
            lifecycle.start.as_ref()
        } else {
            None
        }
    } else {
        *active -= 1;
        if *active == 0 {
            lifecycle.stop.as_ref()
        } else {
            None
        }
    };
    if let Some(hook) = hook {
        lifecycle.completed.store(false, Ordering::Release);
        let mut context = context.clone();
        context.output = None;
        context.cancellation = watch::channel(None).0;
        let mut complete = dispatch(jobs, hook.clone(), context, Vec::new()).await?;
        let closing = async {
            if let Some(input) = input
                && start
            {
                let _ = input.message().await;
                return;
            }
            future::pending::<()>().await;
        };
        let result = tokio::select! {
            biased;
            result = &mut complete => Some(result),
            // Cleanup Retains Its Work Until the Hook Has Completed Its Effect.
            () = output.closed(), if start || lifecycle.completed.load(Ordering::Acquire) => None,
            () = closing => None,
        };
        let Some(result) = result else {
            complete.cancel().await;
            // Preserve Completed Hook Effects When Their Reply Lost the Race With Cancellation.
            if lifecycle.completed.load(Ordering::Acquire) {
                if start {
                    *active += 1;
                }
                return Ok(());
            }
            return Err(Status::cancelled("Collection Listener Hook: demand ended before hook completion"));
        };
        result.map_err(|_| Status::internal("Collection Listener Hook Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
    }
    if start {
        *active += 1;
    }
    Ok(())
}

fn collection_part(bytes: &[u8], message_bytes: usize) -> result::Result<InstanceSamplesResponse, Status> {
    let batch = plugin::CollectionBatch::decode(bytes).map_err(|error| Status::internal(error.to_string()))?;
    let response = InstanceSamplesResponse {
        samples: Vec::new(),
        frame: Some(plugin::instance_samples_response::Frame::Collection(batch)),
    };
    if response.encoded_len() > message_bytes {
        return Err(Status::resource_exhausted("Publish Collection Tick: reduce rows per tick to fit the carrier message ceiling"));
    }
    Ok(response)
}

/* === Tests === */
