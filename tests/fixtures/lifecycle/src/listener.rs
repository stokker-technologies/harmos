/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use harmos::listeners::{Delivery, Source};
use harmos::operations::{
    Context, Declarations, OperationPolicy, PluginRegistration, RetryPolicy, Visibility,
    WireDeclaration,
};
use harmos::plugin::StreamDeclaration;
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions,
    PluginEndpoint, PluginLifecycle, PluginLifecycleAuthority, PluginManager, PluginTransition,
    Router,
};
use harmos::schema::Described;
use harmos::streams::{PluginListenerRegistration, StreamRegistration};
use harmos::{Error, Message, Result};
use schematic_app::{
    app::{Runtime, StreamManager},
    lanes::Lanes,
};

/* === Constants === */
const OWNER: &str = "shutdown-listener";
const STREAM: &str = "shutdown-listener.samples";

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<(), (), Backend>;

/* === Structs === */
#[harmos::message]
struct Sample {
    value: u32,
}

#[derive(Default)]
struct Trace {
    started: Notify,
    runs: AtomicUsize,
    finalized: AtomicUsize,
}

struct Receive {
    trace: Arc<Trace>,
}

#[derive(Default)]
struct Backend {
    endpoint: Option<PluginEndpoint>,
    operations: Option<PluginRegistration>,
    streams: Option<StreamRegistration>,
    listeners: Option<PluginListenerRegistration>,
}

struct Host {
    operations: harmos::operations::OperationManager,
    streams: StreamManager,
    trace: Arc<Trace>,
    publication: Option<Arc<harmos::streams::PublicationLease>>,
}

pub(super) struct Listener {
    manager: PluginManager<(), (), Backend>,
    host: Host,
}

/* === Enums === */
#[harmos::catalog]
enum Callbacks {
    Receive(Receive),
}

/* === Traits === */

/* === Implementations === */
#[harmos::operation(
    id = "shutdown-listener.receive",
    visibility = Visibility::Shared,
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(300),
        attempt_timeout: Duration::from_secs(300),
        retries: RetryPolicy::Never,
    }
)]
impl Receive {
    fn new(trace: Arc<Trace>) -> Self {
        Self { trace }
    }
    async fn run(
        &mut self,
        _: Context<'_>,
        sample: &mut Delivery<Sample>,
    ) -> std::result::Result<(), Error> {
        if sample.row().value != 0 {
            return Err(Error::NotReady {
                domain: "shutdown listener ordering",
            });
        }
        self.trace.runs.fetch_add(1, Ordering::SeqCst);
        self.trace.started.notify_one();
        std::future::pending().await
    }
    async fn finalize(&mut self) -> Result<()> {
        self.trace.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl PluginLifecycle for Host {
    type Guest = ();
    type Sidecar = ();
    type Embedded = Backend;
    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Backend::default()))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        _: &PluginDefinition,
        router: &Router,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(backend) = backend else {
            unreachable!("embedded exercise")
        };
        backend.endpoint = Some(router.enter(OWNER).map_err(failed)?);
        let scope = backend.endpoint.as_ref().unwrap().scope().unwrap();
        backend.operations = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register_wire_operation::<Receive>(Lanes::Open, self.trace.clone())
        })?);
        backend.streams = Some(
            self.streams
                .stage_plugin(scope, [publication()])
                .await
                .map_err(failed)?,
        );
        self.publication = Some(backend.streams.as_ref().unwrap().leases()[0].clone());
        backend.listeners = Some(self.streams.stage_listeners(
            scope,
            &self.operations,
            |bindings| bindings.publication(STREAM, 1, 0),
        )?);
        Ok(())
    }
    async fn stop(
        _: &PluginDefinition,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(backend) = backend else {
            unreachable!("embedded exercise")
        };
        if let Some(listeners) = &mut backend.listeners {
            listeners.shutdown().await?;
        }
        if let Some(operations) = &mut backend.operations {
            operations.shutdown().await?;
        }
        backend.listeners.take();
        backend.operations.take();
        backend.streams.take();
        backend.endpoint.take();
        Ok(())
    }
}

impl Listener {
    pub(super) fn new(runtime: &Runtime) -> Result<Self> {
        let callback = WireDeclaration::of::<ReceiveDeclaration>()
            .contract()
            .map_err(failed)?;
        let listener = harmos::listeners::Definition::new(
            harmos::Identifier::new("shutdown-listener.samples-received").map_err(failed)?,
            Source::Stream {
                id: STREAM.into(),
                schema_ordinal: 1,
                fields: Cow::Borrowed(Sample::FIELDS),
            },
            callback,
        )
        .map_err(failed)?;
        let definitions = PluginDefinitions::new(
            [PluginDefinition {
                id: OWNER.into(),
                version: "1.0.0".parse().map_err(failed)?,
                artifact: PluginArtifact::Embedded {
                    revision: "shutdown-exercise".into(),
                },
                requires: Vec::new(),
                configuration: b"null".to_vec(),
                contributions: PluginContributions {
                    operations: Declarations::of::<Callbacks>().map_err(failed)?,
                    listeners: vec![listener],
                    streams: vec![publication()],
                    ..PluginContributions::default()
                },
            }],
            [],
        )
        .map_err(failed)?;
        Ok(Self {
            manager: PluginManager::new(definitions)?,
            host: Host {
                operations: runtime.operations.clone(),
                streams: runtime.streams.clone(),
                trace: Arc::new(Trace::default()),
                publication: None,
            },
        })
    }
    pub(super) async fn start(&mut self) -> Result<()> {
        self.manager.initialize(&mut self.host).await?;
        let publication = self.host.publication.as_ref().unwrap();
        publication
            .publish(Sample { value: 0 }.encode_to_vec(), 0)
            .map_err(failed)?;
        self.host.trace.started.notified().await;
        for value in 1..4 {
            publication
                .publish(Sample { value }.encode_to_vec(), 0)
                .map_err(failed)?;
        }
        Ok(())
    }
    pub(super) async fn shutdown(&mut self) -> Result<()> {
        self.manager.shutdown(&mut self.host).await?;
        self.host.publication.take();
        Ok(())
    }
    pub(super) fn verify(&self) -> Result<()> {
        if self.host.trace.runs.load(Ordering::SeqCst) != 1
            || self.host.trace.finalized.load(Ordering::SeqCst) != 1
        {
            return Err(Error::NotReady {
                domain: "shutdown listener cleanup",
            });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn publication() -> StreamDeclaration {
    StreamDeclaration {
        metadata: harmos::Metadata::new(harmos::Identifier::new(STREAM).unwrap(), "Samples", None),
        row_metadata: Vec::new(),
        id: STREAM.into(),
        schema: 1,
        window: 4,
        fields: Sample::FIELDS.to_vec(),
    }
}

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: OWNER.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
