/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "operation_lifecycle.rs"]
mod operation_lifecycle;

#[path = "streams.rs"]
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
use super::assembly;
use crate::sidecar::support::Bank;
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::{Declarations, OperationDeclaration, OperationDefinition as Definition, OperationPolicy, RetryPolicy};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginLifecycleAuthority,
    PluginManager, PluginTransition, Router,
};
use harmos_runtime::services::ServiceContext;
use harmos_runtime::sidecar::{Attach, Attached, Notice, Supervised};

use super::SampleProjection;

/* === Constants === */

const CAPACITY: usize = 2;
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<Counts>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

type RetirementGate = Arc<Mutex<Option<(Arc<Notify>, Arc<Notify>)>>>;
type Source = Advertised<harmos_sidecar::PluginNative<Endpoint>>;
pub(crate) type Resident<A> = Supervised<Bank, A, fn(&ServiceContext<Bank>, Notice)>;
type Backends<A> = PluginBackend<Backend<A>, ()>;

/* === Structs === */

#[derive(Default)]
struct Counts {
    cancellation: Arc<crate::operations::cancellation::Probe>,
    attached: AtomicUsize,
    initialized: AtomicUsize,
    fail_initialization: AtomicBool,
    cleaned: AtomicUsize,
    started: AtomicUsize,
    block_attempt: AtomicBool,
    attempt_entered: Notify,
    runs: AtomicUsize,
    finished: AtomicUsize,
    call_child: AtomicBool,
    children: AtomicUsize,
    stale_refused: AtomicUsize,
    saved: Mutex<Option<harmos_runtime::sidecar::Supervisor>>,
    tenants: Mutex<Vec<Option<String>>>,
    submit_child: AtomicBool,
    refresh_hold: AtomicBool,
    refresh_started: Notify,
    refresh_release: Notify,
    refresh_completed: AtomicUsize,
    refresh_finished: Notify,
    refresh_cleaned: AtomicUsize,
    refresh_cleanup: Notify,
    detached_status: Mutex<Option<String>>,
}

#[harmos_runtime::message]
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

#[harmos_runtime::operation(resources = [Counts, harmos_runtime::sidecar::Supervisor], operations = [RefreshDeclaration], id = "device.read",
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: harmos_runtime::operations::Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: harmos_runtime::operations::Jitter::None,
            },
        },
    },
    response = (),
    description = "Reads a device",
    visibility = Public,
    grpc(package = "fixture.v1", service = "Native", method = "Read"),
)]
struct ReadDeclaration {}

struct ReadGuard(Arc<Counts>);

struct RefreshGuard(Arc<Counts>);

#[harmos_runtime::operation(resources = [Counts], id = "device.refresh",  policy = ReadDeclaration::POLICY, response = ())]
struct RefreshDeclaration {}

#[derive(Clone)]
struct Advertised<S> {
    source: S,
    operations: Vec<Definition>,
    counts: Arc<Counts>,
    seed: Arc<Seed>,
}

struct Backend<A: Attach> {
    services: Option<harmos_runtime::operations::OperationManager<super::support::Lanes>>,
    retirement_gate: RetirementGate,
    resident: Option<Resident<A>>,
    lease: Option<harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
    sdk: Option<harmos_runtime::operations::PluginSdkRegistration>,
    initializing: Option<harmos_runtime::operations::EncodedInvocation>,
    shutting_down: Option<harmos_runtime::operations::EncodedInvocation>,
    registered: bool,
    initialized: bool,
    shutdown_attempted: bool,
}

struct Host<A: Attach> {
    ready: Option<tokio::sync::watch::Receiver<bool>>,
    resident: Resident<A>,
    lane: Lanes,
    capacity: std::num::NonZeroUsize,
    supervision: harmos_runtime::ServicePolicy,
    operations: harmos_runtime::operations::OperationManager<super::support::Lanes>,
    retirement_gate: RetirementGate,
}

pub(crate) struct Managed<A: Attach> {
    pub(crate) runtime: harmos_runtime::Runtime<Bank>,
    sample_projection: Option<Arc<SampleProjection>>,
    manager: PluginManager<Backend<A>, ()>,
    host: Host<A>,
}

pub(crate) struct Calling(pub(crate) harmos_runtime::operations::EncodedInvocation);

/* === Enums === */

pub(crate) use super::support::Lanes;

#[harmos_runtime::catalog]
enum Operations {
    Cancellation(crate::operations::cancellation::Finish),
    CancellationExchange(crate::operations::cancellation::Exchange),
    RefreshDeclaration(RefreshDeclaration),
    ReadDeclaration(ReadDeclaration),
    Upload(streams::UploadDeclaration),
    Download(streams::DownloadDeclaration),
    Exchange(streams::ExchangeDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl harmos_runtime::operations::ContextValue for Tenant {
    const KEY: &'static str = "fixture.tenant";
}

impl harmos_runtime::Operation for ReadDeclaration {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let counts = ctx.resource::<Counts>();
        let context = ctx.resource::<harmos_runtime::sidecar::Supervisor>();
        counts.started.fetch_add(1, Ordering::SeqCst);
        let _guard = ReadGuard(counts.clone());
        counts.attempt_entered.notify_one();
        if counts.block_attempt.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if counts.submit_child.load(Ordering::SeqCst) {
            return submitted(&context, &counts).await.map(Into::into).map_err(std::io::Error::other);
        }
        let attempt = counts.runs.fetch_add(1, Ordering::SeqCst);
        let tenant = context.context().value::<Tenant>().map_err(std::io::Error::other)?;
        assert!(context.context().get::<Principal>().is_none());
        counts.tenants.lock().unwrap().push(tenant.map(|tenant| tenant.name));
        if counts.call_child.load(Ordering::SeqCst) {
            let previous = counts.saved.lock().unwrap().take();
            if let Some(previous) = previous {
                assert!(previous.call::<RefreshDeclaration>(RefreshDeclaration {}).await.is_err());
                counts.stale_refused.fetch_add(1, Ordering::SeqCst);
            }
            *counts.saved.lock().unwrap() = Some((*context).clone());
            context.call::<RefreshDeclaration>(RefreshDeclaration {}).await.map_err(std::io::Error::other)?;
        }
        if attempt.is_multiple_of(2) {
            return Err(std::io::Error::other("one transient failure"));
        }
        Ok(().into())
    }
    fn classify(_: &Self::Error) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
}
impl Drop for ReadGuard {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
    }
}

impl harmos_runtime::Operation for RefreshDeclaration {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let counts = ctx.resource::<Counts>();
        let _guard = RefreshGuard(counts.clone());
        counts.children.fetch_add(1, Ordering::SeqCst);
        if counts.refresh_hold.load(Ordering::SeqCst) {
            counts.refresh_started.notify_one();
            counts.refresh_release.notified().await;
            counts.refresh_completed.fetch_add(1, Ordering::SeqCst);
            counts.refresh_finished.notify_one();
        }
        Ok(().into())
    }
}
impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.0.refresh_cleaned.fetch_add(1, Ordering::SeqCst);
        self.0.refresh_cleanup.notify_one();
    }
}

impl harmos_operations::lifecycle::Policy for Endpoint {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for Endpoint {
    type Config = usize;
    type Context = harmos_runtime::sidecar::Supervisor;
    type Error = std::io::Error;

    async fn prepare(id: usize, setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Self::Error> {
        let counts = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|counts| counts.upgrade())
            .ok_or_else(|| std::io::Error::other("original fixture resources unavailable"))?;
        let plugin = Self { counts };
        setup.resource::<Counts>(plugin.counts.clone());
        setup.resource::<crate::operations::cancellation::Probe>(plugin.counts.cancellation.clone());
        streams::configure(setup.settings()).map_err(std::io::Error::other)?;
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.counts.initialized.fetch_add(1, Ordering::SeqCst);
        if self.counts.fail_initialization.load(Ordering::SeqCst) {
            let source = std::io::Error::from(std::io::ErrorKind::NotFound);
            let failure = harmos_domain::Failure::originate(
                harmos_runtime::operations::FailureKind::Permanent,
                &source,
                harmos_domain::failure::Origin::Plugin,
            )
            .with_detail("secret", "api_token");
            return Err(std::io::Error::other(harmos_sidecar::Refusal::failed(-32_100, failure)));
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.counts.cleaned.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "operation_handshake", requires = ())]
impl harmos_runtime::Plugin for Endpoint {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Operations::CATALOG];
}

impl Drop for Calling {
    fn drop(&mut self) {
        self.0.request_cancel();
    }
}

impl std::future::Future for Calling {
    type Output = harmos_runtime::Result<Vec<u8>>;
    fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
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
                artifact: PluginArtifact::LocalSidecar { revision: "fixture".into() },
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
        Self::from_definition(source, definitions, lane, std::num::NonZeroUsize::new(CAPACITY).unwrap(), vec![], std::convert::identity)
            .await
    }
}

impl<A: Attach> Managed<A> {
    pub(crate) fn drop_manager(self) -> (harmos_runtime::Runtime<Bank>, harmos_runtime::plugin::PluginCleanup) {
        let Self {
            runtime,
            manager,
            host,
            sample_projection,
        } = self;
        let cleanup = manager.cleanup();
        drop(manager);
        drop(host);
        drop(sample_projection);
        (runtime, cleanup)
    }

    pub(crate) async fn attach(
        source: A, configuration: Vec<u8>, lane: Lanes, capacity: std::num::NonZeroUsize, demand_keys: Vec<String>,
        configure: impl FnOnce(Resident<A>) -> Resident<A>,
    ) -> Self {
        let definitions = inspected(&source, configuration);
        Self::from_definition(source, definitions, lane, capacity, demand_keys, configure).await
    }
    pub(crate) async fn from_definition(
        source: A, definitions: PluginDefinitions, lane: Lanes, capacity: std::num::NonZeroUsize, demand_keys: Vec<String>,
        configure: impl FnOnce(Resident<A>) -> Resident<A>,
    ) -> Self {
        let resident = configure(Supervised::new(source, notices as fn(&ServiceContext<Bank>, Notice)).demand_keys(demand_keys));
        let sample_projection = Arc::new(SampleProjection { streams: std::sync::OnceLock::new() });
        let runtime = assembly(|setup| {
            setup.resource::<SampleProjection>(Arc::clone(&sample_projection));
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            });
        })
        .await;
        sample_projection.streams.set(runtime.streams.clone()).expect("the runtime initializes the sample projection once");
        Self::from_runtime_inner(runtime, definitions, resident, lane, capacity, harmos_runtime::services::POLICY, Some(sample_projection))
    }
    pub(crate) fn from_runtime(
        runtime: harmos_runtime::Runtime<Bank>, definitions: PluginDefinitions, resident: Resident<A>, lane: Lanes,
        capacity: std::num::NonZeroUsize, supervision: harmos_runtime::ServicePolicy,
    ) -> Self {
        Self::from_runtime_inner(runtime, definitions, resident, lane, capacity, supervision, None)
    }
    pub(crate) fn from_runtime_with_projection(
        runtime: harmos_runtime::Runtime<Bank>, definitions: PluginDefinitions, resident: Resident<A>, lane: Lanes,
        capacity: std::num::NonZeroUsize, supervision: harmos_runtime::ServicePolicy, sample_projection: Arc<SampleProjection>,
    ) -> Self {
        sample_projection.streams.set(runtime.streams.clone()).expect("the runtime initializes the sample projection once");
        Self::from_runtime_inner(runtime, definitions, resident, lane, capacity, supervision, Some(sample_projection))
    }
    fn from_runtime_inner(
        runtime: harmos_runtime::Runtime<Bank>, definitions: PluginDefinitions, resident: Resident<A>, lane: Lanes,
        capacity: std::num::NonZeroUsize, supervision: harmos_runtime::ServicePolicy, sample_projection: Option<Arc<SampleProjection>>,
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
            sample_projection,
            manager: PluginManager::new(definitions).unwrap(),
            host,
        }
    }
    pub(crate) fn resident(&self) -> &Resident<A> {
        &self.host.resident
    }
    pub(crate) fn status(&self, id: &str) -> Option<harmos_runtime::plugin::PluginStatus> {
        self.manager.status(id)
    }
    pub(crate) async fn publication(&self) -> harmos_runtime::Result<()> {
        let selected =
            harmos_runtime::operations::WireDeclaration::of::<harmos_operations::streams::Publish>().contract().map_err(|source| {
                harmos_runtime::Error::OperationFailed {
                    operation: "host.publish".into(),
                    source: Arc::new(source),
                }
            })?;
        let required = self
            .manager
            .definitions()
            .any(|definition| definition.contributions.required_contracts.iter().flatten().any(|required| required.contract == selected));
        if !required {
            return Err(harmos_runtime::Error::InvalidInput {
                field: "fixture publication provider",
                reason: "provider differs from the original selected native contract",
            });
        }
        self.runtime.operations.register::<crate::sidecar::PublicationOperations>(self.host.lane).await
    }

    pub(crate) async fn call<O>(&self, request: O::Request) -> harmos_runtime::Result<O::Response>
    where
        O: OperationDeclaration,
    {
        self.call_in::<O>(harmos_runtime::operations::CallContext::default(), request).await
    }

    pub(crate) async fn call_in<O>(
        &self, context: harmos_runtime::operations::CallContext, request: O::Request,
    ) -> harmos_runtime::Result<O::Response>
    where
        O: OperationDeclaration,
    {
        let contract =
            harmos_runtime::operations::WireDeclaration::of::<O>().contract().map_err(|source| harmos_runtime::Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            })?;
        let selected = self.manager.definitions().find_map(|definition| {
            definition
                .contributions
                .wire_contracts
                .iter()
                .find(|selected| selected.operation().id.as_str() == O::ID)
                .map(|selected| (definition.id.as_str(), selected))
        });
        if selected.map(|(_, contract)| contract) != Some(&contract) {
            return Err(harmos_runtime::Error::InvalidInput {
                field: "fixture wire operation",
                reason: "caller differs from the original frozen contract",
            });
        }
        let payload = harmos_runtime::Message::encode(&request);
        let target = harmos_runtime::QualifiedId::qualify(selected.expect("validated fixture contract").0, O::ID)
            .expect("fixture uses portable owner and local identities");
        let invocation = self.runtime.operations.with_context(context).submit_encoded(target.as_str(), payload)?;
        let reply = Calling(invocation).await?;
        harmos_runtime::Message::decode(&reply)
            .map_err(|source| harmos_runtime::Error::OperationPayload { operation: O::ID.into(), source })
    }
    pub(crate) async fn initialize(&mut self) -> harmos_runtime::Result<()> {
        self.manager.initialize(&mut self.host).await
    }
    pub(crate) fn pause_retirement(&mut self, entered: Arc<Notify>, release: Arc<Notify>) {
        *self.host.retirement_gate.lock().unwrap() = Some((entered, release));
    }
    pub(crate) async fn shutdown(&mut self) -> harmos_runtime::Result<()> {
        self.manager.shutdown(&mut self.host).await
    }
    pub(crate) async fn reload(&mut self, configuration: Vec<u8>) -> harmos_runtime::Result<()> {
        let mut definition = self.manager.definitions().next().expect("one fixture definition").clone();
        definition.configuration = configuration;
        let candidate = PluginDefinitions::new([definition], [])?;
        self.manager.reload(candidate, &mut self.host).await
    }
    pub(crate) fn ready(&mut self) -> &mut tokio::sync::watch::Receiver<bool> {
        self.host.ready.as_mut().expect("the original resident was staged")
    }
    pub(crate) fn demands(&self) -> harmos_runtime::sidecar::DemandSource {
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
    type Sidecar = Backend<A>;
    type Embedded = ();
    fn preflight(&mut self, definitions: &PluginDefinitions) -> harmos_runtime::Result<()> {
        self.operations.validate_requirements(definitions)
    }
    fn prepare(&mut self, _: &PluginDefinition) -> harmos_runtime::Result<Backends<A>> {
        Ok(PluginBackend::Sidecar(Backend {
            services: Some(self.operations.clone()),
            retirement_gate: self.retirement_gate.clone(),
            ..Backend::default()
        }))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> harmos_runtime::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends<A>,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(backend) = backend else { unreachable!() };
        let resident = self.resident.clone().routed(router);
        self.ready = Some(resident.ready());
        backend.resident = Some(resident.clone());
        self.operations.register_service(
            harmos_runtime::services::ServiceDefinition {
                id: definition.id.clone(),
                policy: self.supervision,
            },
            move |_: Arc<harmos_sdk::ExecutionContext>| resident.clone(),
        )?;
        backend.registered = true;
        backend.lease = Some(self.operations.submit_resident(&definition.id).await?);
        let native =
            backend.resident.as_ref().unwrap().staged_operations().await.map_err(|source| harmos_runtime::Error::OperationFailed {
                operation: definition.id.clone(),
                source: Arc::new(source),
            })?;
        let native = Arc::new(native);
        backend.registration = Some(
            self.operations
                .stage_plugin(native.scope().unwrap(), |bindings| bindings.register_native(self.lane, native.clone(), self.capacity))?,
        );
        backend.sdk = Some(native.stage_sdk(&self.operations, self.lane)?);
        backend.initializing = Some(
            backend.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                std::num::NonZeroUsize::new(
                    2 * definition.configuration.len() + std::mem::size_of::<harmos_operations::lifecycle::Initialize>() + 64 + 30 + 3072,
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
        definition: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends<A>,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(backend) = backend else { unreachable!() };
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
            backend.services.as_ref().unwrap().unregister_resident(&definition.id).await?;
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

    fn operations(&self) -> Result<Vec<Definition>, harmos_domain::Error> {
        Ok(self.operations.clone())
    }

    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        self.source.publications()
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_operations::lifecycle::Descriptor; 2]>> {
        self.source.sdk_lifecycle()
    }
    fn observers(&self) -> Vec<harmos_domain::inventory::observers::Declaration> {
        self.source.observers()
    }
    fn observe_contract(&self) -> harmos_domain::Result<Option<harmos_runtime::operations::Contract>> {
        self.source.observe_contract()
    }
    fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
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
        artifact: PluginArtifact::LocalSidecar { revision: "fixture".into() },
        configuration,
        requires: vec![],
        contributions: PluginContributions {
            operations: source.operations().unwrap(),
            wire_contracts: source.operation_contracts().unwrap().unwrap(),
            required_contracts: source.required_contracts().unwrap(),
            lifecycle: source.sdk_lifecycle().unwrap(),
            streams: source.publications(),
            observers: source.observers(),
            listeners: source.listener_declarations().unwrap().unwrap_or_default(),
            ..PluginContributions::default()
        },
    }
}

/* === Private Functions === */

fn notices(_: &ServiceContext<Bank>, _: Notice) {}

// Drives submitted children through the sidecar binding of the host.
async fn submitted(host: &harmos_runtime::sidecar::Supervisor, counts: &Counts) -> Result<(), harmos_sidecar::CallError> {
    use harmos_runtime::operations::instance::Phase;
    use harmos_runtime::operations::{Host, Instance};

    // Status, Then Join
    let joined = host.submit::<RefreshDeclaration>(RefreshDeclaration {}).await?;
    counts.refresh_started.notified().await;
    let status = joined.status().await?;
    assert_eq!(status.phase, Phase::Running);
    counts.refresh_release.notify_one();
    joined.join().await?;

    // Cancel, Then Join
    let cancelled = host.submit::<RefreshDeclaration>(RefreshDeclaration {}).await?;
    counts.refresh_started.notified().await;
    assert!(Instance::cancel(cancelled).await.is_err());

    // Dropping The Handle Releases The Child
    let dropped = host.submit::<RefreshDeclaration>(RefreshDeclaration {}).await?;
    counts.refresh_started.notified().await;
    let cleaned = counts.refresh_cleaned.load(Ordering::SeqCst);
    drop(dropped);
    (async {
        while counts.refresh_cleaned.load(Ordering::SeqCst) == cleaned {
            counts.refresh_cleanup.notified().await;
        }
    })
    .await;

    // Detaching Keeps The Child Running Within This Attempt
    let detached = host.submit::<RefreshDeclaration>(RefreshDeclaration {}).await?;
    counts.refresh_started.notified().await;
    let id = Instance::detach(detached);
    // Built-ins are ordinary requirements: an undeclared caller is refused.
    let refused = host.call::<harmos_operations::lifecycle::Status>(harmos_operations::lifecycle::Status::of(id)).await;
    *counts.detached_status.lock().unwrap() = refused.err().map(|error| {
        std::iter::successors(Some(&error as &(dyn std::error::Error + 'static)), |error| error.source())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(": ")
    });
    counts.refresh_release.notify_one();
    // Without a handle the child still runs while this attempt lives.
    (async {
        while counts.refresh_completed.load(Ordering::SeqCst) < 2 {
            counts.refresh_finished.notified().await;
        }
    })
    .await;
    Ok(())
}

fn source(counts: Arc<Counts>) -> Source {
    let id = NEXT_SEED.try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1)).unwrap();
    {
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "bounded fixture resource handoff is full");
        seeds.insert(id, Arc::downgrade(&counts));
    }
    let source = harmos_sidecar::PluginNative::<Endpoint>::new(std::num::NonZeroUsize::new(CAPACITY).unwrap()).unwrap();
    let operations = source.operations().unwrap();
    Advertised {
        source,
        operations,
        counts,
        seed: Arc::new(Seed { id }),
    }
}

/* === Tests === */
