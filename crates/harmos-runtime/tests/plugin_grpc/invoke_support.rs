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
    operations::{OperationPolicy, Registration, RetryPolicy},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default, Clone)]
pub struct Host {
    pub value: Vec<u8>,
}

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
pub struct Reply {
    pub value: Vec<u8>,
}

#[derive(Default)]
pub struct Signals {
    pub entered: Notify,
    pub storage_hold: std::sync::atomic::AtomicBool,
    pub storage_entered: Notify,
    pub storage_release: Notify,
}

pub struct HostFixture {
    pub runtime: Runtime<support::App>,
    pub clock: support::Manual,
    pub signals: Arc<Signals>,
    pub stored: Arc<std::sync::Mutex<Vec<(harmos_runtime::streams::Definition, harmos_runtime::streams::PublicationRow)>>>,
    types: tempfile::TempDir,
    request: String,
    response: String,
}

struct Store(
    Arc<std::sync::Mutex<Vec<(harmos_runtime::streams::Definition, harmos_runtime::streams::PublicationRow)>>>,
    harmos_runtime::storage::Cursor,
    Arc<Signals>,
);

impl harmos_runtime::storage::StreamConsumer<support::App> for Store {
    fn resume(&self) -> harmos_runtime::storage::Cursor {
        self.1
    }
    async fn drain(&mut self, batch: harmos_runtime::storage::Batch<support::App>) -> harmos_runtime::storage::Cursor {
        if self.2.storage_hold.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.2.storage_entered.notify_one();
            self.2.storage_release.notified().await;
        }
        self.0.lock().unwrap().extend(batch.publications().map(|(definition, _, row)| (definition.clone(), row.value.clone())));
        self.1 = batch.reaches();
        self.1
    }
}

pub struct Python {
    pub address: String,
    process: tokio::process::Child,
    drain: JoinHandle<()>,
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

impl harmos_operations::operations::RequestShape for Host {
    type Request = Self;
    type Response = Reply;
    type Input = harmos_runtime::operations::NoStream;
    type Output = harmos_runtime::operations::NoStream;
}
static HOST_DESCRIPTION: harmos_operations::operations::WireDeclaration = harmos_operations::operations::WireDeclaration::request::<Host>(
    "fixture.host",
    OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(harmos_runtime::operations::Visibility::Public)
.grpc(harmos_runtime::operations::Grpc::new("fixture.host.v1", "Host", "Call"));

const HOST_OPERATION: harmos_operations::operations::OperationSpec<Host> =
    harmos_operations::operations::OperationSpec::<Host>::new(&HOST_DESCRIPTION);

pub(crate) async fn host_handler(
    request: Host, context: harmos_runtime::operations::FunctionContext<Host>,
) -> Result<harmos_runtime::Response<Reply>, std::convert::Infallible> {
    context.resource::<Signals>().expect("declared resource is registered").entered.notify_one();
    Ok(Reply { value: request.value }.into())
}

pub(crate) fn host_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        HOST_OPERATION,
        const { &[harmos_runtime::Resource::of::<Signals>()] },
        host_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl HostFixture {
    pub async fn new() -> Self {
        Self::with_clock(None).await
    }

    pub async fn with_clock(source: Option<harmos_runtime::clock::Clock>) -> Self {
        let clock = support::Manual::new();
        let signals = Arc::new(Signals::default());
        let stored = Arc::new(std::sync::Mutex::new(Vec::new()));
        let runtime = crate::support::booted(Default::default(), |setup| {
            setup.settings().clock(source.unwrap_or_else(|| harmos_runtime::clock::Clock::with_clock(clock.clone())));
            setup.resource::<Signals>(signals.clone());
            setup.settings().attach_streams(Store(stored.clone(), harmos_runtime::storage::Cursor::ORIGIN, signals.clone()));
        })
        .await;
        let registration = Registration::new(harmos_runtime::operations::Lane::default())
            .operation(host_operation().expect("representable operation"), "fixture.host")
            .unwrap();
        runtime.register_batch(registration).await.unwrap();
        let contract = HOST_OPERATION.wire().contract().unwrap();
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
            types,
            request,
            response,
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

impl Python {
    pub async fn start(arguments: &[&str]) -> Self {
        let sdk = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/python");
        let python = sdk.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" });
        let mut process = tokio::process::Command::new(python)
            .args(["-m", "harmos.conformance", "--address", "127.0.0.1:0", "--shutdown-on-stdin"])
            .args(arguments)
            .current_dir(sdk)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("Python SDK Environment: Run mise run prepare");
        let mut stdout = BufReader::new(process.stdout.take().unwrap()).lines();
        let address = loop {
            let line = stdout.next_line().await.unwrap().expect("Python SDK Readiness");
            if let Some(address) = line.strip_prefix("harmos listening ") {
                break format!("http://{address}");
            }
        };
        let drain = tokio::spawn(async move { while stdout.next_line().await.ok().flatten().is_some() {} });
        Self { address, process, drain }
    }

    pub async fn close(mut self) -> std::process::ExitStatus {
        drop(self.process.stdin.take());
        self.process.wait().await.unwrap()
    }
}

impl Drop for Python {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
        self.drain.abort();
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

pub async fn pending<F: std::future::Future>(mut future: std::pin::Pin<&mut F>) -> bool {
    std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx).is_pending())).await
}

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
