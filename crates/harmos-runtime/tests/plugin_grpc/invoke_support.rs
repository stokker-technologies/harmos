/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// SDK Invoke Story Mechanics

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{process::Stdio, sync::Arc, time::Duration};

// Third Party Imports
use prost::Message as _;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{Notify, mpsc, oneshot},
    task::{JoinHandle, JoinSet},
};

// Local Imports
use super::support;
use harmos_runtime::{
    Runtime,
    operations::{OperationPolicy, Registration, RetryPolicy, WireDeclaration},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
#[derive(Clone)]
pub struct Host {
    pub value: Vec<u8>,
}

#[harmos_runtime::message]
pub struct Reply {
    pub value: Vec<u8>,
}

#[derive(Default)]
pub struct Signals {
    pub entered: Notify,
}

pub struct HostFixture {
    pub runtime: Runtime<support::App>,
    pub clock: support::Manual,
    pub signals: Arc<Signals>,
    stored_changed: Arc<Notify>,
    pub stored: Arc<std::sync::Mutex<Vec<(harmos_runtime::streams::Definition, harmos_runtime::streams::PublicationRow)>>>,
    types: tempfile::TempDir,
    request: String,
    response: String,
}

struct Store(
    Arc<std::sync::Mutex<Vec<(harmos_runtime::streams::Definition, harmos_runtime::streams::PublicationRow)>>>,
    Arc<Notify>,
    harmos_runtime::storage::Cursor,
);

impl harmos_runtime::storage::StreamConsumer<support::App> for Store {
    fn resume(&self) -> harmos_runtime::storage::Cursor {
        self.2
    }
    async fn drain(&mut self, batch: harmos_runtime::storage::Batch<support::App>) -> harmos_runtime::storage::Cursor {
        self.0.lock().unwrap().extend(batch.publications().map(|(definition, _, row)| (definition.clone(), row.value.clone())));
        self.2 = batch.reaches();
        self.1.notify_waiters();
        self.2
    }
}

pub struct Sdk {
    pub address: String,
    process: tokio::process::Child,
    drain: JoinHandle<()>,
    proxy: JoinHandle<()>,
    disconnect: mpsc::Sender<oneshot::Sender<()>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::operations::OperationDeclaration for Host {
    type Request = Self;
    type Response = Reply;
    type Input = harmos_runtime::operations::NoStream;
    type Output = harmos_runtime::operations::NoStream;
    const ID: &'static str = "fixture.host";
    const VISIBILITY: harmos_runtime::operations::Visibility = harmos_runtime::operations::Visibility::Public;
    const GRPC: Option<harmos_runtime::operations::Grpc> = Some(harmos_runtime::operations::Grpc::new("fixture.host.v1", "Host", "Call"));
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    };
}

impl harmos_runtime::Dependent for Host {
    const RESOURCES: &'static [harmos_runtime::operations::Resource] = &[harmos_runtime::operations::Resource::of::<Signals>()];
}

impl harmos_runtime::operations::Requires<Signals> for Host {}

impl harmos_runtime::Operation for Host {
    type Error = std::convert::Infallible;
    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Reply>, Self::Error> {
        context.resource::<Signals>().entered.notify_one();
        Ok(Reply { value: self.value }.into())
    }
}

impl HostFixture {
    pub async fn new() -> Self {
        Self::with_clock(None).await
    }

    pub async fn with_clock(source: Option<harmos_runtime::clock::Clock>) -> Self {
        let clock = support::Manual::new();
        let signals = Arc::new(Signals::default());
        let stored_changed = Arc::new(Notify::new());
        let stored = Arc::new(std::sync::Mutex::new(Vec::new()));
        let runtime = crate::support::booted(Default::default(), |setup| {
            setup.settings().clock(source.unwrap_or_else(|| harmos_runtime::clock::Clock::with_clock(clock.clone())));
            setup.resource::<Signals>(signals.clone());
            setup.settings().attach_streams(Store(stored.clone(), stored_changed.clone(), harmos_runtime::storage::Cursor::ORIGIN));
        })
        .await;
        let registration = Registration::new(<crate::support::Lanes as harmos_runtime::operations::Lane>::default_lane())
            .operation::<Host>("fixture.host")
            .unwrap();
        runtime.register_batch(registration).await.unwrap();
        let contract = WireDeclaration::of::<Host>().contract().unwrap();
        let binding = contract.grpc().unwrap();
        let service = harmos_runtime::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, &contract)]).unwrap();
        let method = service.methods().find(|method| method.name() == "Call").unwrap();
        let request = method.input().full_name().to_owned();
        let response = method.output().full_name().to_owned();
        let types = tempfile::tempdir().unwrap();
        let descriptors = prost_types::FileDescriptorSet {
            file: service.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        std::fs::write(types.path().join("host.pb"), descriptors.encode_to_vec()).unwrap();
        Self {
            runtime,
            clock,
            signals,
            stored,
            stored_changed,
            types,
            request,
            response,
        }
    }

    pub async fn wait_rows(&self, count: usize) {
        loop {
            let changed = self.stored_changed.notified();
            if self.stored.lock().unwrap().len() >= count {
                return;
            }
            changed.await;
        }
    }

    pub fn rows(&self) -> Vec<harmos_runtime::streams::PublicationRow> {
        self.stored.lock().unwrap().iter().map(|(_, row)| row.clone()).collect()
    }

    pub fn values(&self) -> Vec<serde_json::Value> {
        self.stored
            .lock()
            .unwrap()
            .iter()
            .map(|(definition, row)| {
                let codec = harmos_domain::schema::json::JsonCodec::new(definition.schema().root().unwrap(), &Default::default()).unwrap();
                codec.decode(&row.payload).unwrap()
            })
            .collect()
    }

    pub async fn sdk(&self, mode: &str) -> Sdk {
        let mut process = tokio::process::Command::new(support::binary("harmos-sdk-invoke-fixture"))
            .args([mode, self.types.path().join("host.pb").to_str().unwrap(), &self.request, &self.response])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(process.stdout.take().unwrap()).lines();
        let upstream = loop {
            let line = stdout.next_line().await.unwrap().expect("SDK Readiness");
            if let Some(address) = line.strip_prefix("harmos listening ") {
                break address.to_owned();
            }
        };
        let drain = tokio::spawn(async move { while stdout.next_line().await.ok().flatten().is_some() {} });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (disconnect, mut commands) = mpsc::channel::<oneshot::Sender<()>>(1);
        let proxy = tokio::spawn(async move {
            let mut tunnels = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut downstream, _) = accepted.unwrap();
                        let upstream = upstream.clone();
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
        Sdk {
            address,
            process,
            drain,
            proxy,
            disconnect,
        }
    }
}

impl Sdk {
    pub async fn disconnect(&self) {
        let (reply, done) = oneshot::channel();
        self.disconnect.send(reply).await.unwrap();
        done.await.unwrap();
    }
}

impl Drop for Sdk {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
        self.drain.abort();
        self.proxy.abort();
    }
}

/* === Public Functions === */

pub fn payload(value: &[u8]) -> Vec<u8> {
    #[derive(prost::Message)]
    struct Value {
        #[prost(bytes = "vec", tag = "1")]
        value: Vec<u8>,
    }
    Value { value: value.into() }.encode_to_vec()
}

pub fn call_error(error: &harmos_runtime::Error) -> &harmos_runtime::plugin::grpc::CallError {
    match error {
        harmos_runtime::Error::OperationFailed { source, .. } => source.downcast_ref().expect("Protocol Call Failure"),
        error => panic!("Expected Protocol Failure: {error:?}"),
    }
}

/* === Private Functions === */

/* === Tests === */
