/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

#[path = "sidecar_streams.rs"]
mod streams;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{assembly, service_status};
use crate::support::Bank;
use harmos::operations::Host as _;
use harmos::operations::{
    Declarations, Definition, Operation, OperationDeclaration, OperationPolicy, RestartPolicy,
    RetryPolicy, Visibility,
};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions,
    PluginLifecycle, PluginLifecycleAuthority, PluginManager, PluginTransition, Router,
};
use harmos::sidecar::{Attach, Attached, Notice, Supervised};
use harmos::{services::Service, services::ServiceExit, services::ServiceStatus};

/* === Constants === */
const CAPACITY: usize = 2;
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<Counts>>> =
    Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */
type RetirementGate = Arc<Mutex<Option<(Arc<Notify>, Arc<Notify>)>>>;
type Source = Advertised<harmos_sidecar::PluginNative<Endpoint>>;
pub(crate) type Resident<A> = Supervised<Bank, A, fn(&Service<Bank>, Notice)>;
type Backends<A> = PluginBackend<(), Backend<A>, ()>;

/* === Structs === */
#[derive(Default)]
struct Counts {
    attached: AtomicUsize,
    initialized: AtomicUsize,
    cleaned: AtomicUsize,
    prepared: AtomicUsize,
    operation_initialized: AtomicUsize,
    stopped: AtomicUsize,
    fail_initialize: AtomicBool,
    block_initialize: AtomicBool,
    initialization_child: AtomicBool,
    initialization_entered: Notify,
    initialization_saved: Mutex<Option<harmos::sidecar::Supervisor>>,
    runs: AtomicUsize,
    finalized: AtomicUsize,
    fail_cleanup: AtomicBool,
    fail_prepare: AtomicBool,
    call_child: AtomicBool,
    children: AtomicUsize,
    stale_refused: AtomicUsize,
    saved: Mutex<Option<harmos::sidecar::Supervisor>>,
    tenants: Mutex<Vec<Option<String>>>,
    submit_child: AtomicBool,
    refresh_hold: AtomicBool,
    refresh_started: Notify,
    refresh_release: Notify,
    refresh_completed: AtomicUsize,
    refresh_cleaned: AtomicUsize,
    detached_status: Mutex<Option<String>>,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Tenant {
    name: String,
}

struct Principal;

struct Seed {
    id: usize,
}

struct Endpoint {
    counts: Arc<Counts>,
}

struct Read;

struct ReadOperation {
    counts: Arc<Counts>,
    attempts: usize,
}

struct RefreshOperation(Arc<Counts>);

struct Refresh;

#[derive(Clone)]
struct Advertised<S> {
    source: S,
    operations: Vec<Definition>,
    counts: Arc<Counts>,
    seed: Arc<Seed>,
}

struct Backend<A: Attach> {
    services: Option<harmos::operations::OperationManager>,
    retirement_gate: RetirementGate,
    resident: Option<Resident<A>>,
    lease: Option<harmos::operations::OperationInstance<harmos::operations::resident::Activation>>,
    registration: Option<harmos::operations::PluginRegistration>,
    sdk: Option<harmos::operations::PluginSdkRegistration>,
    initializing: Option<harmos::operations::EncodedInvocation>,
    shutting_down: Option<harmos::operations::EncodedInvocation>,
    registered: bool,
    initialized: bool,
    shutdown_attempted: bool,
}

struct Host<A: Attach> {
    ready: Option<tokio::sync::watch::Receiver<bool>>,
    resident: Resident<A>,
    lane: Lanes,
    capacity: std::num::NonZeroUsize,
    supervision: harmos::operations::OperationPolicy,
    operations: harmos::operations::OperationManager,
    retirement_gate: RetirementGate,
}

pub(crate) struct Managed<A: Attach> {
    pub(crate) runtime: harmos::Runtime<Bank>,
    manager: PluginManager<(), Backend<A>, ()>,
    host: Host<A>,
}

pub(crate) struct Calling(pub(crate) harmos::operations::EncodedInvocation);

/* === Enums === */
#[harmos::lanes]
pub(crate) enum Lanes {
    #[default]
    NativeOpen,
    #[lane(serial)]
    NativeSerial,
}

#[harmos::catalog]
enum Operations {
    Refresh(Refresh),
    Read(Read),
    Upload(streams::Upload),
    Download(streams::Download),
    Exchange(streams::Exchange),
}

/* === Traits === */

/* === Implementations === */
impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl harmos::operations::WireOperation for Read {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<harmos::operations::Grpc> = Some(harmos::operations::Grpc::new(
        "fixture.v1",
        "Native",
        "Read",
    ));
    #[cfg(feature = "grpc")]
    fn visit_grpc<V: harmos::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl harmos::operations::WireOperation for Refresh {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Read {
    type Request = ();
    type Response = ();
    const ID: &'static str = "fixture.read";
    const DESCRIPTION: &'static str = "Reads a device";
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: harmos::operations::Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: harmos::operations::Jitter::None,
            },
        },
        restart: RestartPolicy::Never,
    };
}

impl harmos::operations::ContextValue for Tenant {
    const KEY: &'static str = "fixture.tenant";
}

impl Operation for ReadOperation {
    type Declaration = Read;
    type Resources = Counts;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a harmos::sidecar::Supervisor;
    async fn initialize<'ctx>(
        counts: Arc<Counts>,
        context: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        let constructed: Result<Self, Self::Error> = async move {
            counts.prepared.fetch_add(1, Ordering::SeqCst);
            if counts.fail_prepare.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("preparation refused"));
            }
            Ok(Self {
                counts,
                attempts: 0,
            })
        }
        .await;
        let mut operation = constructed?;
        let started: Result<(), Self::Error> = async {
            let operation = &mut operation;
            operation
                .counts
                .operation_initialized
                .fetch_add(1, Ordering::SeqCst);
            operation.counts.initialization_entered.notify_one();
            if operation.counts.block_initialize.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            if operation.counts.fail_initialize.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("partial initialization failed"));
            }
            if operation.counts.initialization_child.load(Ordering::SeqCst) {
                context
                    .call_as::<Refresh>(())
                    .await
                    .map_err(std::io::Error::other)?;
                *operation.counts.initialization_saved.lock().unwrap() = Some(context.clone());
            }
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            if self.counts.submit_child.load(Ordering::SeqCst) {
                return submitted(context, &self.counts)
                    .await
                    .map_err(std::io::Error::other);
            }
            let initialized = self.counts.initialization_saved.lock().unwrap().take();
            if let Some(initialized) = initialized {
                if initialized.call_as::<Refresh>(()).await.is_ok() {
                    return Err(std::io::Error::other(
                        "initialization context remained active",
                    ));
                }
                self.counts.stale_refused.fetch_add(1, Ordering::SeqCst);
            }
            self.attempts += 1;
            self.counts.runs.fetch_add(1, Ordering::SeqCst);
            let tenant = context
                .context()
                .value::<Tenant>()
                .map_err(std::io::Error::other)?;
            assert!(context.context().get::<Principal>().is_none());
            self.counts
                .tenants
                .lock()
                .unwrap()
                .push(tenant.map(|tenant| tenant.name));
            if self.counts.call_child.load(Ordering::SeqCst) {
                let previous = self.counts.saved.lock().unwrap().take();
                if let Some(previous) = previous {
                    if previous.call_as::<Refresh>(()).await.is_ok() {
                        return Err(std::io::Error::other("expired attempt admitted a child"));
                    }
                    self.counts.stale_refused.fetch_add(1, Ordering::SeqCst);
                }
                *self.counts.saved.lock().unwrap() = Some(context.clone());
                context
                    .call_as::<Refresh>(())
                    .await
                    .map_err(std::io::Error::other)?;
            }
            if self.attempts == 1 {
                return Err(std::io::Error::other("one transient failure"));
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.counts.stopped.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.counts.fail_cleanup.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("retained cleanup"));
        }
        self.counts.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn classify(_: &Self::Error) -> harmos::operations::Failure {
        harmos::operations::Failure::Transient
    }
}

impl Operation for RefreshOperation {
    type Declaration = Refresh;
    type Resources = Counts;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a harmos::sidecar::Supervisor;
    async fn initialize<'ctx>(
        counts: Arc<Counts>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self(counts))
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.0.children.fetch_add(1, Ordering::SeqCst);
            if self.0.refresh_hold.load(Ordering::SeqCst) {
                self.0.refresh_started.notify_one();
                self.0.refresh_release.notified().await;
                self.0.refresh_completed.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.refresh_cleaned.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl OperationDeclaration for Refresh {
    type Request = ();
    type Response = ();
    const ID: &'static str = "fixture.refresh";
    const POLICY: OperationPolicy = Read::POLICY;
}

impl harmos::operations::lifecycle::Policy for Endpoint {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(id = "operation-handshake", provides = Operations, requires = ())]
impl harmos::Plugin for Endpoint {
    type Config = usize;
    type Context = harmos::sidecar::Supervisor;
    type Error = std::io::Error;
    type Resources = Counts;
    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let counts = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|counts| counts.upgrade())
            .ok_or_else(|| std::io::Error::other("original fixture resources unavailable"))?;
        Ok(Self { counts })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.counts.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn resources(&self) -> Arc<Counts> {
        self.counts.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<ReadOperation>(self.resources())?;
        table.bind_operation::<RefreshOperation>(self.resources())?;
        streams::bind(table, self.resources())?;
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.counts.cleaned.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for Calling {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl std::future::Future for Calling {
    type Output = harmos::Result<Vec<u8>>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::future::Future::poll(std::pin::Pin::new(&mut self.get_mut().0), context)
    }
}

impl Managed<Source> {
    async fn new(source: Source, lane: Lanes) -> Self {
        let carrier = &source.source;
        let definitions = PluginDefinitions::new(
            [PluginDefinition {
                id: carrier.owner().into(),
                version: "1.0.0".parse().unwrap(),
                artifact: PluginArtifact::LocalSidecar {
                    revision: "fixture".into(),
                },
                configuration: serde_json::to_vec(&source.seed.id).unwrap(),
                requires: vec![],
                contributions: PluginContributions {
                    operations: carrier.operations().unwrap(),
                    wire_contracts: carrier.operation_contracts().unwrap().unwrap(),
                    required_contracts: carrier.required_contracts().unwrap(),
                    lifecycle: carrier.sdk_lifecycle().unwrap(),
                    ..PluginContributions::default()
                },
            }],
            [],
        )
        .unwrap();
        Self::from_definition(
            source,
            definitions,
            lane,
            std::num::NonZeroUsize::new(CAPACITY).unwrap(),
            vec![],
            std::convert::identity,
        )
        .await
    }
}

impl<A: Attach> Managed<A> {
    pub(crate) fn drop_manager(self) -> (harmos::Runtime<Bank>, harmos::plugin::PluginCleanup) {
        let Self {
            runtime,
            manager,
            host,
        } = self;
        let cleanup = manager.cleanup();
        drop(manager);
        drop(host);
        (runtime, cleanup)
    }

    pub(crate) async fn attach(
        source: A,
        configuration: Vec<u8>,
        lane: Lanes,
        capacity: std::num::NonZeroUsize,
        demand_keys: Vec<String>,
        configure: impl FnOnce(Resident<A>) -> Resident<A>,
    ) -> Self {
        let definitions = inspected(&source, configuration);
        Self::from_definition(source, definitions, lane, capacity, demand_keys, configure).await
    }
    pub(crate) async fn from_definition(
        source: A,
        definitions: PluginDefinitions,
        lane: Lanes,
        capacity: std::num::NonZeroUsize,
        demand_keys: Vec<String>,
        configure: impl FnOnce(Resident<A>) -> Resident<A>,
    ) -> Self {
        let runtime = assembly()
            .register(Lanes::catalog())
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        let resident = configure(
            Supervised::new(source, notices as fn(&Service<Bank>, Notice)).demand_keys(demand_keys),
        );
        Self::from_runtime(
            runtime,
            definitions,
            resident,
            lane,
            capacity,
            harmos::services::POLICY,
        )
    }
    pub(crate) fn from_runtime(
        runtime: harmos::Runtime<Bank>,
        definitions: PluginDefinitions,
        resident: Resident<A>,
        lane: Lanes,
        capacity: std::num::NonZeroUsize,
        supervision: harmos::operations::OperationPolicy,
    ) -> Self {
        let host = Host {
            ready: None,
            resident,
            lane,
            capacity,
            supervision,
            operations: runtime.operations.clone(),
            retirement_gate: Arc::default(),
        };
        Self {
            runtime,
            manager: PluginManager::new(definitions).unwrap(),
            host,
        }
    }
    pub(crate) fn resident(&self) -> &Resident<A> {
        &self.host.resident
    }
    pub(crate) fn status(&self, id: &str) -> Option<harmos::plugin::PluginStatus> {
        self.manager.status(id)
    }
    pub(crate) fn publication(&self) -> harmos::Result<()> {
        let selected =
            harmos::operations::WireDeclaration::of::<harmos::operations::host::Publish>()
                .contract()
                .map_err(|source| harmos::Error::OperationFailed {
                    operation: "host.publish".into(),
                    source: Arc::new(source),
                })?;
        let required = self.manager.definitions().any(|definition| {
            definition
                .contributions
                .required_contracts
                .iter()
                .flatten()
                .any(|required| required.contract == selected)
        });
        if !required {
            return Err(harmos::Error::InvalidInput {
                field: "fixture publication provider",
                reason: "provider differs from the original selected native contract",
            });
        }
        self.runtime
            .operations
            .register_wire_operation::<harmos::sidecar::Publish<_, _, _>>(
                "native-publication-core",
                self.host.lane,
                Arc::new(self.host.resident.clone()),
            )
    }

    pub(crate) async fn call_as<O>(&self, request: O::Request) -> harmos::Result<O::Response>
    where
        O: harmos::operations::WireOperation,
        O::Request: harmos::Message + harmos::schema::Described,
        O::Response: harmos::Message + harmos::schema::Described,
    {
        self.call_in::<O>(harmos::operations::CallContext::default(), request)
            .await
    }

    pub(crate) async fn call_in<O>(
        &self,
        context: harmos::operations::CallContext,
        request: O::Request,
    ) -> harmos::Result<O::Response>
    where
        O: harmos::operations::WireOperation,
        O::Request: harmos::Message + harmos::schema::Described,
        O::Response: harmos::Message + harmos::schema::Described,
    {
        let contract = harmos::operations::WireDeclaration::of::<O>()
            .contract()
            .map_err(|source| harmos::Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            })?;
        let selected = self
            .manager
            .definitions()
            .flat_map(|definition| &definition.contributions.wire_contracts)
            .find(|selected| selected.operation().id.as_str() == O::ID);
        if selected != Some(&contract) {
            return Err(harmos::Error::InvalidInput {
                field: "fixture wire operation",
                reason: "caller differs from the original frozen contract",
            });
        }
        let payload = harmos::Message::encode_to_vec(&request);
        let invocation = self
            .runtime
            .operations
            .with_context(context)
            .submit_encoded(O::ID, payload)?;
        let reply = Calling(invocation).await?;
        harmos::Message::decode(&reply).map_err(|source| harmos::Error::OperationPayload {
            operation: O::ID.into(),
            source,
        })
    }
    pub(crate) async fn initialize(&mut self) -> harmos::Result<()> {
        self.manager.initialize(&mut self.host).await
    }
    pub(crate) fn pause_retirement(&mut self, entered: Arc<Notify>, release: Arc<Notify>) {
        *self.host.retirement_gate.lock().unwrap() = Some((entered, release));
    }
    pub(crate) async fn shutdown(&mut self) -> harmos::Result<()> {
        self.manager.shutdown(&mut self.host).await
    }
    pub(crate) async fn reload(&mut self, configuration: Vec<u8>) -> harmos::Result<()> {
        let mut definition = self
            .manager
            .definitions()
            .next()
            .expect("one fixture definition")
            .clone();
        definition.configuration = configuration;
        let candidate = PluginDefinitions::new([definition], [])?;
        self.manager.reload(candidate, &mut self.host).await
    }
    pub(crate) fn ready(&mut self) -> &mut tokio::sync::watch::Receiver<bool> {
        self.host
            .ready
            .as_mut()
            .expect("the original resident was staged")
    }
    pub(crate) fn demands(&self) -> harmos::sidecar::DemandSource {
        self.host.resident.demands()
    }
}

impl<A: Attach> Default for Backend<A> {
    fn default() -> Self {
        Self {
            services: None,
            retirement_gate: Arc::default(),
            resident: None,
            lease: None,
            registration: None,
            sdk: None,
            initializing: None,
            shutting_down: None,
            registered: false,
            initialized: false,
            shutdown_attempted: false,
        }
    }
}

impl<A: Attach> PluginLifecycle for Host<A> {
    type Guest = ();
    type Sidecar = Backend<A>;
    type Embedded = ();
    fn preflight(&mut self, definitions: &PluginDefinitions) -> harmos::Result<()> {
        self.operations.validate_requirements(definitions)
    }
    fn prepare(&mut self, _: &PluginDefinition) -> harmos::Result<Backends<A>> {
        Ok(PluginBackend::Sidecar(Backend {
            services: Some(self.operations.clone()),
            retirement_gate: self.retirement_gate.clone(),
            ..Backend::default()
        }))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> harmos::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends<A>,
    ) -> harmos::Result<()> {
        let PluginBackend::Sidecar(backend) = backend else {
            unreachable!()
        };
        let resident = self.resident.clone().routed(router);
        self.ready = Some(resident.ready());
        backend.resident = Some(resident.clone());
        self.operations.register_service(
            harmos::services::ServiceDefinition {
                id: definition.id.clone(),
                policy: self.supervision,
            },
            move |_: Arc<()>| resident.clone(),
        )?;
        backend.registered = true;
        backend.lease = Some(self.operations.submit_resident(&definition.id).await?);
        let native = tokio::time::timeout(
            Duration::from_secs(5),
            backend.resident.as_ref().unwrap().staged_operations(),
        )
        .await
        .map_err(|source| harmos::Error::TaskFailed {
            domain: "native fixture staging",
            source: Arc::new(source),
        })?
        .map_err(|source| harmos::Error::OperationFailed {
            operation: definition.id.clone(),
            source: Arc::new(source),
        })?;
        let native = Arc::new(native);
        backend.registration = Some(
            self.operations
                .stage_plugin(native.scope().unwrap(), |bindings| {
                    bindings.register_native(self.lane, native.clone(), self.capacity)
                })?,
        );
        backend.sdk = Some(native.stage_sdk(&self.operations, self.lane)?);
        backend.initializing = Some(
            backend.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                std::num::NonZeroUsize::new(
                    2 * definition.configuration.len()
                        + std::mem::size_of::<harmos::operations::lifecycle::InitializeRequest>()
                        + 64
                        + 20,
                )
                .unwrap(),
            )?,
        );
        let result = backend.initializing.as_mut().unwrap().await;
        backend.initializing.take();
        result?;
        backend.initialized = true;
        Ok(())
    }
    async fn stop(
        definition: &PluginDefinition,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends<A>,
    ) -> harmos::Result<()> {
        let PluginBackend::Sidecar(backend) = backend else {
            unreachable!()
        };
        let gate = backend.retirement_gate.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.notify_one();
            release.notified().await;
        }
        if let Some(sdk) = &backend.sdk {
            sdk.settle().await?;
            if let Some(initializing) = &mut backend.initializing {
                let result = initializing.await;
                backend.initializing.take();
                result?;
                backend.initialized = true;
            }
            if backend.initialized && !backend.shutdown_attempted {
                backend.shutting_down = Some(sdk.submit_shutdown(authority)?);
                backend.shutdown_attempted = true;
            }
            if let Some(shutdown) = &mut backend.shutting_down {
                let result = shutdown.await;
                backend.shutting_down.take();
                result?;
                backend.initialized = false;
            }
        }
        if let Some(registration) = &mut backend.registration {
            registration.shutdown().await?;
        }
        if let Some(lease) = backend.lease.take() {
            lease.release().await?;
        }
        if backend.registered {
            backend
                .services
                .as_ref()
                .unwrap()
                .unregister_resident(&definition.id)
                .await?;
        }
        backend.registered = false;
        if let Some(sdk) = &mut backend.sdk {
            sdk.close().await?;
        }
        backend.sdk.take();
        backend.registration.take();
        backend.resident.take();
        Ok(())
    }
}

impl<S: Attach> Attach for Advertised<S> {
    type Error = S::Error;

    fn owner(&self) -> &str {
        self.source.owner()
    }

    fn operations(&self) -> Result<Vec<Definition>, harmos_contract::Error> {
        Ok(self.operations.clone())
    }

    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        self.source.publications()
    }
    fn operation_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_contract::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
        self.source.sdk_lifecycle()
    }
    fn observers(&self) -> Vec<harmos_contract::inventory::observers::Declaration> {
        self.source.observers()
    }
    fn observer_control(&self) -> harmos_contract::Result<Option<harmos::operations::Contract>> {
        self.source.observer_control()
    }
    fn listener_declarations(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::listeners::Definition>>> {
        self.source.listener_declarations()
    }

    async fn attach(&self) -> Result<Attached, Self::Error> {
        self.counts.attached.fetch_add(1, Ordering::SeqCst);
        self.source.attach().await
    }
}

/* === Public Functions === */
pub(crate) fn inspected<A: Attach>(source: &A, configuration: Vec<u8>) -> PluginDefinitions {
    PluginDefinitions::new([definition(source, configuration)], []).unwrap()
}

pub(crate) fn definition<A: Attach>(source: &A, configuration: Vec<u8>) -> PluginDefinition {
    PluginDefinition {
        id: source.owner().into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::LocalSidecar {
            revision: "fixture".into(),
        },
        configuration,
        requires: vec![],
        contributions: PluginContributions {
            operations: source.operations().unwrap(),
            wire_contracts: source.operation_contracts().unwrap().unwrap(),
            required_contracts: source.required_contracts().unwrap(),
            lifecycle: source.sdk_lifecycle().unwrap(),
            streams: source.publications(),
            observers: source.observers(),
            observer_control: source.observer_control().unwrap(),
            listeners: source.listener_declarations().unwrap().unwrap_or_default(),
            ..PluginContributions::default()
        },
    }
}

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

// Drives submitted children through the sidecar binding of the host.
async fn submitted(
    host: &harmos::sidecar::Supervisor,
    counts: &Counts,
) -> Result<(), harmos_sidecar::CallError> {
    use harmos::operations::instance::Phase;
    use harmos::operations::{Host, Instance};

    // Status, Then Join
    let joined = host.submit_as::<Refresh>(()).await?;
    counts.refresh_started.notified().await;
    let status = joined.status().await?;
    assert_eq!(status.phase, Phase::Running);
    counts.refresh_release.notify_one();
    joined.join().await?;

    // Cancel, Then Join
    let cancelled = host.submit_as::<Refresh>(()).await?;
    counts.refresh_started.notified().await;
    assert!(Instance::cancel_and_join(cancelled).await.is_err());

    // Dropping The Handle Releases The Child
    let dropped = host.submit_as::<Refresh>(()).await?;
    counts.refresh_started.notified().await;
    let cleaned = counts.refresh_cleaned.load(Ordering::SeqCst);
    drop(dropped);
    tokio::time::timeout(Duration::from_secs(2), async {
        while counts.refresh_cleaned.load(Ordering::SeqCst) == cleaned {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("a dropped instance is cancelled");

    // Detaching Keeps The Child Running Within This Attempt
    let detached = host.submit_as::<Refresh>(()).await?;
    counts.refresh_started.notified().await;
    let id = Instance::detach(detached);
    // Built-ins are ordinary requirements: an undeclared caller is refused.
    let refused = host.call(harmos::builtin::Status::of(id)).await;
    *counts.detached_status.lock().unwrap() = refused.err().map(|error| {
        std::iter::successors(
            Some(&error as &(dyn std::error::Error + 'static)),
            |error| error.source(),
        )
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
    });
    counts.refresh_release.notify_one();
    // Without a handle the child still runs while this attempt lives.
    tokio::time::timeout(Duration::from_secs(2), async {
        while counts.refresh_completed.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("a detached child runs on");
    Ok(())
}

fn source(counts: Arc<Counts>) -> Source {
    let id = NEXT_SEED
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
        .unwrap();
    {
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "bounded fixture resource handoff is full");
        seeds.insert(id, Arc::downgrade(&counts));
    }
    let source = harmos_sidecar::PluginNative::<Endpoint>::new(
        std::num::NonZeroUsize::new(CAPACITY).unwrap(),
    )
    .unwrap();
    let operations = source.operations().unwrap();
    Advertised {
        source,
        operations,
        counts,
        seed: Arc::new(Seed { id }),
    }
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("catalog_mismatch_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for change in 0..8 {
                        let counts = Arc::new(Counts::default());
                        let mut operations = Declarations::of::<Operations>().unwrap();
                        match change {
                            0 => {
                                operations.pop();
                            }
                            1 => operations[0].policy.timeout += Duration::from_secs(1),
                            2 => operations[0].visibility = Visibility::Shared,
                            3 => {
                                operations[0].replay = harmos::operations::ReplaySafety::Idempotent
                            }
                            4 => operations[0].description.push('!'),
                            5 => operations.push(Definition {
                                id: harmos::Identifier::new("fixture.extra").unwrap(),
                                ..operations[0].clone()
                            }),
                            6 => operations.push(operations[0].clone()),
                            _ => operations[0].policy.timeout = Duration::ZERO,
                        }
                        let mut source = source(Arc::clone(&counts));
                        source.operations = operations;
                        let mut fixture = Managed::new(source, Lanes::NativeOpen).await;
                        assert!(fixture.initialize().await.is_err());
                        let status = service_status(
                            fixture.runtime.operations.watch_residents(),
                            "operation-handshake",
                            |status| matches!(status, ServiceStatus::Inactive { .. }),
                        )
                        .await;
                        let ServiceStatus::Inactive {
                            cause: ServiceExit::Application(reason),
                            ..
                        } = status
                        else {
                            panic!(
                                "operation admission must fail before initialization: {status:?}"
                            );
                        };
                        let expected_reason = match change {
                            6 => "appears more than once",
                            7 => "operation deadlines must be positive",
                            _ => "native canonical contracts differ from the admitted operations",
                        };
                        assert!(reason.contains(expected_reason), "{reason}");
                        assert_eq!(counts.attached.load(Ordering::SeqCst), 0);
                        assert_eq!(counts.initialized.load(Ordering::SeqCst), 0);
                        assert_eq!(counts.cleaned.load(Ordering::SeqCst), 0);
                        assert!(!*fixture.host.ready.as_ref().unwrap().borrow());
                        assert!(matches!(
                            fixture.runtime.stop().await,
                            Err(harmos::Error::TaskFailed {
                                domain: "service",
                                ..
                            })
                        ));
                    }
                });
        });
    }
    {
        scenario_runtime::check("catalog_order_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut source = source(Arc::clone(&counts));
                    source.operations.reverse();
                    let mut fixture = Managed::new(source, Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        fixture
                            .host
                            .ready
                            .as_mut()
                            .unwrap()
                            .wait_for(|ready| *ready),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(counts.attached.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
                    assert!(!*fixture.host.ready.as_ref().unwrap().borrow());
                });
        });
    }
    {
        scenario_runtime::check("runner_retry_finalization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    fixture.call_as::<Read>(()).await.unwrap();
                    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("portable_call_context", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    fixture.call_as::<Read>(()).await.unwrap();
                    let mut context = harmos::operations::CallContext::default();
                    context.insert(Principal).unwrap();
                    context
                        .insert_portable(Tenant { name: "lab".into() })
                        .unwrap();
                    fixture.call_in::<Read>(context, ()).await.unwrap();
                    assert_eq!(
                        *counts.tenants.lock().unwrap(),
                        [None, None, Some("lab".to_owned()), Some("lab".to_owned())],
                        "every retried attempt carries the admitted values; local ones stay"
                    );
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("submitted_child_controls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    counts.submit_child.store(true, Ordering::SeqCst);
                    counts.refresh_hold.store(true, Ordering::SeqCst);
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    harmos::builtin::Catalog::register(
                        &fixture.runtime.operations,
                        "host",
                        Lanes::NativeOpen,
                    )
                    .unwrap();
                    fixture.initialize().await.unwrap();
                    fixture.call_as::<Read>(()).await.unwrap();
                    assert_eq!(counts.children.load(Ordering::SeqCst), 4);
                    assert_eq!(counts.refresh_completed.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.refresh_cleaned.load(Ordering::SeqCst), 4);
                    let refused = counts.detached_status.lock().unwrap().take().unwrap();
                    assert!(refused.contains("may not invoke operation"), "{refused}");
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cleanup_retry_once", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture =
                        Managed::new(source(counts.clone()), Lanes::NativeSerial).await;
                    fixture.initialize().await.unwrap();
                    let runtime = &fixture.runtime;
                    counts.fail_cleanup.store(true, Ordering::SeqCst);
                    assert!(matches!(
                        fixture.call_as::<Read>(()).await,
                        Err(harmos::Error::OperationFinalization { .. })
                    ));
                    assert!(runtime.operations.unregister(Read::ID).await.is_err());
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
                    counts.fail_cleanup.store(false, Ordering::SeqCst);
                    runtime.operations.unregister(Read::ID).await.unwrap();
                    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("preparation_refusal_state", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    counts.fail_prepare.store(true, Ordering::SeqCst);
                    let Err(harmos::Error::OperationFailed { source, .. }) =
                        fixture.call_as::<Read>(()).await
                    else {
                        panic!("preparation is refused before any attempt")
                    };
                    assert!(matches!(
                        source.downcast_ref::<harmos::operations::NativeError>(),
                        Some(harmos::operations::NativeError::Preparation { .. })
                    ));
                    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("children_attempt_lifetime", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for serial in [false, true] {
                        let counts = Arc::new(Counts::default());
                        counts.call_child.store(true, Ordering::SeqCst);
                        let lane = if serial {
                            Lanes::NativeSerial
                        } else {
                            Lanes::NativeOpen
                        };
                        let mut fixture = Managed::new(source(counts.clone()), lane).await;
                        fixture.initialize().await.unwrap();
                        let result = fixture.call_as::<Read>(()).await;
                        if serial {
                            assert!(matches!(result, Err(harmos::Error::OperationFailed { .. })));
                            assert_eq!(counts.children.load(Ordering::SeqCst), 0);
                        } else {
                            result.unwrap();
                            assert_eq!(counts.children.load(Ordering::SeqCst), 2);
                            assert_eq!(counts.stale_refused.load(Ordering::SeqCst), 1);
                            assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
                            assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
                        }
                        let saved = counts.saved.lock().unwrap().take().unwrap();
                        assert!(saved.call_as::<Refresh>(()).await.is_err());
                        fixture.shutdown().await.unwrap();
                        fixture.runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("initialization_failure_skips_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    counts.fail_initialize.store(true, Ordering::SeqCst);
    let Err(harmos::Error::OperationFailed { source, .. }) = fixture.call_as::<Read>(()).await else {
        panic!("initialization returns the original failure")
    };
    assert!(
        matches!(source.downcast_ref::<harmos::operations::NativeError>(),
        Some(harmos::operations::NativeError::Preparation(failure)) if failure.code() == harmos::operations::Failure::Transient && failure.cause().contains("partial initialization failed"))
    );
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
    // A failed or cancelled initialization owns no instance to clean up.
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("initialization_child_expires", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    counts.initialization_child.store(true, Ordering::SeqCst);
                    fixture.call_as::<Read>(()).await.unwrap();
                    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.children.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.stale_refused.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_init_resources", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    fixture.initialize().await.unwrap();
                    counts.block_initialize.store(true, Ordering::SeqCst);
                    let mut invocation = fixture
                        .runtime
                        .operations
                        .submit_encoded(Read::ID, Vec::new())
                        .unwrap();
                    tokio::time::timeout(
                        Duration::from_secs(1),
                        counts.initialization_entered.notified(),
                    )
                    .await
                    .unwrap();
                    assert_eq!(
                        invocation.status().phase(),
                        harmos::operations::OperationPhase::Running
                    );
                    invocation.cancel();
                    let result = tokio::time::timeout(Duration::from_secs(1), &mut invocation)
                        .await
                        .unwrap();
                    assert!(matches!(
                        result,
                        Err(harmos::Error::OperationRefused(harmos::Refusal::Cancelled))
                    ));
                    assert_eq!(
                        invocation.status().phase(),
                        harmos::operations::OperationPhase::Settled
                    );
                    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
                    // A failed or cancelled initialization owns no instance to clean up.
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("grpc_original_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::num::NonZeroUsize;
                    let counts = Arc::new(Counts::default());
                    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
                    assert!(fixture.runtime.grpc().methods().is_empty());
                    fixture.initialize().await.unwrap();
                    assert_eq!(fixture.runtime.grpc().methods().len(), 4);
                    let settings = harmos::grpc::Settings {
                        address: "127.0.0.1:0".parse().unwrap(),
                        operations: harmos::grpc::server::Settings::default(),
                        reflection: true,
                        web: false,
                        allowed_origins: Vec::new(),
                        tls: None,
                        connections: NonZeroUsize::new(4).unwrap(),
                        calls_per_connection: NonZeroUsize::new(8).unwrap(),
                        request_timeout: Duration::from_secs(2),
                        shutdown_timeout: Duration::from_secs(2),
                    };
                    let attached = fixture.runtime.grpc().attach(settings).await.unwrap();
                    let address = attached.local_addr();
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    let mut client = tonic::client::Grpc::new(channel);
                    client.ready().await.unwrap();
                    let settings = harmos::grpc::server::Settings::default();
                    // Unknown Protobuf Fields Are Legal For Unit; Postcard Unit Rejects These Bytes.
                    let codec = harmos::grpc::Codec::<(), harmos::operations::Response<u32>>::new(
                        settings.decode,
                        settings.encode,
                    )
                    .unwrap();
                    client
                        .unary(
                            tonic::Request::new(harmos::operations::Response::new(42u32)),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.v1.Native/Read",
                            ),
                            codec,
                        )
                        .await
                        .unwrap();
                    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    assert!(fixture.runtime.grpc().methods().is_empty());
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
                });
        });
    }
}

/* === Tests === */
