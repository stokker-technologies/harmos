/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Placement Story Mechanics

/* === Imports === */

// Standard Library Imports
use std::{
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_core::Stream;
use futures_util::stream;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Notify, mpsc, oneshot, watch},
    task::{JoinHandle, JoinSet},
    time::Instant,
};
use tonic::{Request, Response, Status, Streaming};

// Local Imports
use harmos_protocol::plugin::instance_server::{Instance, InstanceServer};
use harmos_protocol::v1;
use harmos_runtime::{
    Runtime,
    clock::{Clock, Reading, Sleep, Source},
    plugin::grpc::{Activation, PlacementSettings},
};

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;
pub type App = crate::support::Bank;

/* === Structs === */

#[derive(Clone)]
pub struct Manual {
    state: Arc<Mutex<TimeState>>,
    registered: Arc<Notify>,
}

struct TimeState {
    reading: Reading,
    timers: Vec<(Instant, oneshot::Sender<()>)>,
}

pub struct Remote {
    pub address: String,
    service: Fixture,
    disconnect: mpsc::Sender<oneshot::Sender<()>>,
    proxy: JoinHandle<()>,
    server: JoinHandle<()>,
}

#[derive(Clone)]
struct Fixture {
    catalog: Arc<Mutex<v1::Catalog>>,
    started_at: Arc<AtomicI64>,
    stops: Arc<AtomicUsize>,
    pending_description: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pending_stop: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

/* === Implementations === */

impl Manual {
    pub fn new() -> Self {
        let reading = Clock::real_at(SystemTime::UNIX_EPOCH).read();
        Self {
            state: Arc::new(Mutex::new(TimeState { reading, timers: Vec::new() })),
            registered: Arc::new(Notify::new()),
        }
    }

    pub async fn advance_next(&self) {
        loop {
            let registered = self.registered.notified();
            let timer = {
                let mut state = self.state.lock().unwrap();
                state.timers.retain(|(_, sender)| !sender.is_closed());
                let index = state.timers.iter().enumerate().min_by_key(|(_, timer)| timer.0).map(|(index, _)| index);
                index.map(|index| {
                    let (at, sender) = state.timers.remove(index);
                    let elapsed = at.saturating_duration_since(state.reading.instant);
                    state.reading.instant += elapsed;
                    state.reading.date += elapsed;
                    sender
                })
            };
            if let Some(timer) = timer {
                let _ = timer.send(());
                return;
            }
            registered.await;
        }
    }
}

impl Source for Manual {
    fn read(&self) -> Reading {
        self.state.lock().unwrap().reading
    }

    fn sleep_until(&self, deadline: Instant) -> Sleep {
        let (sender, receiver) = oneshot::channel();
        self.state.lock().unwrap().timers.push((deadline, sender));
        self.registered.notify_one();
        Box::pin(async move {
            let _ = receiver.await;
        })
    }
}

impl Remote {
    pub async fn start() -> Self {
        let service = Fixture {
            catalog: Arc::new(Mutex::new({
                let mut catalog = v1::Catalog::default();
                catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
                catalog
            })),
            started_at: Arc::new(AtomicI64::new(100)),
            stops: Arc::new(AtomicUsize::new(0)),
            pending_description: Arc::new(Mutex::new(None)),
            pending_stop: Arc::new(Mutex::new(None)),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream = listener.local_addr().unwrap();
        let incoming = stream::unfold(listener, |listener| async move {
            let socket = listener.accept().await.map(|(socket, _)| socket);
            Some((socket, listener))
        });
        let endpoint = service.clone();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder().add_service(InstanceServer::new(endpoint)).serve_with_incoming(incoming).await.unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (disconnect, mut commands) = mpsc::channel::<oneshot::Sender<()>>(1);
        let proxy = tokio::spawn(async move {
            let mut tunnels = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut downstream, _) = accepted.unwrap();
                        tunnels.spawn(async move {
                            let mut upstream = TcpStream::connect(upstream).await.unwrap();
                            let _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await;
                        });
                    }
                    Some(reply) = commands.recv() => {
                        tunnels.abort_all();
                        while tunnels.join_next().await.is_some() {}
                        let _ = reply.send(());
                    }
                    Some(_) = tunnels.join_next(), if !tunnels.is_empty() => {},
                }
            }
        });
        Self {
            address,
            service,
            disconnect,
            proxy,
            server,
        }
    }

    pub async fn drop_connection(&self, started_at: i64) {
        self.service.started_at.store(started_at, Ordering::SeqCst);
        let (reply, done) = oneshot::channel();
        self.disconnect.send(reply).await.unwrap();
        done.await.unwrap();
    }

    pub fn stops(&self) -> usize {
        self.service.stops.load(Ordering::SeqCst)
    }

    pub fn catalog(&self, catalog: v1::Catalog) {
        *self.service.catalog.lock().unwrap() = catalog;
    }

    pub fn hold_description(&self) -> oneshot::Receiver<()> {
        let (sender, receiver) = oneshot::channel();
        *self.service.pending_description.lock().unwrap() = Some(sender);
        receiver
    }

    pub fn hold_stop(&self) -> oneshot::Receiver<()> {
        let (sender, receiver) = oneshot::channel();
        *self.service.pending_stop.lock().unwrap() = Some(sender);
        receiver
    }

    pub async fn get(&self) -> harmos_protocol::plugin::InstanceGetResponse {
        let mut client = harmos_protocol::plugin::instance_client::InstanceClient::connect(self.address.clone()).await.unwrap();
        client.get(harmos_protocol::plugin::InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap().into_inner()
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.proxy.abort();
        self.server.abort();
    }
}

#[tonic::async_trait]
impl Instance for Fixture {
    async fn get(
        &self, _: Request<harmos_protocol::plugin::InstanceGetRequest>,
    ) -> Result<Response<harmos_protocol::plugin::InstanceGetResponse>, Status> {
        let pending = self.pending_description.lock().unwrap().take();
        if let Some(pending) = pending {
            let _ = pending.send(());
            return std::future::pending().await;
        }
        Ok(Response::new(harmos_protocol::plugin::InstanceGetResponse {
            id: "remote".into(),
            version: "1.0.0".into(),
            revision: 1,
            sdk: "fixture".into(),
            sdk_version: "1.0.0".into(),
            started_at: Some(prost_types::Timestamp {
                seconds: self.started_at.load(Ordering::SeqCst),
                nanos: 0,
            }),
            catalog: Some(self.catalog.lock().unwrap().clone()),
        }))
    }
    type InvokeStream = Output<harmos_protocol::plugin::InstanceInvokeResponse>;
    async fn invoke(
        &self, _: Request<Streaming<harmos_protocol::plugin::InstanceInvokeRequest>>,
    ) -> Result<Response<Self::InvokeStream>, Status> {
        Err(Status::not_found("Unknown Operation"))
    }
    type SamplesStream = Output<harmos_protocol::plugin::InstanceSamplesResponse>;
    async fn samples(
        &self, _: Request<Streaming<harmos_protocol::plugin::InstanceSamplesRequest>>,
    ) -> Result<Response<Self::SamplesStream>, Status> {
        Err(Status::not_found("Unknown Stream"))
    }
    async fn stop(
        &self, _: Request<harmos_protocol::plugin::InstanceStopRequest>,
    ) -> Result<Response<harmos_protocol::plugin::InstanceStopResponse>, Status> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        let pending = self.pending_stop.lock().unwrap().take();
        if let Some(pending) = pending {
            let _ = pending.send(());
            return std::future::pending().await;
        }
        Ok(Response::new(harmos_protocol::plugin::InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub fn catalog() -> v1::Catalog {
    let types = prost_types::FileDescriptorSet {
        file: vec![prost_types::FileDescriptorProto {
            name: Some("fixture.proto".into()),
            package: Some("fixture".into()),
            syntax: Some("proto3".into()),
            message_type: vec![prost_types::DescriptorProto {
                name: Some("Value".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
    };
    let fingerprint = harmos_protocol::schema::fingerprint("fixture.echo", &types, "fixture.Value", "fixture.Value").unwrap();
    let mut catalog = v1::Catalog {
        types: Some(types),
        operations: vec![v1::OperationContract {
            id: "fixture.echo".into(),
            request: "fixture.Value".into(),
            response: "fixture.Value".into(),
            visibility: v1::Visibility::Public as i32,
            fingerprint,
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
    catalog
}

pub async fn runtime(clock: Option<Clock>) -> Runtime<App> {
    crate::support::booted(Default::default(), |setup| {
        if let Some(clock) = clock {
            setup.settings().clock(clock);
        }
    })
    .await
}

pub fn settings() -> PlacementSettings {
    PlacementSettings {
        connect_timeout: Duration::from_secs(30),
        reconnect_delay: Duration::from_secs(1),
        stop_grace: Duration::from_secs(30),
        stream_window: std::num::NonZeroUsize::new(64).unwrap(),
    }
}

pub fn binary(name: &str) -> PathBuf {
    let extension = std::env::consts::EXE_SUFFIX;
    if std::env::var_os("STKR_TEST_PROFILE").is_some() {
        return crate::support::fixtures::binary(name, None);
    }
    std::env::var_os(format!("NEXTEST_BIN_EXE_{}", name.replace('-', "_")))
        .or_else(|| std::env::var_os(format!("CARGO_BIN_EXE_{name}")))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into()))
                .join("../../target/debug")
                .join(format!("{name}{extension}"))
        })
}

pub async fn offline(status: &mut watch::Receiver<harmos_runtime::plugin::grpc::Status>) {
    loop {
        if status.borrow_and_update().activation.is_none() {
            return;
        }
        status.changed().await.unwrap();
    }
}

pub async fn activation(status: &mut watch::Receiver<harmos_runtime::plugin::grpc::Status>, number: u64) -> Activation {
    loop {
        if let Some(activation) = status.borrow_and_update().activation.clone()
            && activation.number >= number
        {
            return activation;
        }
        status.changed().await.unwrap();
    }
}
