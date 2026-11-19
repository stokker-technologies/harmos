/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Connection Identity and Settlement Over Real Loopback Sockets.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeSet,
    future::Future,
    io,
    net::{self, Shutdown},
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use hyper_util::rt::TokioIo;
use tokio::{
    net::TcpStream,
    sync::{Semaphore, mpsc},
};
use tonic::{
    Request, Streaming,
    client::Grpc,
    codegen::http::uri::PathAndQuery,
    transport::{Channel, Endpoint},
};
use tonic_reflection::pb::v1::{
    ServerReflectionRequest, server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
};
use tower::service_fn;

// Local Imports
use crate::support::Bank;
use harmos_runtime::facade::storage::{CompoundOutcome, DeleteScopeRequest, MutateResponse, MutationId};
use harmos_runtime::grpc::{Attached, Codec, ConnectionEnd, Settings, Transcoded};
use harmos_runtime::operations::{
    self, ConnectionId, FailureKind, FunctionContext, Lane, OperationFactory, OperationPolicy, OperationSpec, Response, RetryPolicy,
    WireDeclaration,
};
use harmos_runtime::{Call, Error, Identifier, Message, Resource, Result, Runtime};

/* === Constants === */

static PROBE: WireDeclaration = WireDeclaration::request::<Probe>(
    "connection.probe",
    OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(operations::Visibility::Public)
.grpc(operations::Grpc::new("fixture.host.v1", "Connection", "Probe"));
static UNARY: WireDeclaration = WireDeclaration::request::<UnaryProbe>(
    "connection.unary",
    OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(operations::Visibility::Public)
.grpc(operations::Grpc::new("fixture.host.v1", "Connection", "Unary"));
const UNARY_SPECIFICATION: OperationSpec<UnaryProbe> = OperationSpec::new(&UNARY);
const SPECIFICATION: OperationSpec<Probe> = OperationSpec::new(&PROBE);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message, Debug, Default, Clone, harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u32)]
struct UnaryProbe {}

#[derive(Message, Debug, Default, Clone, harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u32, output = u32)]
struct Probe {
    streaming: bool,
}

struct Events {
    calls: mpsc::UnboundedSender<Option<ConnectionId>>,
    stopped: Mutex<BTreeSet<ConnectionId>>,
    live_streams: Mutex<BTreeSet<ConnectionId>>,
}

struct Stopped {
    events: Arc<Events>,
    id: ConnectionId,
}

struct Observer {
    events: Arc<Events>,
    notifications: mpsc::UnboundedSender<ConnectionId>,
    fail: bool,
    blocked: Mutex<Option<ConnectionId>>,
    out_of_order: Mutex<BTreeSet<ConnectionId>>,
    release: Semaphore,
}

struct Fixture {
    runtime: Runtime<Bank>,
    attachment: Attached,
    calls: mpsc::UnboundedReceiver<Option<ConnectionId>>,
    notifications: mpsc::UnboundedReceiver<ConnectionId>,
    observer: Arc<Observer>,
}

struct StorageObserver {
    call: Call,
    mutation: MutationId,
    completed: mpsc::UnboundedSender<Result<MutateResponse>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Stopped {
    fn drop(&mut self) {
        self.events.stopped.lock().unwrap().insert(self.id);
    }
}

impl ConnectionEnd for Observer {
    fn ended(&self, connection: ConnectionId) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let streaming = self.events.live_streams.lock().unwrap().contains(&connection);
            let stopped = self.events.stopped.lock().unwrap().contains(&connection);
            if streaming && !stopped {
                self.out_of_order.lock().unwrap().insert(connection);
            }
            self.notifications.send(connection).unwrap();
            let blocked = *self.blocked.lock().unwrap() == Some(connection);
            if blocked {
                self.release.acquire().await.unwrap().forget();
            }
            if self.fail {
                return Err(Error::PermissionDenied);
            }
            Ok(())
        })
    }
}

impl ConnectionEnd for StorageObserver {
    fn ended(&self, _connection: ConnectionId) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let request = DeleteScopeRequest {
                mutation: Some(self.mutation.clone()),
                scope: Identifier::new("connection-resources").unwrap(),
            };
            let written = self.call.storage().delete_scope(request).await;
            self.completed.send(written).unwrap();
            Ok(())
        })
    }
}

impl Fixture {
    async fn new(fail: bool) -> Self {
        let (calls, received_calls) = mpsc::unbounded_channel();
        let (notifications, received_notifications) = mpsc::unbounded_channel();
        let events = Arc::new(Events {
            calls,
            stopped: Mutex::default(),
            live_streams: Mutex::default(),
        });
        let observer = Arc::new(Observer {
            events: events.clone(),
            notifications,
            fail,
            blocked: Mutex::default(),
            out_of_order: Mutex::default(),
            release: Semaphore::new(0),
        });
        let operation =
            OperationFactory::function(SPECIFICATION, const { &[Resource::of::<Events>()] }, probe, |_| FailureKind::Permanent).unwrap();
        let unary =
            OperationFactory::function(UNARY_SPECIFICATION, const { &[Resource::of::<Events>()] }, unary_probe, |_| FailureKind::Permanent)
                .unwrap();
        let runtime = crate::support::app::booted(Default::default(), |setup| {
            setup.resource::<Events>(events).operation(operation, Lane::default()).operation(unary, Lane::default());
        })
        .await;
        runtime.grpc().register_connection_end(observer.clone()).unwrap();
        assert!(runtime.grpc().register_connection_end(observer.clone()).is_err());
        let attachment = runtime.grpc().attach(Settings::default()).await.unwrap();
        Self {
            runtime,
            attachment,
            calls: received_calls,
            notifications: received_notifications,
            observer,
        }
    }

    async fn connect(&self) -> Channel {
        Endpoint::from_shared(format!("http://{}", self.attachment.local_addr())).unwrap().connect().await.unwrap()
    }

    async fn controlled(&self) -> (Channel, net::TcpStream) {
        let socket = TcpStream::connect(self.attachment.local_addr()).await.unwrap().into_std().unwrap();
        let control = socket.try_clone().unwrap();
        let socket = Arc::new(Mutex::new(Some(socket)));
        let channel = Endpoint::from_shared(format!("http://{}", self.attachment.local_addr()))
            .unwrap()
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.lock().unwrap().take();
                async move {
                    let socket = socket.ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "open a new test connection"))?;
                    Ok::<_, io::Error>(TokioIo::new(TcpStream::from_std(socket)?))
                }
            }))
            .await
            .unwrap();
        (channel, control)
    }

    async fn unary(&mut self, channel: Channel) -> ConnectionId {
        let settings = Settings::default().operations;
        let mut client = Grpc::new(channel);
        client.ready().await.unwrap();
        let response = client
            .unary(
                Request::new(Response::new(UnaryProbe {})),
                PathAndQuery::from_static("/fixture.host.v1.Connection/Unary"),
                Codec::<Transcoded, u32, Response<UnaryProbe>>::new(settings.decode, settings.encode).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(*response.into_inner(), 1);
        self.calls.recv().await.unwrap().unwrap()
    }

    async fn call(&mut self, channel: Channel, streaming: bool) -> (ConnectionId, Streaming<operations::StreamItem<u32>>) {
        let settings = Settings::default().operations;
        let mut client = Grpc::new(channel);
        client.ready().await.unwrap();
        let mut output = client
            .server_streaming(
                Request::new(Response::new(Probe { streaming })),
                PathAndQuery::from_static("/fixture.host.v1.Connection/Probe"),
                Codec::<Transcoded, u32, Response<Probe>>::new(settings.decode, settings.encode).unwrap(),
            )
            .await
            .unwrap()
            .into_inner();
        assert_eq!(*output.message().await.unwrap().unwrap(), 1);
        let id = self.calls.recv().await.unwrap().unwrap();
        if !streaming {
            assert!(output.message().await.unwrap().is_none());
        }
        (id, output)
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn unary_probe(_request: UnaryProbe, context: FunctionContext<UnaryProbe>) -> Result<Response<u32>> {
    context.resource::<Events>().unwrap().calls.send(context.call().connection_id()).unwrap();
    Ok(1.into())
}

async fn probe(request: Probe, context: FunctionContext<Probe>) -> Result<Response<u32>> {
    let events = context.resource::<Events>().unwrap();
    let id = context.call().connection_id();
    events.calls.send(id).unwrap();
    let _stopped = if request.streaming {
        let id = id.expect("streaming probe has a transport connection");
        events.live_streams.lock().unwrap().insert(id);
        Some(Stopped { events, id })
    } else {
        None
    };
    context.streams().send_with(4, || Ok(1)).await?;
    if request.streaming {
        context.cancelled().await;
    }
    Ok(1.into())
}

/* === Tests === */

#[tokio::test]
async fn shutdown_observer_writes_storage_through_runtime_before_services_close() {
    // Start Storage and Register the Observer
    let fixture = crate::storage::fixture::Fixture::new(crate::storage::fixture::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), Lane::default()).await.unwrap();
    let (completed, mut written) = mpsc::unbounded_channel();
    let observer = StorageObserver {
        call: fixture.runtime.for_call(&operations::CallContext::default()),
        mutation: MutationId {
            window: fixture.storage.mutation_window().unwrap().id,
            key: Identifier::new("connection-cleanup").unwrap(),
        },
        completed,
    };
    fixture.runtime.grpc().register_connection_end(Arc::new(observer)).unwrap();

    // Leave a Connection Open Until Host Shutdown
    let settings = Settings { reflection: true, ..Settings::default() };
    let attachment = fixture.runtime.grpc().attach(settings).await.unwrap();
    let channel = Endpoint::from_shared(format!("http://{}", attachment.local_addr())).unwrap().connect().await.unwrap();
    let mut client = ServerReflectionClient::new(channel.clone());
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    let mut services = client.server_reflection_info(futures_util::stream::iter([request])).await.unwrap().into_inner();
    assert!(services.message().await.unwrap().is_some());
    fixture.runtime.stop().await.unwrap();

    // Shutdown Awaits the Successful Runtime Storage Write
    let response = written.try_recv().unwrap().unwrap();
    assert!(matches!(response.outcome, CompoundOutcome::Applied { .. }));
    assert!(response.receipt.unwrap().completed);
    assert!(fixture.storage.status().closed);
    assert!(written.try_recv().is_err());
    drop((services, client, channel, attachment));
}

#[tokio::test]
async fn identity_is_stable_and_open_connections_are_not_reported() {
    let mut fixture = Fixture::new(false).await;
    assert_eq!(operations::CallContext::default().connection_id(), None);
    assert_eq!(*fixture.runtime.operation(UnaryProbe {}).await.unwrap(), 1);
    assert_eq!(fixture.calls.recv().await.unwrap(), None);
    let first = fixture.connect().await;
    let one = fixture.unary(first.clone()).await;
    let again = fixture.unary(first.clone()).await;
    assert_eq!(one, again);
    let (stream_id, output) = fixture.call(first.clone(), true).await;
    assert_eq!(stream_id, one);
    drop(output);
    let second = fixture.connect().await;
    let two = fixture.unary(second.clone()).await;
    assert_ne!(one, two);
    assert!(fixture.notifications.try_recv().is_err());
    drop(first);
    assert_eq!(fixture.notifications.recv().await.unwrap(), one);
    let again = fixture.unary(second.clone()).await;
    assert_eq!(again, two);
    assert!(fixture.notifications.try_recv().is_err());
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert_eq!(fixture.notifications.try_recv().unwrap(), two);
    assert!(fixture.notifications.try_recv().is_err());
}

#[tokio::test]
async fn closing_channel_reports_once_after_stream_cancellation() {
    let mut fixture = Fixture::new(false).await;
    let channel = fixture.connect().await;
    let (id, output) = fixture.call(channel.clone(), true).await;
    assert!(fixture.notifications.try_recv().is_err());
    drop(output);
    drop(channel);
    assert_eq!(fixture.notifications.recv().await.unwrap(), id);
    assert!(fixture.observer.events.stopped.lock().unwrap().contains(&id));
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert!(fixture.notifications.try_recv().is_err());
}

#[tokio::test]
async fn shutdown_reports_every_open_connection_once() {
    let mut fixture = Fixture::new(false).await;
    let first = fixture.connect().await;
    let (one, stream_one) = fixture.call(first.clone(), true).await;
    let second = fixture.connect().await;
    let (two, stream_two) = fixture.call(second.clone(), true).await;
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    let reported = BTreeSet::from([fixture.notifications.try_recv().unwrap(), fixture.notifications.try_recv().unwrap()]);
    assert_eq!(reported, BTreeSet::from([one, two]));
    assert!(fixture.notifications.try_recv().is_err());
    drop((first, second, stream_one, stream_two));
}

#[tokio::test]
async fn failing_observer_does_not_prevent_next_notification() {
    let mut fixture = Fixture::new(true).await;
    let first = fixture.connect().await;
    let one = fixture.unary(first.clone()).await;
    drop(first);
    assert_eq!(fixture.notifications.recv().await.unwrap(), one);
    let second = fixture.connect().await;
    let two = fixture.unary(second.clone()).await;
    drop(second);
    assert_eq!(fixture.notifications.recv().await.unwrap(), two);
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert!(fixture.notifications.try_recv().is_err());
}

#[tokio::test]
async fn slow_observer_does_not_block_another_connection_or_escape_shutdown() {
    let mut fixture = Fixture::new(false).await;
    let first = fixture.connect().await;
    let one = fixture.unary(first.clone()).await;
    *fixture.observer.blocked.lock().unwrap() = Some(one);
    drop(first);
    assert_eq!(fixture.notifications.recv().await.unwrap(), one);
    let second = fixture.connect().await;
    let two = fixture.unary(second.clone()).await;
    drop(second);
    assert_eq!(fixture.notifications.recv().await.unwrap(), two);
    let shutdown = fixture.runtime.stop();
    tokio::pin!(shutdown);
    assert!(futures_util::poll!(&mut shutdown).is_pending());
    fixture.observer.release.add_permits(1);
    shutdown.await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert!(fixture.notifications.try_recv().is_err());
}

#[tokio::test]
async fn socket_half_close_settles_a_stream_before_notification() {
    let mut fixture = Fixture::new(false).await;
    let (channel, control) = fixture.controlled().await;
    let (id, output) = fixture.call(channel.clone(), true).await;
    control.shutdown(Shutdown::Write).unwrap();
    assert_eq!(fixture.notifications.recv().await.unwrap(), id);
    assert!(fixture.observer.events.stopped.lock().unwrap().contains(&id));
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert!(fixture.notifications.try_recv().is_err());
    drop((output, channel, control));
}
