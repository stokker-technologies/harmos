/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../fixtures/process_provider.rs"]
mod provider;

#[path = "../fixtures/process_client.rs"]
mod client;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
// Third Party Imports
use contract::{Marks, POLICY};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::Notify;
// Local Imports
use super::graph::{contract, marked};
use super::{HostLanes, assembly, operations, service_status};
use crate::support::Bank;
use harmos::operations::OperationPolicy;
use harmos::sidecar::{Attach, Attached, Notice, Refusal, Supervised, Supervisor};
use harmos::{Runtime, services::Service, services::ServiceStatus};
use harmos_sidecar::attach::{End, Ending};

/* === Constants === */

/* === Macros === */
macro_rules! forwarded {
    ($self:ident, $method:ident) => {
        match &$self.source {
            Source::Provider(source) => source.$method(),
            Source::Client(source) => source.$method(),
            Source::Independent(source) => source.$method(),
        }
    };
}

/* === Type Aliases === */
type Wait = Pin<Box<dyn Future<Output = ()> + Send>>;

/* === Structs === */
#[derive(Default)]
struct Gate {
    attached: AtomicUsize,
    starts: AtomicUsize,
    failures: AtomicUsize,
    joined: AtomicBool,
    finished: Notify,
    hold: AtomicBool,
    fail: AtomicBool,
    entered: Notify,
    release: Arc<Notify>,
    cut: Arc<Notify>,
}

#[derive(Clone)]
struct Joined {
    source: Source,
    gate: Arc<Gate>,
}

struct GatedEnding {
    original: Ending,
    gate: Arc<Gate>,
    started: bool,
    waiting: Option<Wait>,
}

struct CutRead {
    original: Box<dyn AsyncRead + Send + Unpin>,
    cut: Wait,
    closed: bool,
}

struct Independent {
    resources: Arc<Marks>,
}

struct Fixture {
    runtime: Runtime<Bank>,
    _lease: harmos::operations::OperationInstance<harmos::operations::resident::Activation>,
    root: tempfile::TempDir,
    client: Arc<Gate>,
    provider: Arc<Gate>,
    independent: Arc<Gate>,
}

/* === Enums === */
#[derive(Clone)]
enum Source {
    Provider(harmos_sidecar::PluginNative<provider::Provider>),
    Client(harmos_sidecar::PluginNative<client::Client>),
    Independent(harmos_sidecar::PluginNative<Independent>),
}

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Independent {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "process-independent", provides = (), requires = ())]
impl harmos::Plugin for Independent {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Marks;
    fn prepare(root: String) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Marks { root: root.into() }),
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        std::fs::create_dir_all(&self.resources.root).map_err(Refusal::of)?;
        self.resources.mark("initialized")
    }
    fn resources(&self) -> Arc<Marks> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos::operations::WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.mark("shutdown")
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.mark("cleaned")
    }
}

impl AsyncRead for CutRead {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let reader = self.get_mut();
        if reader.closed || reader.cut.as_mut().poll(context).is_ready() {
            reader.closed = true;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut reader.original).poll_read(context, buffer)
    }
}

impl End for GatedEnding {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            if !self.started {
                self.started = true;
                self.gate.starts.fetch_add(1, Ordering::SeqCst);
                self.gate.entered.notify_one();
            }
            if self.gate.hold.load(Ordering::SeqCst) {
                if self.waiting.is_none() {
                    let release = self.gate.release.clone();
                    self.waiting = Some(Box::pin(async move { release.notified().await }));
                }
                self.waiting.as_mut().unwrap().await;
                self.waiting.take();
                self.gate.hold.store(false, Ordering::SeqCst);
            }
            if self.gate.fail.load(Ordering::SeqCst) {
                self.gate.failures.fetch_add(1, Ordering::SeqCst);
                return Err(std::io::Error::other(
                    "original fixture ending remains failed",
                ));
            }
            self.original.settle().await?;
            self.gate.joined.store(true, Ordering::SeqCst);
            self.gate.finished.notify_one();
            Ok(())
        })
    }
}

impl Attach for Joined {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        forwarded!(self, owner)
    }
    fn operations(&self) -> harmos_contract::Result<Vec<harmos::operations::Definition>> {
        forwarded!(self, operations)
    }
    fn operation_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos::operations::Contract>>> {
        forwarded!(self, operation_contracts)
    }
    fn required_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::inventory::contracts::Required>>> {
        forwarded!(self, required_contracts)
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_contract::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
        forwarded!(self, sdk_lifecycle)
    }
    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        forwarded!(self, publications)
    }
    fn observers(&self) -> Vec<harmos_contract::inventory::observers::Declaration> {
        forwarded!(self, observers)
    }
    fn observer_control(&self) -> harmos_contract::Result<Option<harmos::operations::Contract>> {
        forwarded!(self, observer_control)
    }
    fn listener_declarations(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::listeners::Definition>>> {
        forwarded!(self, listener_declarations)
    }
    async fn attach(&self) -> Result<Attached, Self::Error> {
        self.gate.attached.fetch_add(1, Ordering::SeqCst);
        let attached = match &self.source {
            Source::Provider(source) => source.attach().await?,
            Source::Client(source) => source.attach().await?,
            Source::Independent(source) => source.attach().await?,
        };
        let (write, read, control, diagnostics, original) = attached.into_parts();
        let ending = Ending::new(GatedEnding {
            original: original.expect("original SDK physical ending"),
            gate: self.gate.clone(),
            started: false,
            waiting: None,
        });
        let cut = self.gate.cut.clone();
        let read = CutRead {
            original: read,
            cut: Box::pin(async move { cut.notified().await }),
            closed: false,
        };
        let mut attached = Attached::new(write, read).ending(ending);
        if let Some(control) = control {
            attached = attached.control(control);
        }
        if let Some(diagnostics) = diagnostics {
            attached = attached.diagnostics(diagnostics);
        }
        Ok(attached)
    }
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let capacity = NonZeroUsize::new(2).unwrap();
        let provider = Arc::new(Gate::default());
        let client = Arc::new(Gate::default());
        let independent = Arc::new(Gate::default());
        let sources = [
            Joined {
                source: Source::Provider(harmos_sidecar::PluginNative::new(capacity).unwrap()),
                gate: provider.clone(),
            },
            Joined {
                source: Source::Client(harmos_sidecar::PluginNative::new(capacity).unwrap()),
                gate: client.clone(),
            },
            Joined {
                source: Source::Independent(harmos_sidecar::PluginNative::new(capacity).unwrap()),
                gate: independent.clone(),
            },
        ];
        let definitions = sources
            .iter()
            .map(|source| {
                let path = root.path().join(source.owner());
                operations::definition(source, serde_json::to_vec(path.to_str().unwrap()).unwrap())
            })
            .collect::<Vec<_>>();
        let encoding_bytes = definitions
            .iter()
            .map(|definition| definition.configuration.len() * 2)
            .max()
            .unwrap()
            + std::mem::size_of::<harmos::operations::lifecycle::InitializeRequest>()
            + 84;
        assert!(encoding_bytes <= 512);
        let definitions = harmos::plugin::PluginDefinitions::new(definitions, []).unwrap();
        let residents =
            sources.map(|source| Supervised::new(source, notices as fn(&Service<Bank>, Notice)));
        let runtime = assembly()
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        runtime
            .operations
            .register_service(
                harmos::services::ServiceDefinition {
                    id: "local-graph".into(),
                    policy: harmos::operations::OperationPolicy {
                        restart: harmos::operations::RestartPolicy::Never,
                        ..harmos::services::POLICY
                    },
                },
                move |_: Arc<()>| {
                    harmos::sidecar::Managed::new_graph(
                        residents.clone(),
                        definitions.clone(),
                        HostLanes::Open,
                        capacity,
                        NonZeroUsize::new(encoding_bytes).unwrap(),
                    )
                    .unwrap()
                },
            )
            .unwrap();
        let lease = runtime
            .operations
            .submit_resident("local-graph")
            .await
            .unwrap();
        service_status(
            runtime.operations.watch_residents(),
            "local-graph",
            |status| matches!(status, ServiceStatus::Running { .. }),
        )
        .await;
        Self {
            runtime,
            _lease: lease,
            root,
            client,
            provider,
            independent,
        }
    }
    fn path(&self, owner: &str) -> std::path::PathBuf {
        self.root.path().join(owner)
    }
    fn settled(&self) {
        for gate in [&self.client, &self.provider, &self.independent] {
            assert_eq!(gate.attached.load(Ordering::SeqCst), 1);
            assert_eq!(gate.starts.load(Ordering::SeqCst), 1);
            assert!(gate.joined.load(Ordering::SeqCst));
        }
        for owner in ["process-provider", "process-client", "process-independent"] {
            assert_eq!(count(&self.path(owner), "shutdown"), 1);
            assert_eq!(count(&self.path(owner), "cleaned"), 1);
        }
        assert_eq!(count(&self.path("process-provider"), "read-finalized-2"), 1);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

fn count(root: &Path, name: &str) -> usize {
    std::fs::read_to_string(root.join(name))
        .unwrap()
        .lines()
        .count()
}

#[tokio::test]
async fn cancelled_stop_retains() {
    let fixture = Fixture::new().await;
    fixture.client.hold.store(true, Ordering::SeqCst);
    {
        let stop = fixture.runtime.stop();
        tokio::pin!(stop);
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                result = &mut stop => panic!("stop returned before original ending: {result:?}"),
                _ = async {
                    fixture.client.entered.notified().await;
                    fixture.independent.finished.notified().await;
                } => {}
            }
        })
        .await
        .unwrap();
    }
    assert!(fixture.independent.joined.load(Ordering::SeqCst));
    assert!(!fixture.client.joined.load(Ordering::SeqCst));
    assert!(!fixture.provider.joined.load(Ordering::SeqCst));
    assert!(!fixture.path("process-provider").join("shutdown").exists());
    assert_eq!(
        count(&fixture.path("process-provider"), "read-finalized-2"),
        1
    );
    fixture.client.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap()
        .unwrap();
    fixture.settled();
}

#[tokio::test]
async fn failed_ending_retires() {
    let fixture = Fixture::new().await;
    fixture.client.fail.store(true, Ordering::SeqCst);
    let result = tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
    assert!(!fixture.client.joined.load(Ordering::SeqCst));
    assert!(!fixture.provider.joined.load(Ordering::SeqCst));
    assert!(!fixture.path("process-provider").join("shutdown").exists());
    assert!(fixture.independent.joined.load(Ordering::SeqCst));
    assert_eq!(count(&fixture.path("process-independent"), "cleaned"), 1);
    let failures = fixture.client.failures.load(Ordering::SeqCst);
    assert!(failures > 0);
    let result = tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
    assert!(fixture.client.failures.load(Ordering::SeqCst) > failures);
    assert_eq!(fixture.client.starts.load(Ordering::SeqCst), 1);
    assert_eq!(count(&fixture.path("process-independent"), "cleaned"), 1);
    assert!(!fixture.path("process-provider").join("shutdown").exists());
    let failures = fixture.client.failures.load(Ordering::SeqCst);
    fixture.client.fail.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap()
        .unwrap();
    fixture.settled();
    assert_eq!(fixture.client.failures.load(Ordering::SeqCst), failures);
}

#[tokio::test]
async fn unrelated_failure_retires() {
    let fixture = Fixture::new().await;
    fixture.independent.fail.store(true, Ordering::SeqCst);
    let result = tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
    assert!(!fixture.independent.joined.load(Ordering::SeqCst));
    assert!(fixture.client.joined.load(Ordering::SeqCst));
    assert!(fixture.provider.joined.load(Ordering::SeqCst));
    assert_eq!(
        count(&fixture.path("process-provider"), "read-finalized-2"),
        1
    );
    for owner in ["process-client", "process-provider"] {
        assert_eq!(count(&fixture.path(owner), "shutdown"), 1);
        assert_eq!(count(&fixture.path(owner), "cleaned"), 1);
    }
    fixture.independent.fail.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap()
        .unwrap();
    fixture.settled();
}

#[tokio::test]
async fn eof_retains_child() {
    let fixture = Fixture::new().await;
    let provider = fixture.path("process-provider");
    std::fs::write(provider.join("hold-read-2"), []).unwrap();
    fixture.provider.hold.store(true, Ordering::SeqCst);
    {
        let stop = fixture.runtime.stop();
        tokio::pin!(stop);
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                result = &mut stop => panic!("stop returned before child request: {result:?}"),
                _ = marked(&provider, "read-started-2") => {}
            }
            fixture.provider.cut.notify_one();
            tokio::select! {
                result = &mut stop => panic!("stop returned before physical ending: {result:?}"),
                _ = fixture.provider.entered.notified() => {}
            }
        })
        .await
        .unwrap();
        assert!(!fixture.provider.joined.load(Ordering::SeqCst));
        assert!(!fixture.client.joined.load(Ordering::SeqCst));
        assert!(
            !fixture
                .path("process-client")
                .join("dependency-shutdown")
                .exists()
        );
        // Cancel Only The Wait; The Same Original Child And Ending Remain Owned.
    }
    fixture.provider.fail.store(true, Ordering::SeqCst);
    fixture.provider.release.notify_one();
    let failed = tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
        .await
        .unwrap();
    assert!(matches!(
        failed,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
    assert!(fixture.provider.failures.load(Ordering::SeqCst) > 0);
    assert!(!fixture.provider.joined.load(Ordering::SeqCst));
    assert!(!fixture.client.joined.load(Ordering::SeqCst));
    fixture.provider.fail.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        let result = tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
            .await
            .unwrap();
        assert!(
            result.is_ok()
                || matches!(
                    result,
                    Err(harmos::Error::TaskFailed {
                        domain: "service",
                        ..
                    })
                )
        );
        if fixture.provider.joined.load(Ordering::SeqCst)
            && fixture.client.joined.load(Ordering::SeqCst)
        {
            break;
        }
    }
    for gate in [&fixture.client, &fixture.provider, &fixture.independent] {
        assert!(gate.joined.load(Ordering::SeqCst));
        assert_eq!(gate.attached.load(Ordering::SeqCst), 1);
        assert_eq!(gate.starts.load(Ordering::SeqCst), 1);
    }
    assert_eq!(count(&provider, "read-prepared-2"), 1);
    assert_eq!(count(&provider, "read-started-2"), 1);
    assert_eq!(count(&provider, "read-finalized-2"), 1);
    for owner in ["process-provider", "process-client", "process-independent"] {
        assert_eq!(count(&fixture.path(owner), "cleaned"), 1);
    }
}

/* === Tests === */
