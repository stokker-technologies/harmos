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
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Bank, sandbox as wire};
use harmos_runtime::Runtime;
use harmos_runtime::guest::Host;
use harmos_runtime::operations::{
    Context, EncodedInvocation, Operation, OperationDeclaration, PluginRegistration, PluginSdkRegistration, WireDeclaration,
};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginLifecycleAuthority,
    PluginManager, PluginScope, PluginTransition, Router,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, (), Resources>;
pub(super) type Manager = PluginManager<Resources, (), Resources>;

/* === Structs === */

#[derive(Default)]
struct Domain;

struct Echo {
    _resources: Arc<Domain>,
}

struct Publish {
    _resources: Arc<Domain>,
}

#[derive(Default)]
pub(super) struct Resources {
    registered: Option<harmos_runtime::guest::Registered<Bank>>,
    serving: Option<harmos_runtime::guest::Serving<Bank>>,
    endpoint: Option<harmos_runtime::plugin::PluginEndpoint>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
}

pub(super) struct Lifecycle<C = GuestOperations, L = super::GuestLanes> {
    loader: Host<Bank>,
    operations: harmos_runtime::operations::OperationManager,
    prepared: Option<harmos_runtime::guest::Registered<Bank>>,
    bytes: Vec<u8>,
    lane: L,
    provider: Option<PluginDefinition>,
    component: Option<Arc<harmos_runtime::operations::Component>>,
    catalog: PhantomData<fn() -> C>,
}

/* === Enums === */

#[harmos_runtime::catalog]
pub(super) enum GuestOperations {
    Echo(wire::Echo),
    Decline(wire::Decline),
    Spin(wire::Spin),
    Wait(wire::Wait),
    Fail(wire::Fail),
    Read(wire::Read),
    Counts(wire::Counts),
}

#[harmos_runtime::catalog]
enum ProviderOperations {
    Echo(Echo),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Echo {
    type Declaration = wire::ProviderEcho;
    type Resources = Domain;
    type Error = harmos_runtime::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(resources: Arc<Domain>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut wire::ProviderEcho, _endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<wire::Echoed>> {
        let result: ::core::result::Result<wire::Echoed, Self::Error> = async move {
            // The Waiting Probe Asks For An Answer That Never Comes Before Its Deadline
            if request.say == "slow" {
                std::future::pending::<()>().await;
            }
            Ok(wire::Echoed { said: format!("provider:{}", request.say) })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Publish {
    type Declaration = harmos_runtime::operations::streams::Publish;
    type Resources = Domain;
    type Error = harmos_runtime::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(resources: Arc<Domain>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, row: &mut harmos_runtime::operations::streams::Publish, _endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<()>> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            let scope = guest_scope(&context)?;
            if row.sequence.is_some() || row.published_unix_micros.is_some() || row.context.is_some() || !row.following.is_empty() {
                return Err(harmos_runtime::Error::InvalidInput {
                    field: "guest publication",
                    reason: "sequence, batch and publication metadata are host-derived",
                });
            }
            let lease = scope.publication(&row.stream, row.schema)?;
            let row = harmos_runtime::streams::PublicationRow {
                payload: std::mem::take(&mut row.payload),
                published_unix_micros: None,
                context: None,
            };
            lease.stage_derived_row(row, 0).map_err(|source| harmos_runtime::Error::OperationFailed {
                operation: <harmos_runtime::operations::streams::Publish as OperationDeclaration>::ID.into(),
                source: Arc::new(source),
            })?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl<C, L> PluginLifecycle for Lifecycle<C, L>
where
    C: harmos_runtime::catalog::Catalog<harmos_runtime::operations::Declarations>
        + harmos_runtime::catalog::Catalog<harmos_runtime::operations::ComponentBindings>,
    L: harmos_runtime::operations::Lane + Copy,
{
    type Guest = Resources;
    type Sidecar = ();
    type Embedded = Resources;
    fn preflight(&mut self, candidate: &PluginDefinitions) -> harmos_runtime::Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, definition: &PluginDefinition) -> harmos_runtime::Result<Backends> {
        match definition.kind() {
            harmos_runtime::plugin::PluginKind::Guest => Ok(PluginBackend::Guest(self.guest_seed())),
            harmos_runtime::plugin::PluginKind::Embedded => Ok(PluginBackend::Embedded(Resources::default())),
            harmos_runtime::plugin::PluginKind::Sidecar => {
                unreachable!("the fixture owns no native transport")
            }
        }
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> harmos_runtime::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> harmos_runtime::Result<()> {
        match backend {
            PluginBackend::Embedded(resources) => {
                resources.endpoint = Some(router.enter(&definition.id).unwrap());
                let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
                resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                    bindings.register_wire_operations::<ProviderOperations, _>(self.lane, Arc::new(Domain))
                })?);
            }
            PluginBackend::Guest(resources) => {
                self.component = Some(resources.start::<C, L>(&self.operations, definition, router, authority, self.lane).await?);
            }
            PluginBackend::Sidecar(()) => unreachable!(),
        }
        Ok(())
    }
    async fn stop(_: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> harmos_runtime::Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources) | PluginBackend::Embedded(resources) => resources,
            PluginBackend::Sidecar(()) => unreachable!(),
        };
        resources.stop(authority).await
    }
}

impl Resources {
    pub(super) async fn start<C, L>(
        &mut self, operations: &harmos_runtime::operations::OperationManager, definition: &PluginDefinition, router: &Router,
        authority: &PluginLifecycleAuthority<'_>, lane: L,
    ) -> harmos_runtime::Result<Arc<harmos_runtime::operations::Component>>
    where
        C: harmos_runtime::catalog::Catalog<harmos_runtime::operations::Declarations>
            + harmos_runtime::catalog::Catalog<harmos_runtime::operations::ComponentBindings>,
        L: harmos_runtime::operations::Lane + Copy,
    {
        let registered = self.registered.take().unwrap().routed(router).map_err(guest_error)?;
        self.serving = Some(registered.serve_sdk().await.map_err(guest_error)?);
        let serving = self.serving.as_ref().unwrap();
        let component = Arc::new(serving.operations().map_err(guest_error)?);
        self.registration = Some(
            operations
                .stage_plugin(serving.scope().unwrap(), |bindings| bindings.register_component_catalog::<C>(lane, component.clone()))?,
        );
        self.sdk = Some(component.stage_sdk(operations, lane)?);
        self.initializing = Some(
            self.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                NonZeroUsize::new(
                    2 * definition.configuration.len() + std::mem::size_of::<harmos_runtime::operations::lifecycle::Initialize>() + 64 + 20,
                )
                .unwrap(),
            )?,
        );
        let result = self.initializing.as_mut().unwrap().await;
        self.initializing.take();
        result?;
        self.initialized = true;
        Ok(component)
    }
    pub(super) async fn stop(&mut self, authority: &PluginLifecycleAuthority<'_>) -> harmos_runtime::Result<()> {
        if let Some(sdk) = &self.sdk {
            sdk.settle().await?;
            if let Some(initializing) = &mut self.initializing {
                let result = initializing.await;
                self.initializing.take();
                result?;
                self.initialized = true;
            }
            if self.initialized && !self.shutdown_attempted {
                match sdk.submit_shutdown(authority) {
                    Ok(shutdown) => {
                        self.shutting_down = Some(shutdown);
                        self.shutdown_attempted = true;
                    }
                    Err(error) => {
                        if self.serving.as_ref().is_some_and(|serving| serving.operations().is_err()) {
                            // Report The Terminal Refusal Once, Retaining Original Cleanup Owners
                            self.shutdown_attempted = true;
                            self.initialized = false;
                        }
                        return Err(error);
                    }
                }
            }
            if let Some(shutdown) = &mut self.shutting_down {
                let result = shutdown.await;
                self.shutting_down.take();
                result?;
                self.initialized = false;
            }
        }
        if let Some(registration) = &mut self.registration {
            registration.shutdown().await?;
        }
        if let Some(serving) = &mut self.serving {
            serving.finalize(Duration::from_secs(2)).await?;
        }
        if let Some(sdk) = &mut self.sdk {
            sdk.close().await?;
        }
        self.sdk.take();
        self.registration.take();
        self.serving.take();
        self.endpoint.take();
        self.registered.take();
        Ok(())
    }
}

impl Lifecycle {
    pub(super) fn sandbox(runtime: &Runtime<Bank>) -> Self {
        Self::sandbox_in(runtime, super::GuestLanes::Open)
    }
}

impl Lifecycle<super::schematic::operations::Operations> {
    pub(super) fn schematic(runtime: &Runtime<Bank>) -> Self {
        Self::for_component(runtime, super::schematic(), super::GuestLanes::Open)
    }
}

impl<L: harmos_runtime::operations::Lane + Copy> Lifecycle<GuestOperations, L> {
    // The Sandbox Requires Host Publication And An Embedded Provider In Its Own Lane
    pub(super) fn sandbox_in(runtime: &Runtime<Bank>, lane: L) -> Self {
        runtime.operations.register_wire_operation::<Publish>("publication-core", lane, Arc::new(Domain)).unwrap();
        let mut lifecycle = Self::for_component(runtime, super::sandbox(), lane);
        let contract = WireDeclaration::of::<wire::ProviderEcho>().contract().unwrap();
        lifecycle.provider = Some(PluginDefinition {
            id: "provider".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::Embedded { revision: "fixture".into() },
            requires: vec![],
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                operations: vec![contract.operation().clone()],
                wire_contracts: vec![contract],
                required_contracts: Some(vec![]),
                ..PluginContributions::default()
            },
        });
        lifecycle
    }
}

impl<C, L> Lifecycle<C, L> {
    pub(super) fn for_component(runtime: &Runtime<Bank>, bytes: Vec<u8>, lane: L) -> Self {
        Self {
            loader: Host::new(runtime).unwrap(),
            operations: runtime.operations.clone(),
            prepared: None,
            bytes,
            lane,
            provider: None,
            component: None,
            catalog: PhantomData,
        }
    }
    pub(super) fn guest_seed(&mut self) -> Resources {
        Resources {
            registered: Some(self.prepared.take().expect("the original collected store is retained before preparation")),
            ..Resources::default()
        }
    }
    pub(super) fn component(&self) -> Arc<harmos_runtime::operations::Component> {
        self.component.as_ref().expect("the original component was staged").clone()
    }
    pub(super) async fn candidate(&mut self, mode: impl serde::Serialize) -> PluginDefinitions {
        assert!(self.prepared.is_none());
        let loaded = self.loader.load(&self.bytes).unwrap();
        let declaration = loaded.declaration().clone();
        let sha256 = loaded.sha256();
        let registered = loaded.register().await.unwrap();
        let manifest = declaration.contracts;
        let guest = PluginDefinition {
            id: declaration.id,
            version: declaration.version,
            artifact: PluginArtifact::Guest { sha256 },
            requires: vec![],
            configuration: serde_json::to_vec(&mode).unwrap(),
            contributions: PluginContributions {
                lifecycle: Some(manifest.lifecycle),
                operations: declaration.operations,
                wire_contracts: manifest.provides,
                required_contracts: Some(manifest.requires),
                streams: declaration.streams,
                listeners: declaration.listeners,
                registrations: registered.registrations().to_vec(),
                ..PluginContributions::default()
            },
        };
        self.prepared = Some(registered);
        let definitions = self.provider.clone().into_iter().chain([guest]);
        PluginDefinitions::new(definitions, []).unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */

// Serves The Sandbox's Requirements From The Host When No Plugin Provides Them
pub(super) fn serve_host(runtime: &Runtime<Bank>, lane: impl harmos_runtime::operations::Lane + Clone) {
    runtime.operations.register_wire_operation::<Publish>("publication-core", lane.clone(), Arc::new(Domain)).unwrap();
    runtime.operations.register_wire_operation::<Echo>("provider-core", lane, Arc::new(Domain)).unwrap();
}

fn guest_error(source: harmos_runtime::guest::Error) -> harmos_runtime::Error {
    harmos_runtime::Error::OperationFailed { operation: "sandbox-guest".into(), source: Arc::new(source) }
}

fn guest_scope<'a>(context: &'a Context<'_>) -> harmos_runtime::Result<&'a PluginScope> {
    context
        .caller_scope()
        .ok_or(harmos_runtime::Error::InvalidInput { field: "guest caller", reason: "the original guest scope is required" })
}

/* === Tests === */
