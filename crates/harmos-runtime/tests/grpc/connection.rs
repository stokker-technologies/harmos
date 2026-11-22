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
    io::{self, Write},
    net::{self, Shutdown},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use hyper_util::rt::TokioIo;
use rcgen::generate_simple_self_signed;
use tokio::{
    net::TcpStream,
    sync::{Semaphore, mpsc},
};
use tonic::{
    Request, Streaming,
    client::Grpc,
    codegen::http::uri::PathAndQuery,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity, ServerTlsConfig},
};
use tonic_reflection::pb::v1::{
    ServerReflectionRequest, server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
};
use tower::service_fn;

// Local Imports
use crate::support::Bank;
use harmos_runtime::facade::storage::{CompoundOutcome, DeleteScopeRequest, MutateResponse, MutationId};
use harmos_runtime::grpc::middleware::{Enter, Invocation, Middleware};
use harmos_runtime::grpc::{Attached, Codec, ConnectionEnd, ConnectionSession, ServerCertificate, Settings, Transcoded};
use harmos_runtime::operations::{
    self, ConnectionId, FailureKind, FunctionContext, Lane, OperationFactory, OperationPolicy, OperationSpec, Response, RetryPolicy,
    WireDeclaration,
};
use harmos_runtime::services::{Resident, ServiceContext, ServiceDefinition};
use harmos_runtime::{Call, Error, Identifier, Message, Refusal, Resource, Result, Runtime};

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
    sessions: mpsc::UnboundedReceiver<ConnectionSession>,
    tls: Option<ClientTlsConfig>,
}

struct Sessions(mpsc::UnboundedSender<ConnectionSession>);

struct StorageObserver {
    call: Call,
    mutation: MutationId,
    completed: mpsc::UnboundedSender<Result<MutateResponse>>,
}

struct QueuedResident(Arc<AtomicUsize>);

/* === Enums === */

enum Transport {
    Plaintext,
    Tls,
    RotatingTls,
}

/* === Traits === */

/* === Implementations === */

impl Middleware for Sessions {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            let session = call.connection().expect("RPC has a connection session");
            assert_eq!(call.context().connection_id(), Some(session.id()));
            assert_eq!(call.context().get::<ConnectionSession>().unwrap().id(), session.id());
            self.0.send(session.clone()).unwrap();
            Ok(())
        })
    }
}

impl Drop for Stopped {
    fn drop(&mut self) {
        self.events.stopped.lock().unwrap().insert(self.id);
    }
}

impl Resident<Bank> for QueuedResident {
    type Error = io::Error;

    async fn run(&mut self, _context: ServiceContext<Bank>) -> io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn finalize(&mut self) -> io::Result<()> {
        Ok(())
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
        Self::with_transport(fail, Transport::Plaintext).await
    }

    async fn with_transport(fail: bool, transport: Transport) -> Self {
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
        let (sessions, received_sessions) = mpsc::unbounded_channel();
        runtime.grpc().register_middleware(Arc::new(Sessions(sessions))).unwrap();
        let (attachment, tls) = match transport {
            Transport::Plaintext => (runtime.grpc().attach(Settings::default()).await.unwrap(), None),
            Transport::Tls | Transport::RotatingTls => {
                let certificate = generate_simple_self_signed(vec!["localhost".into()]).unwrap();
                let pem = certificate.cert.pem();
                let key = certificate.signing_key.serialize_pem();
                let tls = ClientTlsConfig::new().domain_name("localhost").ca_certificate(Certificate::from_pem(&pem));
                let attachment = match transport {
                    Transport::Tls => {
                        let settings = Settings {
                            tls: Some(ServerTlsConfig::new().identity(Identity::from_pem(pem, key))),
                            ..Settings::default()
                        };
                        runtime.grpc().attach(settings).await.unwrap()
                    }
                    Transport::RotatingTls => {
                        let settings = Settings::default();
                        let handshake_timeout = settings.request_timeout;
                        let certificate = ServerCertificate::from_pem(pem.as_bytes(), key.as_bytes()).unwrap();
                        runtime.grpc().attach_with_certificate(settings, certificate, handshake_timeout).await.unwrap()
                    }
                    Transport::Plaintext => unreachable!(),
                };
                (attachment, Some(tls))
            }
        };
        Self {
            runtime,
            attachment,
            calls: received_calls,
            notifications: received_notifications,
            observer,
            sessions: received_sessions,
            tls,
        }
    }

    fn endpoint(&self) -> Endpoint {
        let scheme = if self.tls.is_some() { "https" } else { "http" };
        let endpoint = Endpoint::from_shared(format!("{scheme}://{}", self.attachment.local_addr())).unwrap();
        match &self.tls {
            Some(tls) => endpoint.tls_config(tls.clone()).unwrap(),
            None => endpoint,
        }
    }

    async fn connect(&self) -> Channel {
        self.endpoint().connect().await.unwrap()
    }

    async fn controlled(&self, reset: bool) -> (Channel, net::TcpStream) {
        let socket = TcpStream::connect(self.attachment.local_addr()).await.unwrap();
        if reset {
            socket.set_zero_linger().unwrap();
        }
        let socket = socket.into_std().unwrap();
        let control = socket.try_clone().unwrap();
        let socket = Arc::new(Mutex::new(Some(socket)));
        let channel = self
            .endpoint()
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
    assert_eq!(context.call().get::<ConnectionSession>().map(ConnectionSession::id), context.call().connection_id());
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
async fn middleware_sessions_follow_connections_across_plaintext_and_tls() {
    for transport in [Transport::Plaintext, Transport::Tls, Transport::RotatingTls] {
        // Share Identity Across RPCs
        let mut fixture = Fixture::with_transport(false, transport).await;
        let first = fixture.connect().await;
        let one = fixture.unary(first.clone()).await;
        let session = fixture.sessions.recv().await.unwrap();
        assert_eq!(session.id(), one);
        assert_eq!(fixture.unary(first.clone()).await, one);
        assert_eq!(fixture.sessions.recv().await.unwrap().id(), one);

        // Isolate a Second Connection
        let second = fixture.connect().await;
        let two = fixture.unary(second.clone()).await;
        let other = fixture.sessions.recv().await.unwrap();
        assert_eq!(other.id(), two);
        assert_ne!(one, two);
        assert!(futures_util::poll!(Box::pin(session.ended())).is_pending());

        // Close One Transport While Retaining Its Session
        drop(first);
        session.ended().await;
        assert_eq!(fixture.notifications.recv().await.unwrap(), one);
        assert!(futures_util::poll!(Box::pin(other.ended())).is_pending());
        assert_eq!(fixture.unary(second.clone()).await, two);
        assert_eq!(fixture.sessions.recv().await.unwrap().id(), two);
        assert!(fixture.notifications.try_recv().is_err());

        // Preserve the End Latch and Drain Every Observer
        session.ended().await;
        fixture.runtime.stop().await.unwrap();
        other.ended().await;
        assert_eq!(fixture.notifications.try_recv().unwrap(), two);
        assert!(fixture.notifications.try_recv().is_err());
        drop(second);
    }
}

#[tokio::test]
async fn shutdown_ends_all_sessions_across_plaintext_and_tls() {
    for transport in [Transport::Plaintext, Transport::Tls, Transport::RotatingTls] {
        // Open Independent Sessions
        let mut fixture = Fixture::with_transport(false, transport).await;
        let first = fixture.connect().await;
        fixture.unary(first.clone()).await;
        let one = fixture.sessions.recv().await.unwrap();
        let second = fixture.connect().await;
        fixture.unary(second.clone()).await;
        let two = fixture.sessions.recv().await.unwrap();

        // Stop With Both Connections Still Open
        fixture.runtime.stop().await.unwrap();
        one.ended().await;
        two.ended().await;
        let reported = BTreeSet::from([fixture.notifications.try_recv().unwrap(), fixture.notifications.try_recv().unwrap()]);
        assert_eq!(reported, BTreeSet::from([one.id(), two.id()]));
        assert!(fixture.notifications.try_recv().is_err());
        drop((first, second));
    }
}

#[tokio::test]
async fn tls_failure_after_handshake_ends_only_its_session() {
    for transport in [Transport::Tls, Transport::RotatingTls] {
        // Complete Handshakes and RPCs
        let mut fixture = Fixture::with_transport(false, transport).await;
        let (channel, mut control) = fixture.controlled(false).await;
        fixture.unary(channel.clone()).await;
        let failed = fixture.sessions.recv().await.unwrap();
        let sibling = fixture.connect().await;
        fixture.unary(sibling.clone()).await;
        let live = fixture.sessions.recv().await.unwrap();

        // Inject an Invalid TLS Record Below the Established TLS Stream
        control.write_all(&[0xff, 0x03, 0x03, 0, 1, 0]).unwrap();
        failed.ended().await;
        assert_eq!(fixture.notifications.recv().await.unwrap(), failed.id());
        assert!(futures_util::poll!(Box::pin(live.ended())).is_pending());
        assert_eq!(fixture.unary(sibling.clone()).await, live.id());
        fixture.runtime.stop().await.unwrap();
        assert_eq!(fixture.notifications.try_recv().unwrap(), live.id());
        assert!(fixture.notifications.try_recv().is_err());
        drop((channel, control, sibling));
    }
}

#[tokio::test]
async fn socket_reset_ends_only_its_session_across_plaintext_and_tls() {
    for transport in [Transport::Plaintext, Transport::Tls, Transport::RotatingTls] {
        // Configure an Abortive Close Before Opening the Connection
        let mut fixture = Fixture::with_transport(false, transport).await;
        let (channel, control) = fixture.controlled(true).await;
        fixture.unary(channel.clone()).await;
        let reset = fixture.sessions.recv().await.unwrap();
        let sibling = fixture.connect().await;
        fixture.unary(sibling.clone()).await;
        let live = fixture.sessions.recv().await.unwrap();

        // Reset One Socket Without Ending Its Sibling
        drop((channel, control));
        reset.ended().await;
        assert_eq!(fixture.notifications.recv().await.unwrap(), reset.id());
        assert!(futures_util::poll!(Box::pin(live.ended())).is_pending());
        assert_eq!(fixture.unary(sibling.clone()).await, live.id());
        fixture.runtime.stop().await.unwrap();
        assert_eq!(fixture.notifications.try_recv().unwrap(), live.id());
        assert!(fixture.notifications.try_recv().is_err());
        drop(sibling);
    }
}

#[tokio::test]
async fn shutdown_cancels_queued_work_before_connection_observers_settle() {
    // Hold Connection Settlement
    let mut fixture = Fixture::new(false).await;
    let channel = fixture.connect().await;
    let connection = fixture.unary(channel.clone()).await;
    *fixture.observer.blocked.lock().unwrap() = Some(connection);

    // Queue a Resident and an Operation Without Yielding
    let attempts = Arc::new(AtomicUsize::new(0));
    let created = attempts.clone();
    let definition = ServiceDefinition {
        id: "queued".into(),
        policy: harmos_runtime::services::POLICY,
    };
    fixture.runtime.operations.register_service(definition, move |_| QueuedResident(created.clone())).unwrap();
    let resident = fixture.runtime.operations.submit_resident("queued").await.unwrap();
    let mut status = resident.subscribe();
    let operation = fixture.runtime.operations.submit::<UnaryProbe>(UnaryProbe {}).unwrap();
    assert_eq!(resident.status().phase(), operations::OperationPhase::Queued);
    assert_eq!(operation.status().phase(), operations::OperationPhase::Queued);

    // Begin Shutdown Before Either Queued Body Can Start
    let shutdown = fixture.runtime.stop();
    tokio::pin!(shutdown);
    assert!(futures_util::poll!(&mut shutdown).is_pending());
    assert_eq!(fixture.notifications.recv().await.unwrap(), connection);
    let stopped = status.wait_for(|status| status.phase() == operations::OperationPhase::Stopped).await.unwrap().clone();
    let answer = operation.await;
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert!(stopped.started_at().is_none());
    assert_eq!(stopped.outcome(), Some(operations::OperationOutcome::Cancelled));
    assert!(matches!(answer, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert!(fixture.calls.try_recv().is_err());
    assert!(fixture.runtime.operations.submit_resident("queued").await.is_err());

    // Repeated Stop Preserves Fresh Settlement Calls
    let operation = fixture.runtime.operations.submit::<UnaryProbe>(UnaryProbe {}).unwrap();
    let repeated = fixture.runtime.stop();
    tokio::pin!(repeated);
    assert!(futures_util::poll!(&mut repeated).is_pending());
    assert_eq!(*operation.await.unwrap(), 1);
    assert_eq!(fixture.calls.recv().await.unwrap(), None);
    fixture.observer.release.add_permits(1);
    shutdown.await.unwrap();
    repeated.await.unwrap();
    drop(channel);
}

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
    let (channel, control) = fixture.controlled(false).await;
    let (id, output) = fixture.call(channel.clone(), true).await;
    control.shutdown(Shutdown::Write).unwrap();
    assert_eq!(fixture.notifications.recv().await.unwrap(), id);
    assert!(fixture.observer.events.stopped.lock().unwrap().contains(&id));
    fixture.runtime.stop().await.unwrap();
    assert!(fixture.observer.out_of_order.lock().unwrap().is_empty());
    assert!(fixture.notifications.try_recv().is_err());
    drop((output, channel, control));
}
