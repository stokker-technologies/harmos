/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod boot;
mod lifecycle;

/* === Re-Exports === */

pub use boot::{BootstrapCleanup, BootstrapStatus, Startup};

/* === Imports === */

// Standard Library Imports
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::Instrument;
// Local Imports
use crate::evidence::{Declaration, Evidence, Vault};
use crate::services::{Once as DeclaredService, ServiceDefinition};
use crate::state::dispatch::Dispatch;
use crate::state::{Feed, Inverting, Origin, ReadRequest, Tail, Undos, replay};
use crate::storage::attachments::{Attachments, Consumers, Horizon, Payload, Source, declare};
use crate::storage::consumer::{JournalConsumer, Series, StreamConsumer};
use crate::streams::{Definition, Holding, Request, StreamManager, Table, Tier};
use crate::supervision::executor::InContext;
use crate::{
    Application, DefinitionId, Error, operations::LaneSet, services::Resident, state::At, state::CaptureLimits, state::DecodeError,
    state::Entry, state::JournalCapture, state::Kind, state::Position, state::StateManager, storage::Cursor,
};

/* === Constants === */

const REQUESTS: usize = 64;

const STOP_GRACE: Duration = Duration::from_millis(100);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Inspection<A: Application> {
    state: Arc<At<A::State>>,
    reads: crate::state::Reads<A>,
}

pub struct Inspector<A: Application> {
    pub state: Inspection<A>,
    source: JoinHandle<()>,
}

pub struct Runtime<A: Application> {
    pub state: StateManager<A>,
    pub records: crate::records::RecordManager<A>,
    pub streams: StreamManager<A>,
    pub operations: crate::operations::OperationManager<A::Lanes>,

    #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
    pub plugin: crate::plugin::PluginHost<A>,

    #[cfg(feature = "grpc")]
    grpc: crate::grpc::GrpcManager,

    pub evidence: Evidence<A>,
    stopping: Mutex<Stopping<A>>,
    closing: AtomicBool,
    storage: Arc<OnceLock<crate::storage::StorageManager<crate::storage::ErasedCatalog>>>,
    journal_handles: Weak<()>,
    participants: Participants,
    notices: watch::Sender<Option<String>>,
}

pub struct Handle<A: Application> {
    pub state: StateManager<A>,
    pub records: crate::records::RecordManager<A>,
    pub streams: StreamManager<A>,
    pub operations: crate::operations::OperationManager<A::Lanes>,

    #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
    pub plugin: crate::plugin::PluginHost<A>,

    #[cfg(feature = "grpc")]
    grpc: crate::grpc::GrpcManager,

    pub evidence: Evidence<A>,
    storage: Arc<OnceLock<crate::storage::StorageManager<crate::storage::ErasedCatalog>>>,
}

pub struct Builder<A: Application> {
    origin: Origin<A::State>,
    resources: Arc<A::Resources>,
    inverses: Inverting<A>,
    catalog: Dispatch<A, Payload<A>>,
    definitions: Vec<Definition>,
    attachments: Attachments<A>,
    managed_storage: Option<crate::config::ManagedStorageSettings>,
    record_projection: Option<Box<dyn crate::storage::RecordProjection>>,
    reconciliation: Option<boot::Reconciliation<A>>,
    initializers: Vec<boot::Initializer<A>>,
    snapshots: Option<crate::state::snapshot::Snapshots<A>>,
    lanes: LaneSet,
    operations: Vec<crate::operations::CatalogBinding>,
    operation_settings: crate::operations::Settings,
    schema_settings: Arc<crate::config::SchemaSettings>,
    operation_middleware: Vec<Arc<dyn crate::operations::middleware::Middleware>>,
    services: Vec<DeclaredService<A>>,
    evidence: Option<Declaration>,

    #[cfg(feature = "simulation")]
    simulation: crate::simulation::Configuration<A>,

    #[cfg(feature = "simulation")]
    live_only: Vec<&'static str>,
}

#[derive(Debug, thiserror::Error)]
#[error("{cause}")]
pub struct Incompatible {
    #[source]
    cause: Cause,

    cleanup: Option<boot::Cleanup>,
}

struct Supervision<A: Application> {
    consumers: Consumers<A>,
    feed: Feed<A>,
    progress: watch::Receiver<Position>,
    streams: StreamManager<A>,
    delivery: watch::Receiver<()>,
    demand: Vec<Holding>,
    writer: JoinHandle<()>,
    catalog: Arc<Dispatch<A, Payload<A>>>,
    inverses: Inverting<A>,
    requests: mpsc::Receiver<ReadRequest<A>>,
    served: mpsc::Receiver<Request>,
    ready: oneshot::Sender<()>,
}

#[derive(Clone)]
struct Participants {
    stopping: watch::Sender<bool>,
    active: watch::Sender<usize>,
}

pub struct StopParticipant {
    stopping: watch::Receiver<bool>,
    active: watch::Sender<usize>,
    released: bool,
}

/* === Enums === */

enum Stopping<A: Application> {
    Running(Option<JoinHandle<Position>>),
    Closing {
        cleanup: Arc<crate::supervision::cleanup::Cleanup<lifecycle::Shutdown<A>>>,
        outcome: Arc<OnceLock<lifecycle::Outcome>>,
        progress: watch::Sender<&'static str>,
    },
}

#[allow(clippy::type_complexity)]
pub enum Catalog<A: Application> {
    Transactions {
        definitions: Vec<(Kind, DefinitionId, u32)>,
        decode: fn(&DefinitionId, u32, &[u8]) -> Result<Option<A::Transaction>, DecodeError>,
        inverses: Option<fn(&A::Transaction, &A::State) -> Option<A::Transaction>>,
    },
    Streams(Vec<Definition>),
    Lanes(LaneSet),
}

#[derive(Debug, thiserror::Error)]
enum Cause {
    #[error("the stored history cannot be replayed by this build: {0}")]
    Undeclared(DecodeError),

    #[error("the stored history cannot be replayed by this build: {0}")]
    Unreadable(String),

    #[error("the stream catalog cannot be honored: {0}")]
    Dishonorable(crate::streams::Refused),

    #[error("the work declarations cannot be honored: {0}")]
    Work(String),

    #[error("the operation declarations cannot be honored: {0}")]
    Operations(#[source] crate::Error),

    #[error("runtime initialization failed: {0}")]
    Initialization(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("{source}; bootstrap cleanup also failed: {failure}")]
    Cleanup {
        #[source]
        source: Box<Incompatible>,

        failure: crate::Error,
    },

    #[error("application storage reconciliation failed: {0}")]
    Reconciliation(String),

    #[error("the evidence declarations cannot be honored: {0}")]
    Kept(String),

    #[cfg(feature = "simulation")]
    #[error("this build cannot be simulated after attaching {0}")]
    Simulation(String),
}

enum Turn<A: Application> {
    Advanced { journal: bool, streams: bool },
    Read(ReadRequest<A>),
    Serve(Box<Request>),
    Stopped,
}

/* === Traits === */

pub trait Registration<A: Application>: Clone + Send {
    fn register(self) -> Catalog<A>;
}

/* === Implementations === */

impl<A, S, T> Registration<A> for harmos_domain::state::TransactionDefinitions<S, T>
where
    A: Application<State = S, Transaction = T>,
{
    fn register(self) -> Catalog<A> {
        Catalog::Transactions {
            definitions: self.definitions,
            decode: self.decode,
            inverses: Some(self.inverses),
        }
    }
}

impl<A, R> Registration<A> for harmos_domain::state::RecordDefinitions<R>
where
    A: Application<Record = R>,
{
    fn register(self) -> Catalog<A> {
        Catalog::Transactions {
            definitions: self.definitions,
            decode: |_, _, _| Ok(None),
            inverses: None,
        }
    }
}

impl<A: Application> Inspection<A> {
    pub async fn read<R>(&self, read: impl FnOnce(&A::State) -> R) -> Result<At<R>, Error> {
        Ok(At {
            value: read(&self.state.value),
            position: self.state.position,
        })
    }

    pub async fn state_at(&self, origin: Origin<A::State>, at: Position) -> Result<At<A::State>, Error> {
        self.reads.state(origin, at).await
    }

    pub async fn query<X>(&self, after: Position) -> impl Iterator<Item = Result<Entry<X>, Error>>
    where
        X: Clone + Send + 'static,
    {
        self.query_through(after, self.state.position).await
    }

    pub async fn query_through<X>(&self, after: Position, through: Position) -> impl Iterator<Item = Result<Entry<X>, Error>>
    where
        X: Clone + Send + 'static,
    {
        if through > self.state.position {
            return vec![Err(Error::FrontierAhead)].into_iter();
        }
        if after >= through {
            return Vec::new().into_iter();
        }

        match self.reads.entries(after, through).await {
            Ok(entries) => entries
                .into_iter()
                .filter_map(|entry| {
                    entry.value.select::<X>().map(|value| {
                        Ok(Entry {
                            prepared: None,
                            position: entry.position,
                            metadata: entry.metadata,
                            value,
                        })
                    })
                })
                .collect::<Vec<_>>()
                .into_iter(),
            Err(refusal) => vec![Err(refusal)].into_iter(),
        }
    }

    pub fn applied(&self) -> Position {
        self.state.position
    }

    pub fn stored(&self) -> Position {
        self.state.position
    }
}

impl<A: Application> Drop for Inspector<A> {
    fn drop(&mut self) {
        self.source.abort();
    }
}

// Hand-written, because a derived `Clone` would demand `A: Clone` of an
// application that is only ever a marker.
impl<A: Application> Clone for Handle<A> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            records: self.records.clone(),
            streams: self.streams.clone(),
            operations: self.operations.clone(),
            #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
            plugin: self.plugin.clone(),
            #[cfg(feature = "grpc")]
            grpc: self.grpc.clone(),
            evidence: self.evidence.clone(),
            storage: self.storage.clone(),
        }
    }
}

impl<A: Application> Handle<A> {
    #[cfg(feature = "grpc")]
    pub fn grpc(&self) -> &crate::grpc::GrpcManager {
        &self.grpc
    }
}

impl<A: Application> Runtime<A> {
    #[cfg(feature = "grpc")]
    pub fn grpc(&self) -> &crate::grpc::GrpcManager {
        &self.grpc
    }

    pub fn builder(origin: impl Into<Origin<A::State>>, resources: impl Into<Arc<A::Resources>>) -> Builder<A> {
        Builder {
            origin: origin.into(),
            resources: resources.into(),
            inverses: none::<A>,
            catalog: Dispatch::default(),
            definitions: Vec::new(),
            attachments: Attachments::new(),
            managed_storage: None,
            record_projection: None,
            reconciliation: None,
            initializers: Vec::new(),
            snapshots: None,
            lanes: LaneSet::default(),
            operations: Vec::new(),
            operation_settings: crate::operations::Settings::default(),
            schema_settings: Arc::new(crate::config::SchemaSettings::default()),
            operation_middleware: Vec::new(),
            services: Vec::new(),
            evidence: None,
            #[cfg(feature = "simulation")]
            simulation: crate::simulation::Configuration::new(),
            #[cfg(feature = "simulation")]
            live_only: Vec::new(),
        }
    }

    pub async fn inspect(origin: impl Into<Origin<A::State>>, source: impl Source<A>) -> Result<Inspector<A>, Incompatible> {
        let mut attachments = Attachments::new();
        attachments.recover(source);
        let mut consumers = attachments.supervised();
        let recovery = consumers.load().await?;
        let origin = origin.into();
        let anchor = origin.position;
        let mut state = At { value: origin.value, position: anchor };
        let entries = recovery.entries.into_iter().filter(|entry| entry.position > anchor).collect();
        replay(None, entries, &mut state, &mut Undos::default(), none::<A>, 0).map_err(Incompatible::of)?;
        state.position = state.position.max(recovery.frontier).max(anchor);

        let (reads, requests) = crate::state::Reads::channel(REQUESTS);
        let source = tokio::spawn(inspect_reads(consumers, requests));

        Ok(Inspector {
            state: Inspection { state: Arc::new(state), reads },
            source,
        })
    }

    pub fn stop_participant(&self) -> StopParticipant {
        self.participants.register()
    }

    pub fn notices(&self) -> watch::Receiver<Option<String>> {
        self.notices.subscribe()
    }

    pub fn handle(&self) -> Handle<A> {
        Handle {
            state: self.state.clone(),
            records: self.records.clone(),
            streams: self.streams.clone(),
            operations: self.operations.clone(),
            #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
            plugin: self.plugin.clone(),
            #[cfg(feature = "grpc")]
            grpc: self.grpc.clone(),
            evidence: self.evidence.clone(),
            storage: self.storage.clone(),
        }
    }
}

impl<A: Application> Registration<A> for Vec<Definition> {
    fn register(self) -> Catalog<A> {
        Catalog::Streams(self)
    }
}

impl<A: Application, const N: usize> Registration<A> for [Definition; N] {
    fn register(self) -> Catalog<A> {
        Catalog::Streams(self.into())
    }
}

impl<A: Application> Registration<A> for LaneSet {
    fn register(self) -> Catalog<A> {
        Catalog::Lanes(self)
    }
}

impl<A: Application> Builder<A> {
    /// Configures Schema Preparation For Runtime-Owned Record And Stream Registrations.
    pub fn schema_settings(mut self, settings: crate::config::SchemaSettings) -> Self {
        self.schema_settings = Arc::new(settings);
        self
    }

    pub fn register<C: Registration<A> + 'static>(mut self, catalog: C) -> Self {
        #[cfg(feature = "simulation")]
        self.simulation.register(catalog.clone());

        match catalog.register() {
            Catalog::Transactions { definitions, decode, inverses } => {
                if let Some(inverses) = inverses {
                    self.inverses = inverses;
                }
                for (kind, id, version) in definitions {
                    declare(&mut self.catalog, kind, id, version, decode);
                }
            }
            Catalog::Streams(definitions) => self.definitions.extend(definitions),
            Catalog::Lanes(declared) => self.lanes.extend(declared),
        }
        self
    }

    pub fn with_registration(self, registration: crate::operations::Registration<A::Lanes>) -> Self {
        let binding = crate::operations::CatalogBinding::new(registration);
        self.bind_operations(binding)
    }
    pub fn operation_middleware(mut self, middleware: Arc<dyn crate::operations::middleware::Middleware>) -> Self {
        #[cfg(feature = "simulation")]
        self.simulation.operation_middleware(middleware.clone());
        self.operation_middleware.push(middleware);
        self
    }

    pub fn operation_settings(mut self, settings: crate::operations::Settings) -> Self {
        #[cfg(feature = "simulation")]
        self.simulation.operation_settings(settings);
        self.operation_settings = settings;
        self
    }

    pub(crate) fn bind_operations(mut self, binding: crate::operations::CatalogBinding) -> Self {
        #[cfg(feature = "simulation")]
        self.simulation.operations(binding.clone());
        self.operations.push(binding);
        self
    }

    pub fn service<R>(self, name: impl Into<String>, resident: R) -> Self
    where
        R: Resident<A>,
    {
        self.service_with(name, crate::services::POLICY, resident)
    }

    pub fn service_with<R>(mut self, name: impl Into<String>, policy: crate::ServicePolicy, resident: R) -> Self
    where
        R: Resident<A>,
    {
        #[cfg(feature = "simulation")]
        self.live_only.push("resident services");

        let definition = ServiceDefinition { id: name.into(), policy };
        self.services.push(DeclaredService::new(definition, resident));
        self
    }

    pub fn evidence(mut self, vault: impl Vault, kinds: impl IntoIterator<Item = (DefinitionId, u32)>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("an evidence vault");

        self.evidence = Some(Declaration::new(vault, kinds));
        self
    }

    pub fn attach(mut self, consumer: impl JournalConsumer<A>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("an attached fold");

        self.attachments.attach(consumer);
        self
    }

    pub fn attach_streams(mut self, consumer: impl StreamConsumer<A>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("an attached stream fold");

        self.attachments.attach_streams(consumer);
        self
    }

    pub fn series(mut self, series: impl Series<A>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("a series copy");

        self.attachments.series(series);
        self
    }

    pub fn recover(mut self, source: impl Source<A>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("a recovery source");

        self.attachments.recover(source);
        self
    }

    pub fn snapshot(mut self, snapshot: impl JournalConsumer<A>) -> Self {
        #[cfg(feature = "simulation")]
        self.live_only.push("a snapshot fold");

        self.attachments.snapshot(snapshot);
        self
    }

    pub fn require_capture(mut self, limits: CaptureLimits, consumer: impl JournalCapture<A>) -> Result<Self, crate::state::CaptureError> {
        #[cfg(feature = "simulation")]
        self.live_only.push("required capture");

        self.attachments.require_capture(limits, consumer)?;

        Ok(self)
    }

    pub fn retain(mut self, horizon: Horizon) -> Self {
        self.attachments.retain(horizon);
        self
    }

    pub fn tail(mut self, entries: usize) -> Self {
        self.attachments.tail(entries);
        self
    }

    #[cfg(feature = "simulation")]
    pub fn simulated_service<R>(mut self, name: impl Into<String>, resident: R) -> Self
    where
        R: Resident<A> + Clone,
    {
        self.simulation.service(name, resident);
        self
    }

    #[cfg(feature = "simulation")]
    pub fn simulated_service_with<R>(mut self, name: impl Into<String>, policy: crate::ServicePolicy, resident: R) -> Self
    where
        R: Resident<A> + Clone,
    {
        self.simulation.service_with(name, policy, resident);
        self
    }

    #[cfg(feature = "simulation")]
    pub fn simulated_evidence(mut self, vault: impl Vault + Clone, kinds: impl IntoIterator<Item = (DefinitionId, u32)>) -> Self {
        self.simulation.evidence(vault, kinds);
        self
    }

    #[cfg(feature = "simulation")]
    pub fn entropy_seed(mut self, seed: u64) -> Self {
        self.simulation.entropy_seed(seed);
        self
    }

    #[cfg(feature = "simulation")]
    pub fn simulate(self) -> Result<crate::simulation::SimulatedRuntime<A>, Incompatible>
    where
        A::State: Clone + fmt::Debug + PartialEq,
        A::Transaction: Clone + fmt::Debug,
        A::Record: Clone + fmt::Debug,
    {
        if !self.live_only.is_empty() {
            return Err(Incompatible::simulation(self.live_only.join(", ")));
        }

        self.simulation.simulate(self.origin, self.resources)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn runtime(self) -> Startup<A> {
        Startup::new(self, None)
    }

    pub fn managed_storage(mut self, settings: crate::config::ManagedStorageSettings) -> Self
    where
        A::State: serde::Serialize + serde::de::DeserializeOwned,
    {
        self.attachments.tail(settings.journal_tail);
        self.snapshots = settings.snapshots.clone().map(crate::state::snapshot::Snapshots::new);
        self.managed_storage = Some(settings);
        #[cfg(feature = "simulation")]
        self.live_only.push("managed storage recovery");
        self
    }

    pub fn store_records(mut self, projection: impl crate::storage::RecordProjection) -> Self {
        self.record_projection = Some(Box::new(projection));
        #[cfg(feature = "simulation")]
        self.live_only.push("managed primary record capture");
        self
    }

    pub fn reconcile<F, Fut>(mut self, reconcile: F) -> Self
    where
        F: FnOnce(StateManager<A>, crate::storage::StorageManager<crate::storage::ErasedCatalog>) -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        self.reconciliation = Some(Box::new(move |journal, storage| Box::pin(reconcile(journal, storage))));
        #[cfg(feature = "simulation")]
        self.live_only.push("saved application data reconciliation");
        self
    }

    pub fn initialize<F, Fut, E>(mut self, timeout: Duration, initialize: F) -> Self
    where
        F: Fn(Handle<A>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<(), E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        #[cfg(feature = "simulation")]
        self.simulation.initialize(timeout, initialize.clone());
        self.initializers.push(boot::Initializer::new(timeout, initialize));
        self
    }

    pub fn runtime_with_storage<C>(self, storage: &crate::storage::StorageManager<C>) -> Startup<A> {
        Startup::new(self, Some(storage.erased()))
    }

    async fn boot(
        self, storage: Option<&crate::storage::StorageManager<crate::storage::ErasedCatalog>>, cleanup: BootstrapCleanup,
    ) -> Result<Runtime<A>, Incompatible> {
        if self
            .initializers
            .iter()
            .any(|initializer| initializer.timeout.is_zero() || tokio::time::Instant::now().checked_add(initializer.timeout).is_none())
        {
            let source: crate::Error = Error::InvalidInput {
                field: "runtime initialization timeout",
                reason: "requires a positive representable finite duration",
            };
            return Err(Incompatible::initialization(Box::new(source)));
        }
        if self.reconciliation.is_some() && self.managed_storage.is_none() {
            return Err(Incompatible::reconciliation("reconciliation requires managed state"));
        }
        if self.record_projection.is_some() && self.managed_storage.is_none() {
            return Err(Incompatible::unreadable("primary record capture requires managed state"));
        }
        if self.managed_storage.is_some() && (storage.is_none() || self.attachments.has_recovery_owner()) {
            return Err(Incompatible::unreadable("managed state requires storage and exclusive recovery ownership"));
        }
        let mut boot = boot::Boot::new(storage, cleanup.clone()).map_err(Incompatible::work)?;
        if let Err(error) = self.boot_owned(&mut boot).await {
            return match boot.close().await {
                Ok(()) => Err(error),
                Err(failure) => {
                    let failure = Arc::new(failure);
                    let retained = boot.retain_cleanup();
                    cleanup.failed(retained.clone(), Arc::clone(&failure));
                    Err(Incompatible {
                        cause: Cause::Cleanup {
                            source: Box::new(error),
                            failure: Error::TaskFailed {
                                domain: "bootstrap cleanup",
                                source: failure,
                            },
                        },
                        cleanup: retained,
                    })
                }
            };
        }
        Ok(boot.finish())
    }

    async fn boot_owned(self, boot: &mut boot::Boot<A>) -> Result<(), Incompatible> {
        boot.acquire().await.map_err(Incompatible::work)?;
        let Self {
            origin,
            resources,
            inverses,
            catalog,
            definitions,
            mut attachments,
            managed_storage,
            record_projection,
            reconciliation,
            initializers,
            snapshots,
            lanes,
            operations: bindings,
            operation_settings,
            schema_settings,
            operation_middleware,
            services,
            evidence,
            ..
        } = self;
        let tail = attachments.tail_window()?;
        let mut consumers = attachments.supervised();
        let recovery = consumers.load().await?;
        let (resume, selections) = consumers.recordings().fold((Cursor::ORIGIN, Vec::new()), |(resume, mut selections), recording| {
            selections.push(recording.selections());

            (resume.max(recording.resume()), selections)
        });
        let anchor = origin.position;
        let mut state = At { value: origin.value, position: anchor };
        let mut undos = Undos::default();
        let entries = recovery.entries.into_iter().filter(|entry| entry.position > anchor).collect();
        let mut recovered = replay(Some(&catalog), entries, &mut state, &mut undos, inverses, tail).map_err(Incompatible::of)?;
        let stored = recovery.encoded.into_iter().filter(|(position, _)| *position > anchor).collect();
        state.position = state.position.max(recovery.frontier).max(anchor);
        let mut managed_durable = None;
        if let Some(settings) = managed_storage {
            let storage = boot.storage.as_ref().expect("managed state requires storage");
            let loaded = StateManager::recover(storage, state, &catalog, inverses, tail, settings.replay, snapshots.as_ref()).await?;
            state = loaded.state;
            undos = loaded.undos;
            recovered = loaded.tail;
            managed_durable = Some(loaded.durable);
            attachments.capture = Some(crate::state::capture::Declaration::Managed {
                storage: storage.clone(),
                receipts: loaded.receipts,
                projection: record_projection.map(|handler| crate::storage::capture::Projection { limits: settings.capture, handler }),
                snapshots,
            });
        }
        let windows = Table::resuming(definitions, state.position, resume).map_err(Incompatible::dishonored)?;
        let demand = windows.hold(selections).map_err(Incompatible::dishonored)?;
        let replayed = recovered.clone();
        consumers.redelivering(recovered.clone(), stored, state.position);

        let durability = managed_durable.or_else(|| consumers.durability(state.position));
        let tail = Tail::recovered(state.position, tail, recovered);
        let catalog = Arc::new(catalog);
        let (journal, writer, requests) = StateManager::start(
            state,
            inverses,
            undos,
            REQUESTS,
            tail,
            durability,
            Arc::clone(&catalog),
            attachments.capture.take(),
            boot.storage.clone(),
            schema_settings.clone(),
            crate::state::Timestamp::now,
        );
        let journal_handles = journal.handle_probe();
        let records = crate::records::RecordManager::new(journal.clone());
        boot.writer = Some(writer);
        boot.state = Some(journal.clone());
        // Evidence folds from the same replayed order the state above it
        // folded from, so a boot learns what was finalized from facts rather
        // than from whatever files happen to be lying in a vault.
        let evidence = Evidence::wired(journal.clone(), evidence, &replayed).map_err(Incompatible::kept)?;
        // The windows are wired before the consumers start, so a recorder
        // attached below never misses a row published in the gap between the
        // two.
        let (tier, served) = Tier::channel(REQUESTS);
        let streams = StreamManager::wired(windows, journal.progress(), tier, schema_settings.clone());
        let delivery = streams.delivery();
        operation_settings.validate().map_err(|source| Incompatible {
            cause: Cause::Operations(source),
            cleanup: None,
        })?;
        let lanes = crate::operations::lane::Lanes::freeze(lanes).map_err(Incompatible::work)?;
        let storage = Arc::new(boot.storage.clone().map(OnceLock::from).unwrap_or_default());
        let handles = crate::services::Handles {
            storage: storage.clone(),
            state: journal.clone(),
            records: records.clone(),
            streams: streams.clone(),
            evidence: evidence.clone(),
        };
        let instances = crate::operations::instances::Instances::start(lanes, Box::new(handles.clone()), resources, &operation_settings);
        let mut reserved = std::collections::BTreeSet::new();
        for service in &services {
            crate::services::validate(&service.definition()).map_err(Incompatible::work)?;
            if !reserved.insert(service.id().to_owned()) {
                let duplicate: crate::Error = crate::Error::AlreadyRegistered {
                    domain: "service",
                    name: service.id().to_owned(),
                };
                return Err(Incompatible::work(duplicate));
            }
        }
        let operations =
            crate::operations::Manager::new::<A>(instances, &operation_settings, reserved, boot.identity.clone(), schema_settings);
        for middleware in operation_middleware {
            operations.register_middleware(middleware).map_err(Incompatible::work)?;
        }
        for binding in bindings {
            binding.bind(&operations).map_err(|source| Incompatible {
                cause: Cause::Operations(source),
                cleanup: None,
            })?;
        }
        if let Some(storage) = &boot.storage {
            storage.start_on::<A>(&operations).await.map_err(Incompatible::work)?;
        }
        let (ready, booted) = oneshot::channel();
        let supervisor = tokio::spawn(supervise(Supervision {
            consumers,
            feed: journal.feed(),
            progress: journal.progress(),
            streams: streams.clone(),
            delivery,
            demand,
            writer: boot.writer.take().expect("boot owns writer"),
            catalog,
            inverses,
            requests,
            served,
            ready,
        }));
        #[cfg(feature = "grpc")]
        let grpc = crate::grpc::GrpcManager::new::<A>(&operations);
        #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
        let plugin = crate::plugin::PluginHost::new(
            operations.clone(),
            #[cfg(feature = "plugin-sidecar")]
            handles,
        )
        .map_err(Incompatible::work)?;
        boot.runtime = Some(Runtime {
            state: journal,
            records,
            streams,
            operations: crate::operations::OperationManager::new(operations),
            #[cfg(all(feature = "plugin", not(target_arch = "wasm32")))]
            plugin,
            #[cfg(feature = "grpc")]
            grpc,
            evidence,
            stopping: Mutex::new(Stopping::Running(Some(supervisor))),
            closing: AtomicBool::new(false),
            storage,
            journal_handles,
            participants: Participants::new(),
            notices: watch::channel(None).0,
        });
        boot.state.take();
        booted.await.map_err(Incompatible::work)?;
        if let Some(reconcile) = reconciliation {
            let runtime = boot.runtime.as_ref().expect("boot owns runtime");
            let storage = runtime.storage().expect("reconciliation requires storage");
            reconcile(runtime.state.clone(), storage.clone()).await.map_err(Incompatible::reconciliation)?;
            storage.flush().await.map_err(Incompatible::reconciliation)?;
            storage.flush_recovery().await.map_err(Incompatible::reconciliation)?;
        }
        let operations = &boot.runtime.as_ref().expect("runtime is owned during boot").operations;
        for service in services {
            let lease = operations.start_service(service, false, true).map_err(Incompatible::work)?;
            operations.hold_resident(lease);
        }
        for initializer in initializers {
            let handle = boot.runtime.as_ref().expect("runtime is owned during initialization").handle();
            let initialize = (initializer.initialize)(handle);
            match tokio::time::timeout(initializer.timeout, initialize).await {
                Ok(result) => result.map_err(Incompatible::initialization)?,
                Err(source) => return Err(Incompatible::initialization(Box::new(source))),
            }
        }
        Ok(())
    }
}

impl Incompatible {
    fn initialization(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            cause: Cause::Initialization(source),
            cleanup: None,
        }
    }

    pub fn cleanup_error(&self) -> Option<&crate::Error> {
        match (&self.cleanup, &self.cause) {
            (Some(_), Cause::Cleanup { failure, .. }) => Some(failure),
            _ => None,
        }
    }

    pub async fn retry_cleanup(&mut self) -> crate::Result<()> {
        if let Some(cleanup) = &self.cleanup {
            cleanup.close().await?;
            self.cleanup.take();
        }
        Ok(())
    }

    fn reconciliation(cause: impl fmt::Display) -> Self {
        Self {
            cause: Cause::Reconciliation(cause.to_string()),
            cleanup: None,
        }
    }

    pub fn unreadable(cause: impl fmt::Display) -> Self {
        Self {
            cause: Cause::Unreadable(cause.to_string()),
            cleanup: None,
        }
    }

    fn of(cause: DecodeError) -> Self {
        Self {
            cause: Cause::Undeclared(cause),
            cleanup: None,
        }
    }

    pub(crate) fn dishonored(cause: crate::streams::Refused) -> Self {
        Self {
            cause: Cause::Dishonorable(cause),
            cleanup: None,
        }
    }

    pub(crate) fn work(cause: impl fmt::Display) -> Self {
        Self {
            cause: Cause::Work(cause.to_string()),
            cleanup: None,
        }
    }

    #[cfg(feature = "simulation")]
    fn simulation(live_only: String) -> Self {
        Self {
            cause: Cause::Simulation(live_only),
            cleanup: None,
        }
    }

    fn kept(cause: String) -> Self {
        Self { cause: Cause::Kept(cause), cleanup: None }
    }
}

#[cfg(feature = "plugin-sidecar")]
impl From<harmos_sidecar::ExecutableError> for Incompatible {
    fn from(error: harmos_sidecar::ExecutableError) -> Self {
        Self::work(error)
    }
}

impl Participants {
    fn new() -> Self {
        Self {
            stopping: watch::channel(false).0,
            active: watch::channel(0).0,
        }
    }

    fn register(&self) -> StopParticipant {
        self.active.send_modify(|active| *active += 1);

        StopParticipant {
            stopping: self.stopping.subscribe(),
            active: self.active.clone(),
            released: false,
        }
    }

    async fn stop(&self) {
        self.stopping.send_replace(true);
        let mut active = self.active.subscribe();
        while *active.borrow_and_update() != 0 {
            if active.changed().await.is_err() {
                break;
            }
        }
    }
}

impl StopParticipant {
    pub fn is_stopping(&self) -> bool {
        *self.stopping.borrow()
    }

    pub async fn stopping(&mut self) {
        while !*self.stopping.borrow_and_update() {
            if self.stopping.changed().await.is_err() {
                break;
            }
        }
    }

    pub fn finish(mut self) {
        self.release();
    }

    fn release(&mut self) {
        if !self.released {
            self.active.send_modify(|active| *active = active.saturating_sub(1));
            self.released = true;
        }
    }
}

impl Drop for StopParticipant {
    fn drop(&mut self) {
        self.release();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn none<A: Application>(_change: &A::Transaction, _before: &A::State) -> Option<A::Transaction> {
    None
}

async fn supervise<A: Application>(supervision: Supervision<A>) -> Position {
    let Supervision {
        mut consumers,
        feed,
        mut progress,
        streams,
        mut delivery,
        demand: _demand,
        mut writer,
        catalog,
        inverses,
        mut requests,
        mut served,
        ready,
    } = supervision;
    // Starting the consumers is a step of boot rather than a consequence of the
    // next commit: a fold whose checkpoint predates the crash is owed entries
    // the moment the runtime is up, however quiet the writer then stays.
    let booted = *progress.borrow_and_update();
    consumers.drain(&feed, booted).await;
    deliver(&mut consumers, &streams).await;
    stand_windows(&streams, &mut consumers, booted).await;
    let _ = ready.send(());

    loop {
        match turn(&mut progress, &mut delivery, &mut requests, &mut served, &mut writer).await {
            Turn::Advanced { journal, streams: rows } => {
                if journal {
                    let applied = *progress.borrow_and_update();
                    consumers.drain(&feed, applied).await;
                }
                if rows {
                    delivery.borrow_and_update();
                    deliver(&mut consumers, &streams).await;
                }
            }
            Turn::Read(request) => answer(request, &mut consumers, Some(&catalog), inverses).await,
            Turn::Serve(request) => serve(*request, &mut consumers).await,
            Turn::Stopped => break,
        }
    }

    let applied = *progress.borrow();
    consumers.drain(&feed, applied).await;
    deliver(&mut consumers, &streams).await;
    consumers.flush().await;

    // What the flush made durable is read back through the contract's own
    // question, so the reported frontier includes the batches it just wrote.
    consumers.settle(applied).await
}

async fn deliver<A: Application>(consumers: &mut Consumers<A>, streams: &StreamManager<A>) {
    for recording in consumers.recordings() {
        let resume = recording.resume();
        let batch = streams.batch(resume);

        if batch.reaches() > resume {
            recording.drain(batch).await;
        }
    }
}

async fn stand_windows<A: Application>(streams: &StreamManager<A>, consumers: &mut Consumers<A>, booted: Position) {
    for wired in streams.declared() {
        let durable = consumers.sealed(wired.definition.clone(), &[]).await;

        if durable > Position::ORIGIN && durable <= booted {
            wired.window.stands_on(durable);
        }
    }
}

async fn turn<A: Application>(
    progress: &mut watch::Receiver<Position>, delivery: &mut watch::Receiver<()>, requests: &mut mpsc::Receiver<ReadRequest<A>>,
    served: &mut mpsc::Receiver<Request>, writer: &mut JoinHandle<()>,
) -> Turn<A> {
    let mut advanced = Box::pin(progress.changed());
    let mut published = Box::pin(delivery.changed());

    std::future::poll_fn(|context| {
        if Pin::new(&mut *writer).poll(context).is_ready() {
            return Poll::Ready(Turn::Stopped);
        }
        let journal = match advanced.as_mut().poll(context) {
            Poll::Ready(Ok(())) => true,
            Poll::Ready(Err(_)) => return Poll::Ready(Turn::Stopped),
            Poll::Pending => false,
        };
        let streams = match published.as_mut().poll(context) {
            Poll::Ready(Ok(())) => true,
            Poll::Ready(Err(_)) => return Poll::Ready(Turn::Stopped),
            Poll::Pending => false,
        };

        if journal || streams {
            Poll::Ready(Turn::Advanced { journal, streams })
        } else {
            match requests.poll_recv(context) {
                Poll::Ready(Some(request)) => Poll::Ready(Turn::Read(request)),
                Poll::Ready(None) | Poll::Pending => match served.poll_recv(context) {
                    Poll::Ready(Some(request)) => Poll::Ready(Turn::Serve(Box::new(request))),
                    Poll::Ready(None) | Poll::Pending => Poll::Pending,
                },
            }
        }
    })
    .await
}

async fn answer<A: Application>(
    request: ReadRequest<A>, consumers: &mut Consumers<A>, catalog: Option<&Dispatch<A, Payload<A>>>, inverses: Inverting<A>,
) {
    match request {
        ReadRequest::Entries { after, through, reply, trace } => {
            let answer =
                consumers.read(after, through).instrument(trace.span.clone()).in_context(trace.dispatcher.clone()).await.map(|recovery| {
                    recovery
                        .entries
                        .into_iter()
                        .map(|envelope| Entry {
                            prepared: envelope.payload.prepared.clone(),
                            position: envelope.position,
                            metadata: envelope.metadata,
                            value: envelope.payload.value,
                        })
                        .collect()
                });
            let _ = reply.send(answer);
        }
        ReadRequest::State { origin, at, reply, trace } => {
            let answer = async {
                let recovery = consumers.read(origin.position, at).await?;
                if recovery
                    .commits
                    .iter()
                    .copied()
                    .chain(recovery.entries.iter().filter_map(|entry| entry.metadata.commit))
                    .any(|(first, end)| (first <= at && at < end) || (first <= origin.position && origin.position < end))
                {
                    return Err(crate::Error::IncompleteCommit);
                }
                if recovery.frontier < at {
                    return Err(crate::Error::BeyondTail);
                }
                let mut state = At {
                    value: origin.value,
                    position: origin.position,
                };
                replay(catalog, recovery.entries, &mut state, &mut Undos::default(), inverses, 0).map_err(|_| crate::Error::Unavailable)?;
                state.position = at;

                Ok(state)
            }
            .instrument(trace.span.clone())
            .in_context(trace.dispatcher.clone())
            .await;
            let _ = reply.send(answer);
        }
    }
}

async fn serve<A: Application>(request: Request, consumers: &mut Consumers<A>) {
    match request {
        Request::Reader {
            stream,
            track,
            start,
            end,
            limits,
            permit,
            reply,
        } => {
            if !reply.is_closed() {
                let answer = consumers.row_reader(stream, &track, start, end, limits).await;
                let _ = reply.send(crate::streams::Opened { source: answer, permit });
            }
        }
        Request::Rows { stream, track, start, end, reply } => {
            let answer = consumers.served(stream, &track, start, end).await;
            let _ = reply.send(answer);
        }
        Request::Buckets { stream, track, start, end, budget, reply } => {
            let answer = consumers.surveyed(stream, &track, start, end, budget).await;
            let _ = reply.send(answer);
        }
        Request::Aggregate { stream, track, start, end, kind, reply } => {
            let answer = consumers.aggregated(stream, &track, start, end, kind).await;
            let _ = reply.send(answer);
        }
        Request::Seal { stream, track, reply } => {
            let _ = reply.send(consumers.sealed(stream, &track).await);
        }
    }
}

async fn inspect_reads<A: Application>(mut consumers: Consumers<A>, mut requests: mpsc::Receiver<ReadRequest<A>>) {
    while let Some(request) = requests.recv().await {
        answer(request, &mut consumers, None, none::<A>).await;
    }
}

/* === Tests === */
