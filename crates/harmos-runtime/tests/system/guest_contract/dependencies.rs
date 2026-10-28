/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "dependency_admission.rs"]
mod dependency_admission;

use super::lifecycle::{GuestOperations, Lifecycle, Resources};
use super::sandbox::{EchoOperation, Echoed, ProviderEchoOperation};
use super::{Bank, GuestLanes, component_error, retire_failed_guest, runtime};
use harmos_runtime::Runtime;
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::{OperationDeclaration, OperationPolicy};
use harmos_sidecar::Attach;

const NATIVE_CAPACITY: usize = 64;
static NEXT_NATIVE_SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NATIVE_SEEDS: std::sync::Mutex<std::collections::BTreeMap<u64, std::sync::Weak<ProviderOwner>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

type NativeResident = harmos_runtime::sidecar::Supervised<
    Bank,
    NativeAttachment,
    fn(&harmos_runtime::services::ServiceContext<Bank>, harmos_runtime::sidecar::Notice),
>;
type GuestResources = Resources;
type DependencyBackends = harmos_runtime::plugin::PluginBackend<GuestResources, NativeResources, ProviderResources>;

#[derive(Default)]
struct ProviderOwner {
    prepared: std::sync::atomic::AtomicUsize,
    started: tokio::sync::Notify,
    cancelled: std::sync::Arc<tokio::sync::Notify>,
    observed: std::sync::Mutex<Vec<String>>,
}

struct Cancelled(std::sync::Arc<tokio::sync::Notify>);

#[harmos_runtime::operation(resources = [NativeClient, harmos_sidecar::Supervisor], operations = [EchoOperation], id = "client.forward",  response = Echoed, policy = super::sandbox::EchoOperation::POLICY,
    visibility = Shared)]
#[derive(Clone)]
struct ForwardDeclaration {
    say: String,
}

struct NativeClient;

struct NativeSeed {
    id: u64,
}

struct NativeProviderPlugin {
    resources: std::sync::Arc<ProviderOwner>,
}

struct NativeClientPlugin {
    resources: std::sync::Arc<NativeClient>,
}

#[derive(Clone)]
struct NativeAttachment {
    id: String,
    _seed: Option<std::sync::Arc<NativeSeed>>,
}

#[derive(Default)]
struct NativeResources {
    services: Option<harmos_runtime::operations::OperationManager<super::GuestLanes>>,
    sdk: Option<harmos_runtime::operations::PluginSdkRegistration>,
    initializing: Option<harmos_runtime::operations::EncodedInvocation>,
    shutting_down: Option<harmos_runtime::operations::EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    resident: Option<NativeResident>,
    lease: Option<harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
    registered: bool,
}

#[derive(Default)]
struct ProviderResources {
    endpoint: Option<harmos_runtime::plugin::PluginEndpoint>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
}

struct DependencyHost {
    native_seed: Option<std::sync::Arc<NativeSeed>>,
    guest: Lifecycle,
    operations: harmos_runtime::operations::OperationManager<super::GuestLanes>,
    provider: std::sync::Arc<ProviderOwner>,
}

#[harmos_runtime::catalog]
enum ProviderOperations {
    EchoOperation(ProviderRead),
}

#[harmos_runtime::catalog]
enum ForwardOperations {
    ForwardDeclaration(ForwardDeclaration),
}

#[harmos_domain::catalog]
enum NativeRequirements {
    EchoOperation(EchoOperation),
}

#[harmos_runtime::operation(codec = manual, resources = [ProviderOwner], id = ProviderEchoOperation::ID, response = Echoed,
    policy = ProviderEchoOperation::POLICY, visibility = Shared)]
#[derive(Clone)]
struct ProviderRead(ProviderEchoOperation);

impl harmos_runtime::Message for ProviderRead {
    const CODEC: &'static str = <ProviderEchoOperation as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <ProviderEchoOperation as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <ProviderEchoOperation as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for ProviderRead {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for ProviderRead {
    const NAME: &'static str = <ProviderEchoOperation as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <ProviderEchoOperation as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <ProviderEchoOperation as harmos_runtime::schema::Described>::MARKER;
}

impl harmos_runtime::Operation for ProviderRead {
    type Error = harmos_domain::Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Echoed>, Self::Error> {
        let resources = ctx.resource::<ProviderOwner>();
        resources.prepared.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        resources.answer(&self.0).await.map(Into::into)
    }
}
impl harmos_runtime::Operation for ForwardDeclaration {
    type Error = harmos_domain::Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Echoed>, Self::Error> {
        ctx.resource::<harmos_sidecar::Supervisor>().call::<EchoOperation>(EchoOperation { say: self.say }).await.map_err(Self::Error::of)
    }
}

impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

impl ProviderOwner {
    async fn answer(&self, state: &ProviderEchoOperation) -> Result<Echoed, harmos_domain::Refusal> {
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

impl harmos_operations::lifecycle::Policy for NativeProviderPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = super::sandbox::EchoOperation::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = super::sandbox::EchoOperation::POLICY;
}

impl harmos_runtime::Lifecycle for NativeProviderPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = u64;
    type Error = std::io::Error;

    async fn prepare(id: u64, setup: &mut harmos_sdk::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        let provider = NATIVE_SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|resource| resource.upgrade())
            .ok_or_else(|| std::io::Error::other("the original native fixture resource is unavailable"))?;
        let plugin = Self { resources: provider };
        setup.resource::<ProviderOwner>(plugin.resources.clone());
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "provider", requires = ())]
impl harmos_runtime::Plugin for NativeProviderPlugin {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[ProviderOperations::CATALOG];
}

impl harmos_operations::lifecycle::Policy for NativeClientPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = super::sandbox::EchoOperation::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = super::sandbox::EchoOperation::POLICY;
}

impl harmos_runtime::Lifecycle for NativeClientPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = ();
    type Error = std::io::Error;

    async fn prepare(_: (), setup: &mut harmos_sdk::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        let plugin = Self {
            resources: std::sync::Arc::new(NativeClient),
        };
        setup.resource::<NativeClient>(plugin.resources.clone());
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "client", requires = NativeRequirements)]
impl harmos_runtime::Plugin for NativeClientPlugin {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[ForwardOperations::CATALOG];
}

impl harmos_sidecar::Attach for NativeAttachment {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        &self.id
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::OperationDefinition>> {
        if self.id == "provider" { native_provider().operations() } else { native_client().operations() }
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        if self.id == "provider" { native_provider().operation_contracts() } else { native_client().operation_contracts() }
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        if self.id == "provider" { native_provider().required_contracts() } else { native_client().required_contracts() }
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_operations::lifecycle::Descriptor; 2]>> {
        if self.id == "provider" { native_provider().sdk_lifecycle() } else { native_client().sdk_lifecycle() }
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        if self.id == "provider" { native_provider().attach().await } else { native_client().attach().await }
    }
}

impl DependencyHost {
    async fn new(runtime: &Runtime<Bank>, provider: std::sync::Arc<ProviderOwner>) -> Self {
        Self {
            native_seed: None,
            guest: Lifecycle::sandbox(runtime).await,
            operations: runtime.operations.clone(),
            provider,
        }
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
                resources.registration =
                    Some(self.operations.stage_plugin(scope, |bindings| bindings.operations::<ProviderOperations>(GuestLanes::Open))?);
            }
            harmos_runtime::plugin::PluginBackend::Guest(resources) => {
                resources.start::<GuestOperations>(&self.operations, definition, router, authority, GuestLanes::Open).await?;
            }
            harmos_runtime::plugin::PluginBackend::Sidecar(resources) => {
                let resident: NativeResident = harmos_runtime::sidecar::Supervised::new(
                    NativeAttachment {
                        id: definition.id.clone(),
                        _seed: if definition.id == "provider" { self.native_seed.take() } else { None },
                    },
                    dependency_notice as fn(&harmos_runtime::services::ServiceContext<Bank>, harmos_runtime::sidecar::Notice),
                )
                .routed(router);
                resources.resident = Some(resident.clone());
                let mut ready = resident.ready();
                self.operations.register_service(
                    harmos_runtime::services::ServiceDefinition {
                        id: definition.id.clone(),
                        policy: harmos_runtime::services::POLICY,
                    },
                    move |_: std::sync::Arc<harmos_sdk::ExecutionContext>| resident.clone(),
                )?;
                resources.registered = true;
                resources.lease = Some(self.operations.submit_resident(&definition.id).await?);
                let handle = if definition.contributions.lifecycle.is_some() {
                    let native = resources.resident.as_ref().unwrap().staged_operations().await.map_err(|source| {
                        harmos_runtime::Error::OperationFailed {
                            operation: definition.id.clone(),
                            source: std::sync::Arc::new(source),
                        }
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
                                    + std::mem::size_of::<harmos_operations::lifecycle::Initialize>()
                                    + 64
                                    + 30
                                    + 3072,
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

fn native_provider() -> harmos_sidecar::PluginNative<NativeProviderPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap()).unwrap()
}

fn native_client() -> harmos_sidecar::PluginNative<NativeClientPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap()).unwrap()
}

fn dependency_notice(_: &harmos_runtime::services::ServiceContext<Bank>, _: harmos_runtime::sidecar::Notice) {}

async fn runtime_with_provider(provider: std::sync::Arc<ProviderOwner>) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(super::support::Ledger::default(), (), |setup| {
        setup.resource::<ProviderOwner>(provider);
        setup.settings().register(super::support::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap()
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
            requires: vec![Requirement {
                id: "sandbox_guest".into(),
                version: "*".parse().unwrap(),
            }],
        });
    }
    PluginDefinitions::new(definitions, []).unwrap()
}
