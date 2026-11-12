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
use std::{collections::HashMap, io::Write, pin::Pin, sync::Arc, time::SystemTime};

// Third Party Imports
use futures_core::Stream;
use futures_util::stream;
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
};
use tonic::{Request, Response, Status, Streaming};

// Local Imports
use crate::{Context, Error, Plugin, Result, operation::Handler};
use harmos_protocol::plugin::{
    instance_invoke_request, instance_invoke_response, instance_samples_request,
    instance_server::{Instance as Service, InstanceServer},
};
use harmos_protocol::v1::{self as v1, response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send>>;

/* === Structs === */

struct Completion {
    result: oneshot::Receiver<Result<Vec<u8>>>,
    settled: oneshot::Receiver<()>,
}

struct Settled(Option<oneshot::Sender<()>>);

struct Server {
    description: harmos_protocol::plugin::InstanceGetResponse,
    operations: HashMap<String, Arc<dyn Handler>>,
    local: Arc<HashMap<String, Arc<crate::Operation>>>,
    streams: HashMap<String, Publication>,
    requires: Arc<HashMap<String, Vec<u8>>>,
    jobs: mpsc::Sender<Job>,
    cleanup: Vec<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

struct Publication {
    observer: Option<Arc<dyn Handler>>,
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

impl std::future::Future for Completion {
    type Output = std::result::Result<Result<Vec<u8>>, oneshot::error::RecvError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
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
impl Service for Server {
    async fn get(
        &self, request: Request<harmos_protocol::plugin::InstanceGetRequest>,
    ) -> std::result::Result<Response<harmos_protocol::plugin::InstanceGetResponse>, Status> {
        if request.into_inner().revision == 0 {
            return Err(Status::invalid_argument("Revision Must Be at Least One"));
        }
        Ok(Response::new(self.description.clone()))
    }

    type InvokeStream = Output<harmos_protocol::plugin::InstanceInvokeResponse>;

    async fn invoke(
        &self, request: Request<Streaming<harmos_protocol::plugin::InstanceInvokeRequest>>,
    ) -> std::result::Result<Response<Self::InvokeStream>, Status> {
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
        context.local = self.local.clone();
        let (output, receiver) = mpsc::channel(1);
        context.output = Some(output.clone());
        let jobs = self.jobs.clone();
        tokio::spawn(async move {
            let result = invoke(jobs, handler, context, payload, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(output_stream(receiver)))
    }

    type SamplesStream = Output<harmos_protocol::plugin::InstanceSamplesResponse>;

    async fn samples(
        &self, request: Request<Streaming<harmos_protocol::plugin::InstanceSamplesRequest>>,
    ) -> std::result::Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let Some(instance_samples_request::Frame::Open(open)) = input.message().await?.and_then(|frame| frame.frame) else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        let publication = self.streams.get(&open.stream).ok_or_else(|| Status::not_found("Unknown Stream"))?;
        let observer = publication.observer.clone();
        let producer = publication.producer.clone();
        let context = Context::new(
            v1::Request {
                scope: open.scope,
                operation: open.stream,
                ..Default::default()
            },
            self.description.id.clone(),
            self.requires.clone(),
        );
        let (output, receiver) = mpsc::channel(1);
        let jobs = self.jobs.clone();
        tokio::spawn(async move {
            let result = samples(jobs, observer, producer, context, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(output_stream(receiver)))
    }

    async fn stop(
        &self, _: Request<harmos_protocol::plugin::InstanceStopRequest>,
    ) -> std::result::Result<Response<harmos_protocol::plugin::InstanceStopResponse>, Status> {
        for cleanup in &self.cleanup {
            cleanup().map_err(|error| Status::internal(error.to_string()))?;
        }
        tracing::info!(plugin = %self.description.id, "Plugin Cleanup Completed");
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(harmos_protocol::plugin::InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub(crate) async fn serve(plugin: Plugin, catalog: v1::Catalog) -> Result<()> {
    let started_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|error| Error::Invalid(error.to_string()))?;
    let address = std::env::var("HARMOS_ADDRESS").unwrap_or_else(|_| "127.0.0.1:0".into());
    let listener = TcpListener::bind(address).await?;
    let (shutdown, stopped) = oneshot::channel();
    let (jobs, receiver) = mpsc::channel(1);
    let executor = tokio::task::spawn_local(execute(receiver));
    let requires = Plugin::requirements(&catalog);
    let streams = plugin
        .streams
        .into_iter()
        .map(|stream| {
            let observer =
                plugin.observers.iter().find(|observer| observer.stream == stream.declaration.id).map(|observer| observer.handler.clone());
            (stream.declaration.id, Publication { observer, producer: stream.producer })
        })
        .collect();
    let server = Server {
        description: harmos_protocol::plugin::InstanceGetResponse {
            id: plugin.id,
            version: plugin.version,
            started_at: Some(prost_types::Timestamp {
                seconds: started_at.as_secs() as i64,
                nanos: started_at.subsec_nanos() as i32,
            }),
            catalog: Some(catalog),
            revision: 1,
            sdk: "harmos_rust".into(),
            sdk_version: env!("CARGO_PKG_VERSION").into(),
        },
        local: Arc::new(
            plugin
                .operations
                .iter()
                .filter(|operation| operation.contract.visibility == v1::Visibility::Private as i32)
                .map(|operation| (operation.contract.id.clone(), Arc::new(operation.clone())))
                .collect(),
        ),
        operations: plugin
            .operations
            .into_iter()
            .filter(|operation| operation.contract.visibility != v1::Visibility::Private as i32)
            .map(|operation| (operation.contract.id, operation.handler))
            .collect(),
        streams,
        requires,
        jobs,
        cleanup: plugin.cleanup,
        shutdown: Mutex::new((!plugin.remote).then_some(shutdown)),
    };
    let address = listener.local_addr()?;
    let incoming = stream::unfold(listener, |listener| async {
        let accepted = listener.accept().await.map(|(socket, _)| socket);
        Some((accepted, listener))
    });
    println!("harmos listening {address}");
    std::io::stdout().flush()?;
    tracing::info!(%address, "Plugin Listening");
    let result = tonic::transport::Server::builder()
        .add_service(InstanceServer::new(server))
        .serve_with_incoming_shutdown(incoming, async move {
            if plugin.remote {
                std::future::pending::<()>().await;
            }
            let _ = stopped.await;
        })
        .await;
    executor.abort();
    let _ = executor.await;
    result?;
    Ok(())
}

/* === Private Functions === */

fn output_stream<T: Send + 'static>(receiver: mpsc::Receiver<std::result::Result<T, Status>>) -> Output<T> {
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
                let mut future = std::pin::pin!(future);
                let first = futures_util::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx))).await;
                let _ = started.send(());
                match first {
                    std::task::Poll::Ready(result) => result,
                    std::task::Poll::Pending => tokio::select! {
                        _ = complete.closed() => return,
                        result = &mut future => result,
                    },
                }
            };
            let _ = complete.send(result);
        });
    }
}

async fn dispatch(
    jobs: &mpsc::Sender<Job>, handler: Arc<dyn Handler>, context: Context, payload: Vec<u8>,
) -> std::result::Result<Completion, Status> {
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
    jobs: mpsc::Sender<Job>, handler: Arc<dyn Handler>, context: Context, payload: Vec<u8>,
    input: &mut Streaming<harmos_protocol::plugin::InstanceInvokeRequest>,
    output: &mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceInvokeResponse, Status>>,
) -> std::result::Result<(), Status> {
    let mut complete = dispatch(&jobs, handler, context.clone(), payload).await?;
    let mut eof = false;
    let result = loop {
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            _ = context.call_changes.notified() => context.flush_cancels().await.map_err(|error| Status::unavailable(error.to_string()))?,
            result = &mut complete => break result.map_err(|_| Status::internal("Handler Panicked"))?,
            frame = input.message(), if !eof => match frame? {
                Some(harmos_protocol::plugin::InstanceInvokeRequest { frame: Some(instance_invoke_request::Frame::Child(child)) }) => context.reply(child).await?,
                Some(harmos_protocol::plugin::InstanceInvokeRequest { frame: Some(instance_invoke_request::Frame::Cancel(cancel)) }) => context.cancel(cancel.reason).await.map_err(|error| Status::unavailable(error.to_string()))?,
                None => { eof = true; if !context.calls.lock().expect("Child Calls Poisoned").pending.is_empty() { return Err(Status::unavailable("Host Closed Before Child Replies")); } },
                _ => return Err(Status::invalid_argument("Repeated Request")),
            }
        }
    };
    context.cancel_calls(false, "Invocation Settled", false).await.map_err(|error| Status::unavailable(error.to_string()))?;
    let result = match result {
        Ok(payload) => response::Result::Payload(payload),
        Err(error) => error.response(&context.operation, &context.plugin)?,
    };
    output
        .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Response(v1::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn samples(
    jobs: mpsc::Sender<Job>, observer: Option<Arc<dyn Handler>>, producer: Option<Arc<dyn Handler>>, mut context: Context,
    input: &mut Streaming<harmos_protocol::plugin::InstanceSamplesRequest>,
    output: &mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceSamplesResponse, Status>>,
) -> std::result::Result<(), Status> {
    let (samples, mut rows) = mpsc::channel(1);
    context.samples = Some(samples);
    let mut producing = match producer {
        Some(producer) => Some(dispatch(&jobs, producer, context.clone(), Vec::new()).await?),
        None => None,
    };
    let mut observing = None;
    let mut next_tick = None;
    let mut sample = None;
    let mut sequence = 0;
    let result = async {
    loop {
        if observing.is_none()
            && sample.is_none()
            && let Some(at) = next_tick.take()
        {
            let handler = observer.as_ref().expect("Validated Observer Tick");
            context.sequence = sequence + 1;
            observing = Some((at, dispatch(&jobs, handler.clone(), context.clone(), Vec::new()).await?));
        }
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            frame = input.message(), if next_tick.is_none() => {
                let Some(frame) = frame? else { return Ok(()); };
                let Some(instance_samples_request::Frame::Tick(tick)) = frame.frame else { return Err(Status::invalid_argument("Expected Tick")); };
                let at = tick.at.ok_or_else(|| Status::invalid_argument("Tick Time Required"))?; crate::stream::validate_timestamp(at)?;
                if observer.is_none() { return Err(Status::invalid_argument("Device Streams Do Not Accept Ticks")); }
                next_tick = Some(at);
            }
            permit = output.reserve(), if sample.is_some() => {
                let permit = permit.map_err(|_| Status::cancelled("Demand Closed"))?;
                sequence += 1;
                let (row, at) = sample.take().expect("Sample Ready");
                permit.send(Ok(harmos_protocol::plugin::InstanceSamplesResponse { samples: vec![v1::Sample { at, row, sequence }] }));
            }
            result = async { let (_, complete) = observing.as_mut().expect("Observer Active"); complete.await }, if observing.is_some() => {
                let (at, _) = observing.take().expect("Observer Active");
                let row = result.map_err(|_| Status::internal("Observer Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
                sample = Some((row, Some(at)));
            }
            row = rows.recv(), if observer.is_none() && sample.is_none() => {
                let Some(row) = row else { return Ok(()); };
                sample = Some(row);
            }
            result = async { producing.as_mut().expect("Producer Active").await }, if producing.is_some() => {
                result.map_err(|_| Status::internal("Producer Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
                producing = None;
            }
        }
    }
    }.await;
    if let Some((_, complete)) = observing {
        complete.cancel().await;
    }
    if let Some(complete) = producing {
        complete.cancel().await;
    }
    result
}

/* === Tests === */
