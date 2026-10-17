/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "dependency_admission.rs"]
mod dependency_admission;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::lifecycle::{GuestOperations, Lifecycle, Resources};
use super::sandbox::{Echo, Echoed, Fail, ProviderEcho};
use super::{Bank, GuestLanes, component_error, execution_error, retire_failed_guest, runtime};
use harmos_runtime::Runtime;
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::{OperationDeclaration, OperationPolicy};
use harmos_sidecar::Attach;

/* === Constants === */
const NATIVE_CAPACITY: usize = 64;
static NEXT_NATIVE_SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NATIVE_SEEDS: std::sync::Mutex<std::collections::BTreeMap<u64, std::sync::Weak<ProviderOwner>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[harmos_runtime::catalog]
enum ProviderOperations {
    Echo(ProviderEcho),
}

#[derive(Default)]
struct ProviderOwner {
    prepared: std::sync::atomic::AtomicUsize,
    cleanup_attempts: std::sync::atomic::AtomicUsize,
    cleanup_finished: std::sync::atomic::AtomicUsize,
    started: tokio::sync::Notify,
    cancelled: std::sync::Arc<tokio::sync::Notify>,
    observed: std::sync::Mutex<Vec<String>>,
    hold_cleanup: std::sync::atomic::AtomicBool,
    fail_cleanup: std::sync::atomic::AtomicBool,
    cleanup_started: tokio::sync::Notify,
    cleanup_release: tokio::sync::Notify,
}
struct Cancelled(std::sync::Arc<tokio::sync::Notify>);
impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
#[harmos_runtime::catalog]
enum EmbeddedOperations {
    Read(EmbeddedReading),
}

struct EmbeddedReading {
    resources: std::sync::Arc<ProviderOwner>,
    cleanup: bool,
}

impl harmos_runtime::operations::Operation for EmbeddedReading {
    type Declaration = ProviderEcho;
    type Resources = ProviderOwner;
    type Error = harmos_domain::Refusal;
    type Endpoints = ();
    type Context<'a> = harmos_runtime::operations::Context<'a>;
    async fn initialize<'ctx>(resources: std::sync::Arc<ProviderOwner>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        resources.prepared.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self { resources, cleanup: false })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut ProviderEcho, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            self.cleanup = request.say == "physical-cleanup";
            self.resources.answer(request).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.cleanup(self.cleanup).await
    }
}
impl ProviderOwner {
    async fn answer(&self, state: &ProviderEcho) -> Result<Echoed, harmos_domain::Refusal> {
        if state.say == "cancel" {
            let _cancelled = Cancelled(self.cancelled.clone());
            self.started.notify_one();
            std::future::pending::<()>().await;
        }
        if state.say == "timeout" {
            std::future::pending::<()>().await;
        }
        if state.say == "refuse" {
            return Err(harmos_domain::Refusal::new(73, "provider refusal"));
        }
        self.observed.lock().unwrap().push(state.say.clone());
        Ok(Echoed { said: state.say.clone() })
    }
    async fn cleanup(&self, physical: bool) -> Result<(), harmos_domain::Refusal> {
        use std::sync::atomic::Ordering;
        if !physical {
            return Ok(());
        }
        self.cleanup_attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail_cleanup.load(Ordering::SeqCst) {
            return Err(harmos_domain::Refusal::new(1, "fixture physical cleanup refused"));
        }
        if self.hold_cleanup.swap(false, Ordering::SeqCst) {
            self.cleanup_started.notify_one();
            self.cleanup_release.notified().await;
        }
        self.cleanup_finished.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[harmos_runtime::message]
#[harmos_runtime::operation(id = "client.forward", version = 1, response = Echoed, policy = Echo::POLICY,
    visibility = harmos_runtime::operations::Visibility::Shared)]
#[derive(Clone)]
struct Forward {
    say: String,
}

#[harmos_runtime::catalog]
enum ForwardOperations {
    Forward(Forward),
}
struct NativeProvider(std::sync::Arc<ProviderOwner>);
struct NativeClient;
struct NativeReading {
    resources: std::sync::Arc<NativeProvider>,
    cleanup: bool,
}
impl harmos_runtime::operations::Operation for NativeReading {
    type Declaration = ProviderEcho;
    type Resources = NativeProvider;
    type Error = harmos_domain::Refusal;
    type Endpoints = ();
    type Context<'a> = &'a harmos_sidecar::Supervisor;
    async fn initialize<'ctx>(resources: std::sync::Arc<NativeProvider>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        resources.0.prepared.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self { resources, cleanup: false })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut ProviderEcho, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            self.cleanup = request.say == "physical-cleanup";
            self.resources.0.answer(request).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.0.cleanup(self.cleanup).await
    }
}
struct NativeForwarding {
    _resources: std::sync::Arc<NativeClient>,
}
impl harmos_runtime::operations::Operation for NativeForwarding {
    type Declaration = Forward;
    type Resources = NativeClient;
    type Error = harmos_domain::Refusal;
    type Endpoints = ();
    type Context<'a> = &'a harmos_sidecar::Supervisor;
    async fn initialize<'ctx>(resources: std::sync::Arc<NativeClient>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut Forward, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            host.call(ProviderEcho { say: request.say.clone() })
                .await
                .map(harmos_runtime::operations::Response::into_owned)
                .map_err(Self::Error::of)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
struct NativeSeed {
    id: u64,
}
impl NativeSeed {
    fn new(provider: &std::sync::Arc<ProviderOwner>) -> std::io::Result<Self> {
        use std::sync::atomic::Ordering;
        let id = NEXT_NATIVE_SEED
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
            .map_err(|_| std::io::Error::other("native fixture resource identities exhausted"))?;
        let mut seeds = NATIVE_SEEDS.lock().unwrap();
        if seeds.len() >= NATIVE_CAPACITY {
            return Err(std::io::Error::other("native fixture resource handoff is full"));
        }
        seeds.insert(id, std::sync::Arc::downgrade(provider));
        Ok(Self { id })
    }
}
impl Drop for NativeSeed {
    fn drop(&mut self) {
        NATIVE_SEEDS.lock().unwrap().remove(&self.id);
    }
}
struct NativeProviderPlugin {
    resources: std::sync::Arc<NativeProvider>,
}
impl harmos_runtime::operations::lifecycle::Policy for NativeProviderPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}
#[harmos_runtime::plugin(id = "provider", provides = ProviderOperations, requires = ())]
impl harmos_runtime::Plugin for NativeProviderPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = u64;
    type Error = std::io::Error;
    type Resources = NativeProvider;
    fn prepare(id: u64) -> Result<Self, Self::Error> {
        let provider = NATIVE_SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|resource| resource.upgrade())
            .ok_or_else(|| std::io::Error::other("the original native fixture resource is unavailable"))?;
        Ok(Self { resources: std::sync::Arc::new(NativeProvider(provider)) })
    }
    fn resources(&self) -> std::sync::Arc<NativeProvider> {
        self.resources.clone()
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bind_operations(
        &self, table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<NativeReading>(self.resources())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct NativeClientPlugin {
    resources: std::sync::Arc<NativeClient>,
}
#[harmos_runtime::catalog]
enum NativeRequirements {
    Echo(Echo),
}
impl harmos_runtime::operations::lifecycle::Policy for NativeClientPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}
#[harmos_runtime::plugin(id = "client", provides = ForwardOperations, requires = NativeRequirements)]
impl harmos_runtime::Plugin for NativeClientPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = ();
    type Error = std::io::Error;
    type Resources = NativeClient;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self { resources: std::sync::Arc::new(NativeClient) })
    }
    fn resources(&self) -> std::sync::Arc<NativeClient> {
        self.resources.clone()
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bind_operations(
        &self, table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<NativeForwarding>(self.resources())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[derive(Clone)]
struct NativeAttachment {
    id: String,
    _seed: Option<std::sync::Arc<NativeSeed>>,
}
impl harmos_sidecar::Attach for NativeAttachment {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        &self.id
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::Definition>> {
        if self.id == "provider" { native_provider().operations() } else { native_client().operations() }
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        if self.id == "provider" { native_provider().operation_contracts() } else { native_client().operation_contracts() }
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        if self.id == "provider" { native_provider().required_contracts() } else { native_client().required_contracts() }
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
        if self.id == "provider" { native_provider().sdk_lifecycle() } else { native_client().sdk_lifecycle() }
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        if self.id == "provider" { native_provider().attach().await } else { native_client().attach().await }
    }
}
fn native_provider() -> harmos_sidecar::PluginNative<NativeProviderPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap()).unwrap()
}
fn native_client() -> harmos_sidecar::PluginNative<NativeClientPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap()).unwrap()
}
fn dependency_notice(_: &harmos_runtime::services::Service<Bank>, _: harmos_runtime::sidecar::Notice) {}
type NativeResident = harmos_runtime::sidecar::Supervised<
    Bank,
    NativeAttachment,
    fn(&harmos_runtime::services::Service<Bank>, harmos_runtime::sidecar::Notice),
>;
#[derive(Default)]
struct NativeResources {
    services: Option<harmos_runtime::operations::OperationManager>,
    sdk: Option<harmos_runtime::operations::PluginSdkRegistration>,
    initializing: Option<harmos_runtime::operations::EncodedInvocation>,
    shutting_down: Option<harmos_runtime::operations::EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    resident: Option<NativeResident>,
    lease: Option<harmos_runtime::operations::OperationInstance<harmos_runtime::operations::resident::Activation>>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
    registered: bool,
}
type GuestResources = Resources;
#[derive(Default)]
struct ProviderResources {
    endpoint: Option<harmos_runtime::plugin::PluginEndpoint>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
}
type DependencyBackends = harmos_runtime::plugin::PluginBackend<GuestResources, NativeResources, ProviderResources>;
struct DependencyHost {
    native_seed: Option<std::sync::Arc<NativeSeed>>,
    guest: Lifecycle,
    operations: harmos_runtime::operations::OperationManager,
    provider: std::sync::Arc<ProviderOwner>,
}
impl DependencyHost {
    fn new(runtime: &Runtime<Bank>, provider: std::sync::Arc<ProviderOwner>) -> Self {
        Self { native_seed: None, guest: Lifecycle::sandbox(runtime), operations: runtime.operations.clone(), provider }
    }
}
impl harmos_runtime::plugin::PluginLifecycle for DependencyHost {
    type Guest = GuestResources;
    type Sidecar = NativeResources;
    type Embedded = ProviderResources;
    fn preflight(&mut self, candidate: &harmos_runtime::plugin::PluginDefinitions) -> harmos_runtime::Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, definition: &harmos_runtime::plugin::PluginDefinition) -> harmos_runtime::Result<DependencyBackends> {
        Ok(match definition.kind() {
            harmos_runtime::plugin::PluginKind::Guest => harmos_runtime::plugin::PluginBackend::Guest(self.guest.guest_seed()),
            harmos_runtime::plugin::PluginKind::Embedded => harmos_runtime::plugin::PluginBackend::Embedded(ProviderResources::default()),
            harmos_runtime::plugin::PluginKind::Sidecar => harmos_runtime::plugin::PluginBackend::Sidecar(NativeResources {
                services: Some(self.operations.clone()),
                ..NativeResources::default()
            }),
        })
    }
    async fn fence(
        &mut self, _: harmos_runtime::plugin::PluginTransition, _: &[harmos_runtime::plugin::PluginDefinition],
    ) -> harmos_runtime::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &harmos_runtime::plugin::PluginDefinition, router: &harmos_runtime::plugin::Router,
        authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>, backend: &mut DependencyBackends,
    ) -> harmos_runtime::Result<()> {
        match backend {
            harmos_runtime::plugin::PluginBackend::Embedded(resources) => {
                resources.endpoint = Some(router.enter(&definition.id).unwrap());
                let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
                resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                    bindings.register_wire_operations::<EmbeddedOperations, _>(GuestLanes::Open, self.provider.clone())
                })?);
            }
            harmos_runtime::plugin::PluginBackend::Guest(resources) => {
                resources.start::<GuestOperations, _>(&self.operations, definition, router, authority, GuestLanes::Open).await?;
            }
            harmos_runtime::plugin::PluginBackend::Sidecar(resources) => {
                let resident: NativeResident = harmos_runtime::sidecar::Supervised::new(
                    NativeAttachment {
                        id: definition.id.clone(),
                        _seed: if definition.id == "provider" { self.native_seed.take() } else { None },
                    },
                    dependency_notice as fn(&harmos_runtime::services::Service<Bank>, harmos_runtime::sidecar::Notice),
                )
                .routed(router);
                resources.resident = Some(resident.clone());
                let mut ready = resident.ready();
                self.operations.register_service(
                    harmos_runtime::services::ServiceDefinition { id: definition.id.clone(), policy: harmos_runtime::services::POLICY },
                    move |_: std::sync::Arc<()>| resident.clone(),
                )?;
                resources.registered = true;
                resources.lease = Some(self.operations.submit_resident(&definition.id).await?);
                let handle = if definition.contributions.lifecycle.is_some() {
                    let native = resources.resident.as_ref().unwrap().staged_operations().await.map_err(|source| {
                        harmos_runtime::Error::OperationFailed { operation: definition.id.clone(), source: std::sync::Arc::new(source) }
                    })?;
                    std::sync::Arc::new(native)
                } else {
                    ready.wait_for(|ready| *ready).await.unwrap();
                    std::sync::Arc::new(resources.resident.as_ref().unwrap().operations().unwrap())
                };
                resources.registration = Some(self.operations.stage_plugin(handle.scope().unwrap(), |bindings| {
                    if definition.id == "provider" {
                        bindings.register_native_catalog::<ProviderOperations>(
                            GuestLanes::Open,
                            handle.clone(),
                            std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap(),
                        )
                    } else {
                        bindings.register_native_catalog::<ForwardOperations>(
                            GuestLanes::Open,
                            handle.clone(),
                            std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap(),
                        )
                    }
                })?);
                if definition.contributions.lifecycle.is_some() {
                    resources.sdk = Some(handle.stage_sdk(&self.operations, GuestLanes::Open)?);
                    resources.initializing = Some(
                        resources.sdk.as_ref().unwrap().submit_initialize(
                            authority,
                            &definition.configuration,
                            std::num::NonZeroUsize::new(
                                2 * definition.configuration.len()
                                    + std::mem::size_of::<harmos_runtime::operations::lifecycle::Initialize>()
                                    + 64
                                    + 20,
                            )
                            .unwrap(),
                        )?,
                    );
                    let result = resources.initializing.as_mut().unwrap().await;
                    resources.initializing.take();
                    result?;
                    resources.initialized = true;
                }
            }
        }
        Ok(())
    }
    async fn stop(
        definition: &harmos_runtime::plugin::PluginDefinition, authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut DependencyBackends,
    ) -> harmos_runtime::Result<()> {
        match backend {
            harmos_runtime::plugin::PluginBackend::Guest(resources) => {
                resources.stop(authority).await?;
            }
            harmos_runtime::plugin::PluginBackend::Embedded(resources) => {
                if let Some(registration) = &mut resources.registration {
                    registration.shutdown().await?;
                }
                resources.registration.take();
                resources.endpoint.take();
            }
            harmos_runtime::plugin::PluginBackend::Sidecar(resources) => {
                if let Some(sdk) = &resources.sdk {
                    sdk.settle().await?;
                    if let Some(initializing) = &mut resources.initializing {
                        let result = initializing.await;
                        resources.initializing.take();
                        result?;
                        resources.initialized = true;
                    }
                    if resources.initialized && !resources.shutdown_attempted {
                        resources.shutting_down = Some(sdk.submit_shutdown(authority)?);
                        resources.shutdown_attempted = true;
                    }
                    if let Some(shutdown) = &mut resources.shutting_down {
                        let result = shutdown.await;
                        resources.shutting_down.take();
                        result?;
                        resources.initialized = false;
                    }
                }
                if let Some(registration) = &mut resources.registration {
                    registration.shutdown().await?;
                }
                if let Some(lease) = resources.lease.take() {
                    lease.release().await?;
                }
                if resources.registered {
                    resources.services.as_ref().unwrap().unregister_resident(&definition.id).await?;
                }
                resources.registered = false;
                if let Some(sdk) = &mut resources.sdk {
                    sdk.close().await?;
                }
                resources.sdk.take();
                resources.registration.take();
                resources.resident.take();
            }
        }
        Ok(())
    }
}
async fn dependency_candidate(lifecycle: &mut DependencyHost, configuration: &str) -> harmos_runtime::plugin::PluginDefinitions {
    lifecycle.guest.candidate(configuration).await
}

async fn native_dependency_candidate(
    lifecycle: &mut DependencyHost, configuration: &str, client: bool,
) -> harmos_runtime::plugin::PluginDefinitions {
    use harmos_runtime::plugin::{PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitions, Requirement};
    let mut definitions: Vec<_> = dependency_candidate(lifecycle, configuration).await.definitions().cloned().collect();
    assert!(lifecycle.native_seed.is_none());
    let seed = std::sync::Arc::new(NativeSeed::new(&lifecycle.provider).unwrap());
    let provider = definitions.iter_mut().find(|definition| definition.id == "provider").unwrap();
    provider.artifact = PluginArtifact::LocalSidecar { revision: "fixture".into() };
    provider.configuration = serde_json::to_vec(&seed.id).unwrap();
    let carrier = native_provider();
    provider.contributions = PluginContributions {
        lifecycle: carrier.sdk_lifecycle().unwrap(),
        operations: carrier.operations().unwrap(),
        wire_contracts: carrier.operation_contracts().unwrap().unwrap(),
        required_contracts: carrier.required_contracts().unwrap(),
        ..Default::default()
    };
    lifecycle.native_seed = Some(seed);
    if client {
        definitions.push(PluginDefinition {
            id: "client".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar { revision: "fixture".into() },
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                lifecycle: native_client().sdk_lifecycle().unwrap(),
                operations: native_client().operations().unwrap(),
                wire_contracts: native_client().operation_contracts().unwrap().unwrap(),
                required_contracts: native_client().required_contracts().unwrap(),
                ..Default::default()
            },
            requires: vec![Requirement { id: "sandbox-guest".into(), version: "*".parse().unwrap() }],
        });
    }
    PluginDefinitions::new(definitions, []).unwrap()
}
