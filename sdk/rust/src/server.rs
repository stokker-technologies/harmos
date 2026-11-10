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
use harmos_protocol::v1::{
    self as v1, host_frame, plugin_frame,
    plugin_server::{Plugin as Service, PluginServer},
    publish_frame, response,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send>>;
type Completion = oneshot::Receiver<Result<Vec<u8>>>;

/* === Structs === */

struct Server {
    description: v1::Description,
    operations: HashMap<String, Arc<dyn Handler>>,
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
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl Service for Server {
    async fn describe(&self, request: Request<v1::DescribeRequest>) -> std::result::Result<Response<v1::Description>, Status> {
        if request.into_inner().revision == 0 {
            return Err(Status::invalid_argument("Revision Must Be at Least One"));
        }
        Ok(Response::new(self.description.clone()))
    }

    type InvokeStream = Output<v1::PluginFrame>;

    async fn invoke(&self, request: Request<Streaming<v1::HostFrame>>) -> std::result::Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let Some(host_frame::Frame::Request(request)) = input.message().await?.and_then(|frame| frame.frame) else {
            return Err(Status::invalid_argument("Request Must Be First"));
        };
        if request.invocation.is_empty() || request.attempt == 0 {
            return Err(Status::invalid_argument("Invocation and Attempt Are Required"));
        }
        let handler = self.operations.get(&request.operation).ok_or_else(|| Status::not_found("Unknown Operation"))?.clone();
        handler.validate(&request.payload)?;
        let payload = request.payload.clone();
        let mut context = Context::new(request, self.description.id.clone(), self.requires.clone());
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

    type PublishStream = Output<v1::Samples>;

    async fn publish(&self, request: Request<Streaming<v1::PublishFrame>>) -> std::result::Result<Response<Self::PublishStream>, Status> {
        let mut input = request.into_inner();
        let Some(publish_frame::Frame::Open(open)) = input.message().await?.and_then(|frame| frame.frame) else {
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
            let result = publish(jobs, observer, producer, context, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(output_stream(receiver)))
    }

    async fn stop(&self, _: Request<v1::StopRequest>) -> std::result::Result<Response<v1::StopResponse>, Status> {
        for cleanup in &self.cleanup {
            cleanup().map_err(|error| Status::internal(error.to_string()))?;
        }
        tracing::info!(plugin = %self.description.id, "Plugin Cleanup Completed");
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(v1::StopResponse {}))
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
        description: v1::Description {
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
        operations: plugin.operations.into_iter().map(|operation| (operation.contract.id, operation.handler)).collect(),
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
        .add_service(PluginServer::new(server))
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

pub(crate) fn validate_timestamp(at: prost_types::Timestamp) -> std::result::Result<(), Status> {
    if at.nanos < 0 || at.nanos >= 1_000_000_000 || !(-62_135_596_800..=253_402_300_799).contains(&at.seconds) {
        return Err(Status::invalid_argument("Invalid Timestamp"));
    }
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
            } = job;
            let future = handler.execute(&context, payload);
            let mut future = std::pin::pin!(future);
            let first = futures_util::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx))).await;
            let _ = started.send(());
            let result = match first {
                std::task::Poll::Ready(result) => result,
                std::task::Poll::Pending => tokio::select! {
                    _ = complete.closed() => return,
                    result = &mut future => result,
                },
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
    jobs.send(Job {
        handler,
        context,
        payload,
        complete,
        started,
    })
    .await
    .map_err(|_| Status::unavailable("Plugin Stopped"))?;
    ready.await.map_err(|_| Status::internal("Handler Panicked"))?;
    Ok(receiver)
}

async fn invoke(
    jobs: mpsc::Sender<Job>, handler: Arc<dyn Handler>, context: Context, payload: Vec<u8>, input: &mut Streaming<v1::HostFrame>,
    output: &mpsc::Sender<std::result::Result<v1::PluginFrame, Status>>,
) -> std::result::Result<(), Status> {
    let mut complete = dispatch(&jobs, handler, context.clone(), payload).await?;
    let mut eof = false;
    let result = loop {
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            result = &mut complete => break result.map_err(|_| Status::internal("Handler Panicked"))?,
            frame = input.message(), if !eof => match frame? {
                Some(v1::HostFrame { frame: Some(host_frame::Frame::Child(child)) }) => context.reply(child)?,
                Some(v1::HostFrame { frame: Some(host_frame::Frame::Cancel(cancel)) }) => context.cancel(cancel.reason).await.map_err(|error| Status::unavailable(error.to_string()))?,
                None => { eof = true; if !context.calls.lock().expect("Child Calls Poisoned").pending.is_empty() { return Err(Status::unavailable("Host Closed Before Child Replies")); } },
                _ => return Err(Status::invalid_argument("Repeated Request")),
            }
        }
    };
    let result = match result {
        Ok(payload) => response::Result::Payload(payload),
        Err(error) => error.response(&context.operation, &context.plugin)?,
    };
    output
        .send(Ok(v1::PluginFrame {
            frame: Some(plugin_frame::Frame::Response(v1::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn publish(
    jobs: mpsc::Sender<Job>, observer: Option<Arc<dyn Handler>>, producer: Option<Arc<dyn Handler>>, mut context: Context,
    input: &mut Streaming<v1::PublishFrame>, output: &mpsc::Sender<std::result::Result<v1::Samples, Status>>,
) -> std::result::Result<(), Status> {
    let (samples, mut rows) = mpsc::channel(1);
    context.samples = Some(samples);
    let mut producing = match producer {
        Some(producer) => Some(dispatch(&jobs, producer, context.clone(), Vec::new()).await?),
        None => None,
    };
    let mut observing = None;
    let mut sequence = 0;
    loop {
        tokio::select! {
            biased;
            _ = output.closed() => return Ok(()),
            frame = input.message(), if observing.is_none() => {
                let Some(frame) = frame? else { return Ok(()); };
                let Some(publish_frame::Frame::Tick(tick)) = frame.frame else { return Err(Status::invalid_argument("Expected Tick")); };
                let at = tick.at.ok_or_else(|| Status::invalid_argument("Tick Time Required"))?; validate_timestamp(at)?;
                let handler = observer.as_ref().ok_or_else(|| Status::invalid_argument("Device Streams Do Not Accept Ticks"))?;
                context.sequence = sequence + 1;
                let complete = dispatch(&jobs, handler.clone(), context.clone(), Vec::new()).await?;
                observing = Some((at, complete));
            }
            result = async { let (_, complete) = observing.as_mut().expect("Observer Active"); complete.await }, if observing.is_some() => {
                let (at, _) = observing.take().expect("Observer Active");
                let row = result.map_err(|_| Status::internal("Observer Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
                sequence += 1;
                output.send(Ok(v1::Samples { samples: vec![v1::Sample { at: Some(at), row, sequence }] })).await.map_err(|_| Status::cancelled("Demand Closed"))?;
            }
            row = rows.recv(), if producing.is_some() => {
                let Some((row, at)) = row else { return Ok(()); }; sequence += 1;
                output.send(Ok(v1::Samples { samples: vec![v1::Sample { at, row, sequence }] })).await.map_err(|_| Status::cancelled("Demand Closed"))?;
            }
            result = async { producing.as_mut().expect("Producer Active").await }, if producing.is_some() => {
                result.map_err(|_| Status::internal("Producer Panicked"))?.map_err(|error| Status::internal(error.to_string()))?;
                producing = None;
            }
        }
    }
}

/* === Tests === */
