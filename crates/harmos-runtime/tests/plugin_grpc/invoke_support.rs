/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// SDK Invoke Story Mechanics

/* === Imports === */

// Standard Library Imports
use std::{
    convert::Infallible,
    fs,
    future::{self, Future},
    num::NonZeroUsize,
    path::Path,
    pin::Pin,
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::Duration,
};

// Third Party Imports
use prost::Message;
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    process,
    sync::{Notify, mpsc, oneshot},
    task::{JoinHandle, JoinSet},
};

// Local Imports
use super::support;
use harmos_operations::operations::{OperationSpec, RequestShape, WireDeclaration};
use harmos_runtime::{
    Error, Resource, Response, Runtime, Scope,
    clock::Clock,
    grpc, operations,
    operations::{OperationPolicy, Registration, RetryPolicy},
    plugin, storage, streams,
};

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
    pub publications: Mutex<Option<Vec<(streams::Definition, streams::ManagedPublication)>>>,
    pub gaps: Mutex<Vec<(streams::Definition, streams::Gap)>>,
    pub incomplete: Mutex<Vec<streams::Definition>>,
    pub storage_hold: AtomicBool,
    pub storage_entered: Notify,
    pub storage_release: Notify,
    pub storage_ingested: Notify,
}

pub struct HostFixture {
    pub runtime: Runtime<support::App>,
    pub clock: support::Manual,
    pub signals: Arc<Signals>,
    pub stored: Arc<Mutex<Vec<(streams::Definition, streams::PublicationRow)>>>,
    types: tempfile::TempDir,
    request: String,
    response: String,
}

struct Store(Arc<Mutex<Vec<(streams::Definition, streams::PublicationRow)>>>, storage::Cursor, Arc<Signals>);

impl storage::StreamConsumer<support::App> for Store {
    fn resume(&self) -> storage::Cursor {
        self.1
    }
    async fn drain(&mut self, batch: storage::Batch<support::App>) -> storage::Cursor {
        if self.2.storage_hold.swap(false, Ordering::SeqCst) {
            self.2.storage_entered.notify_one();
            self.2.storage_release.notified().await;
        }
        self.2.gaps.lock().unwrap().extend(batch.gaps().map(|(definition, gap)| (definition.clone(), gap.clone())));
        self.2.incomplete.lock().unwrap().extend_from_slice(batch.incomplete());
        let publications =
            batch.publications().map(|(definition, _, row)| (definition.clone(), row.value.row().clone())).collect::<Vec<_>>();
        if let Some(publications) = self.2.publications.lock().unwrap().as_mut() {
            publications.extend(batch.publications().map(|(definition, _, row)| (definition.clone(), row.value.clone())));
        }
        let published = !publications.is_empty();
        self.0.lock().unwrap().extend(publications);
        self.1 = batch.reaches();
        if published {
            self.2.storage_ingested.notify_one();
        }
        self.1
    }
}

pub struct Python {
    pub address: String,
    process: process::Child,
}

pub struct Sdk {
    pub address: String,
    process: process::Child,
    proxy: JoinHandle<()>,
    disconnect: mpsc::Sender<oneshot::Sender<()>>,
}

/* === Implementations === */

impl RequestShape for Host {
    type Request = Self;
    type Response = Reply;
    type Input = operations::NoStream;
    type Output = operations::NoStream;
}
static HOST_DESCRIPTION: WireDeclaration = WireDeclaration::request::<Host>(
    "fixture.host",
    OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(operations::Visibility::Public)
.grpc(operations::Grpc::new("fixture.host.v1", "Host", "Call"));

const HOST_OPERATION: OperationSpec<Host> = OperationSpec::<Host>::new(&HOST_DESCRIPTION);

pub(crate) async fn host_handler(request: Host, context: operations::FunctionContext<Host>) -> Result<Response<Reply>, Infallible> {
    context.resource::<Signals>().expect("declared resource is registered").entered.notify_one();
    let value = if request.value == b"scope" { context.call().get::<Scope>().unwrap().as_str().as_bytes().to_vec() } else { request.value };
    Ok(Reply { value }.into())
}

pub(crate) fn host_operation() -> Result<operations::OperationFactory, operations::ServingError> {
    operations::OperationFactory::function(HOST_OPERATION, const { &[Resource::of::<Signals>()] }, host_handler, |_| {
        operations::FailureKind::Permanent
    })
}

impl HostFixture {
    pub async fn new() -> Self {
        Self::with_clock(None).await
    }

    pub async fn with_clock(source: Option<Clock>) -> Self {
        Self::configured(source, NonZeroUsize::new(4096).unwrap(), None).await
    }

    pub async fn with_middleware(middleware: Arc<dyn operations::middleware::Middleware>) -> Self {
        Self::configured(None, NonZeroUsize::new(4096).unwrap(), Some(middleware)).await
    }

    pub(super) async fn configured(
        source: Option<Clock>, preparation_bytes: NonZeroUsize, middleware: Option<Arc<dyn operations::middleware::Middleware>>,
    ) -> Self {
        let clock = support::Manual::new();
        let signals = Arc::new(Signals::default());
        let stored = Arc::new(Mutex::new(Vec::new()));
        let runtime = crate::support::booted(Default::default(), |setup| {
            if let Some(middleware) = middleware {
                setup.settings().operation_middleware(middleware);
            }
            setup.settings().clock(source.unwrap_or_else(|| Clock::with_clock(clock.clone())));
            setup.resource::<Signals>(signals.clone());
            setup.settings().operation_settings(operations::Settings {
                preparation_bytes: Some(preparation_bytes),
                ..Default::default()
            });
            setup.settings().attach_streams(Store(stored.clone(), storage::Cursor::ORIGIN, signals.clone()));
        })
        .await;
        let registration = Registration::new(operations::Lane::default())
            .operation(host_operation().expect("representable operation"), "fixture.host")
            .unwrap();
        runtime.register_batch(registration).await.unwrap();
        let contract = HOST_OPERATION.wire().contract().unwrap();
        let binding = contract.grpc().unwrap();
        let service = grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, &contract)]).unwrap();
        let method = service.methods().find(|method| method.name() == "Call").unwrap();
        let request = method.input().full_name().to_owned();
        let response = method.output().full_name().to_owned();
        let types = tempfile::tempdir().unwrap();
        fs::write(types.path().join("host.pb"), service.parent_pool().encode_to_vec()).unwrap();
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

    /// Watch a Plugin Stream: While a Watch Is Held, the Activation's Unscoped Demand Grants Its Windows.
    pub fn watch(&self, stream: &str) -> impl futures_core::Stream<Item = streams::Item<streams::PublicationRow>> + Send + Unpin + use<> {
        self.runtime.streams.current_publication(stream).expect("an attached plugin stream").watch()
    }

    pub fn rows(&self) -> Vec<streams::PublicationRow> {
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

    /// Every Interval Sample in Storage Order: Its Time in Unix Nanoseconds and Its Row Built From the Batch Columns.
    pub fn samples(&self) -> Vec<(i64, serde_json::Value)> {
        Self::expand(self.values())
    }

    /// The Samples of One Stream, as [`Self::samples`] Returns Them.
    pub fn samples_of(&self, stream: &str) -> Vec<(i64, serde_json::Value)> {
        Self::expand(self.stream_values(stream).into_iter().map(|(batch, _)| batch).collect())
    }

    /// One Stream's Stored Batches: First Sample Time, Sample Count and Scope.
    pub fn batches_of(&self, stream: &str) -> Vec<(i64, u64, Option<String>)> {
        self.stream_values(stream)
            .into_iter()
            .map(|(batch, context)| {
                let scope = context
                    .and_then(|context| serde_json::from_str::<serde_json::Value>(&context).ok()?["scope"].as_str().map(str::to_owned));
                (batch["start_unix_nanos"].as_i64().unwrap(), batch["count"].as_u64().unwrap(), scope)
            })
            .collect()
    }

    fn stream_values(&self, stream: &str) -> Vec<(serde_json::Value, Option<String>)> {
        self.stored
            .lock()
            .unwrap()
            .iter()
            .filter(|(definition, _)| definition.id() == stream)
            .map(|(definition, row)| {
                let codec = harmos_domain::schema::json::JsonCodec::new(definition.schema().root().unwrap(), &Default::default()).unwrap();
                (codec.decode(&row.payload).unwrap(), row.context.clone())
            })
            .collect()
    }

    fn expand(batches: Vec<serde_json::Value>) -> Vec<(i64, serde_json::Value)> {
        let mut samples = Vec::new();
        for batch in batches {
            let start = batch["start_unix_nanos"].as_i64().expect("an interval batch row");
            let period = batch["period_nanos"].as_i64().unwrap();
            let columns = batch["columns"].as_object().unwrap();
            for index in 0..batch["count"].as_u64().unwrap() as usize {
                let row = columns.iter().map(|(name, values)| (name.clone(), values[index].clone())).collect();
                samples.push((start + index as i64 * period, serde_json::Value::Object(row)));
            }
        }
        samples
    }

    pub fn managed_sdk(&self, mode: &str) -> plugin::grpc::Placement {
        plugin::grpc::Placement::Managed(plugin::grpc::Managed {
            executable: support::binary("harmos-sdk-invoke-fixture"),
            directory: None,
            arguments: vec![
                mode.into(),
                self.types.path().join("host.pb").into_os_string(),
                self.request.clone().into(),
                self.response.clone().into(),
                "--managed".into(),
            ],
            environment: Default::default(),
        })
    }

    pub async fn sdk(&self, mode: &str) -> Sdk {
        let (process, upstream) = support::announced(
            process::Command::new(support::binary("harmos-sdk-invoke-fixture"))
                .args([mode, self.types.path().join("host.pb").to_str().unwrap(), &self.request, &self.response])
                .stderr(Stdio::inherit()),
        )
        .await;
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
                            let _ = io::copy_bidirectional(&mut downstream, &mut upstream).await;
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
        Sdk { address, process, proxy, disconnect }
    }
}

impl Python {
    pub async fn start(arguments: &[&str]) -> Self {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/python");
        let python = sdk.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" });
        assert!(python.is_file(), "Python SDK Environment: Run mise run prepare");
        let (process, address) = support::announced(
            process::Command::new(python)
                .args(["-m", "tests.fixtures.conformance", "--address", "127.0.0.1:0", "--shutdown-on-stdin"])
                .args(arguments)
                .current_dir(sdk)
                .stdin(Stdio::piped())
                .stderr(Stdio::inherit()),
        )
        .await;
        Self {
            address: format!("http://{address}"),
            process,
        }
    }

    pub async fn close(mut self) -> ExitStatus {
        drop(self.process.stdin.take());
        self.process.wait().await.unwrap()
    }
}

impl Drop for Python {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
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
        self.proxy.abort();
    }
}

/* === Public Functions === */

pub async fn pending<F: Future>(mut future: Pin<&mut F>) -> bool {
    future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx).is_pending())).await
}

pub fn payload(value: &[u8]) -> Vec<u8> {
    #[derive(prost::Message)]
    struct Value {
        #[prost(bytes = "vec", tag = "1")]
        value: Vec<u8>,
    }
    Value { value: value.into() }.encode_to_vec()
}

pub fn call_error(error: &Error) -> &plugin::grpc::CallError {
    match error {
        Error::OperationFailed { source, .. } => source.downcast_ref().expect("Protocol Call Failure"),
        error => panic!("Expected Protocol Failure: {error:?}"),
    }
}
