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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Bank, sandbox as wire};
use harmos_runtime::Runtime;
use harmos_runtime::guest::Host;
use harmos_runtime::guest::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginLifecycleAuthority,
    PluginManager, PluginScope, PluginTransition, Router,
};
use harmos_runtime::operations::{Context, EncodedInvocation, PluginRegistration, PluginSdkRegistration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources>;
pub(super) type Manager = PluginManager<Resources, Resources>;

/* === Structs === */

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = wire::Echoed)]
#[derive(Clone)]
struct Echo(wire::ProviderEchoOperation);

static ECHO_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<Echo>(
    wire::PROVIDER_ECHO_OPERATION_OPERATION.entry().id,
    wire::PROVIDER_ECHO_OPERATION_OPERATION.entry().policy,
)
.visibility(harmos_runtime::operations::Visibility::Public);

const ECHO_OPERATION: harmos_runtime::operations::OperationSpec<Echo> =
    harmos_runtime::operations::OperationSpec::<Echo>::new(&ECHO_DESCRIPTION);

impl harmos_runtime::Message for Echo {
    const CODEC: &'static str = <wire::ProviderEchoOperation as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <wire::ProviderEchoOperation as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <wire::ProviderEchoOperation as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Echo {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Echo {
    const NAME: &'static str = <wire::ProviderEchoOperation as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <wire::ProviderEchoOperation as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <wire::ProviderEchoOperation as harmos_runtime::schema::Described>::MARKER;
}

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = ())]
#[derive(Clone)]
struct Publish(harmos_guest::builtins::streams::Publish);

static PUBLISH_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<Publish>(
    harmos_guest::builtins::streams::PUBLISH_DESCRIPTION.operation.id,
    harmos_guest::builtins::streams::PUBLISH_DESCRIPTION.operation.policy,
)
.visibility(harmos_runtime::operations::Visibility::Public);

const PUBLISH_OPERATION: harmos_runtime::operations::OperationSpec<Publish> =
    harmos_runtime::operations::OperationSpec::<Publish>::new(&PUBLISH_DESCRIPTION);

impl harmos_runtime::Message for Publish {
    const CODEC: &'static str = <harmos_guest::builtins::streams::Publish as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <harmos_guest::builtins::streams::Publish as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <harmos_guest::builtins::streams::Publish as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Publish {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Publish {
    const NAME: &'static str = <harmos_guest::builtins::streams::Publish as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] =
        <harmos_guest::builtins::streams::Publish as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_guest::builtins::streams::Publish as harmos_runtime::schema::Described>::MARKER;
}

#[derive(Default)]
pub(super) struct Resources {
    registered: Option<harmos_runtime::guest::Registered<Bank>>,
    serving: Option<harmos_runtime::guest::Serving<Bank>>,
    endpoint: Option<harmos_runtime::guest::PluginEndpoint>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<EncodedInvocation>,
    shutting_down: Option<EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
}

pub(super) struct Lifecycle {
    loader: Host<Bank>,
    operations: harmos_runtime::operations::OperationManager,
    prepared: Option<harmos_runtime::guest::Registered<Bank>>,
    bytes: Vec<u8>,
    lane: harmos_runtime::operations::Lane,
    provider: Option<PluginDefinition>,
    catalog: harmos_runtime::operations::ComponentCatalog,
}

/* === Enums === */

static ECHO_OPERATIONS_CATALOG_DECLARATIONS: [harmos_runtime::catalog::Node<harmos_runtime::operations::WireDeclaration>; 1] =
    [harmos_runtime::catalog::Node::Leaf(ECHO_OPERATION.wire())];
static ECHO_OPERATIONS_CATALOG_CALLEES: [harmos_runtime::catalog::Node<&'static [harmos_runtime::operations::WireDeclaration]>; 1] =
    [harmos_runtime::catalog::Node::Leaf(ECHO_OPERATION.callees())];

const ECHO_OPERATIONS_CATALOG: harmos_runtime::operations::Catalog = harmos_runtime::operations::Catalog::new(
    concat!(module_path!(), "::EchoOperations"),
    &ECHO_OPERATIONS_CATALOG_DECLARATIONS,
    &ECHO_OPERATIONS_CATALOG_CALLEES,
    Some(echo_operations_catalog_batch),
);

fn echo_operations_catalog_batch() -> harmos_runtime::__private::RegistrationBatch {
    let mut batch = harmos_runtime::__private::RegistrationBatch::new();
    batch = batch.entry(echo_operation().expect("representable operation"));
    batch
}

static PUBLICATION_OPERATIONS_CATALOG_DECLARATIONS: [harmos_runtime::catalog::Node<harmos_runtime::operations::WireDeclaration>; 1] =
    [harmos_runtime::catalog::Node::Leaf(PUBLISH_OPERATION.wire())];
static PUBLICATION_OPERATIONS_CATALOG_CALLEES: [harmos_runtime::catalog::Node<&'static [harmos_runtime::operations::WireDeclaration]>; 1] =
    [harmos_runtime::catalog::Node::Leaf(PUBLISH_OPERATION.callees())];

const PUBLICATION_OPERATIONS_CATALOG: harmos_runtime::operations::Catalog = harmos_runtime::operations::Catalog::new(
    concat!(module_path!(), "::PublicationOperations"),
    &PUBLICATION_OPERATIONS_CATALOG_DECLARATIONS,
    &PUBLICATION_OPERATIONS_CATALOG_CALLEES,
    Some(publication_operations_catalog_batch),
);

fn publication_operations_catalog_batch() -> harmos_runtime::__private::RegistrationBatch {
    let mut batch = harmos_runtime::__private::RegistrationBatch::new();
    batch = batch.entry(publish_operation().expect("representable operation"));
    batch
}

pub(super) fn guest_catalog() -> harmos_runtime::operations::ComponentCatalog {
    harmos_runtime::operations::ComponentCatalog::new()
        .operation(wire::ECHO_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::DECLINE_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::SPIN_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::WAIT_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::FAIL_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::READ_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::COUNTS_OPERATION_OPERATION)
        .unwrap()
        .operation(wire::FINISH_OPERATION_OPERATION)
        .unwrap()
}

/* === Traits === */

/* === Implementations === */

async fn echo_handler(
    request: Echo, _: harmos_runtime::operations::FunctionContext<Echo>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<wire::Echoed>> {
    let request = request.0;
    let result: ::core::result::Result<wire::Echoed, harmos_runtime::Error> = async move {
        // The Waiting Probe Asks for an Answer That Never Comes Before Its Deadline
        if request.say == "slow" {
            std::future::pending::<()>().await;
        }
        Ok(wire::Echoed {
            said: format!("provider:{}", request.say),
        })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn echo_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(ECHO_OPERATION, const { &[] }, echo_handler, |_| {
        harmos_runtime::operations::FailureKind::Permanent
    })
}

async fn publish_handler(
    request: Publish, ctx: harmos_runtime::operations::FunctionContext<Publish>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<()>> {
    let mut row = request.0;
    let result: ::core::result::Result<(), harmos_runtime::Error> = async move {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
        let context = host.borrow();
        let scope = guest_scope(&context)?;
        if row.sequence.is_some() || row.published_unix_micros.is_some() || row.context.is_some() || !row.following.is_empty() {
            return Err(harmos_runtime::Error::InvalidInput {
                field: "guest publication",
                reason: "sequence, batch and publication metadata are host-derived",
            });
        }
        let lease = scope.publication(&row.stream)?;
        let row = harmos_runtime::streams::PublicationRow {
            payload: std::mem::take(&mut row.payload),
            published_unix_micros: None,
            context: None,
        };
        lease.stage_derived_row(row, 0).map_err(|source| harmos_runtime::Error::OperationFailed {
            operation: harmos_guest::builtins::streams::PUBLISH_DESCRIPTION.operation.id.into(),
            source: Arc::new(source),
        })?;
        Ok(())
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn publish_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PUBLISH_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        publish_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl PluginLifecycle for Lifecycle {
    type Guest = Resources;

    type Embedded = Resources;
    fn preflight(&mut self, candidate: &PluginDefinitions) -> harmos_runtime::Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, definition: &PluginDefinition) -> harmos_runtime::Result<Backends> {
        match definition.kind() {
            harmos_runtime::guest::PluginKind::Guest => Ok(PluginBackend::Guest(self.guest_seed())),
            harmos_runtime::guest::PluginKind::Embedded => Ok(PluginBackend::Embedded(Resources::default())),
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
                resources.registration =
                    Some(self.operations.stage_plugin(scope, |bindings| bindings.catalog(ECHO_OPERATIONS_CATALOG, self.lane))?);
            }
            PluginBackend::Guest(resources) => {
                resources.start(&self.catalog, &self.operations, definition, router, authority, self.lane).await?;
            }
        }
        Ok(())
    }
    async fn stop(_: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> harmos_runtime::Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources) | PluginBackend::Embedded(resources) => resources,
        };
        resources.stop(authority).await
    }
}

impl Resources {
    pub(super) async fn start(
        &mut self, catalog: &harmos_runtime::operations::ComponentCatalog, operations: &harmos_runtime::operations::OperationManager,
        definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>, lane: harmos_runtime::operations::Lane,
    ) -> harmos_runtime::Result<Arc<harmos_runtime::operations::Component>> {
        let registered = self.registered.take().unwrap().routed(router).map_err(guest_error)?;
        self.serving = Some(registered.serve_sdk().await.map_err(guest_error)?);
        let serving = self.serving.as_ref().unwrap();
        let component = Arc::new(serving.operations().map_err(guest_error)?);
        self.registration =
            Some(operations.stage_plugin(serving.scope().unwrap(), |bindings| {
                bindings.register_component_catalog(catalog, lane, component.clone())
            })?);
        self.sdk = Some(component.stage_sdk(operations, lane)?);
        self.initializing = Some(
            self.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                NonZeroUsize::new(
                    2 * definition.configuration.len() + std::mem::size_of::<harmos_guest::lifecycle::Initialize>() + 64 + 30 + 3072,
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
                            // Report the Terminal Refusal Once, Retaining Original Cleanup Owners
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
    pub(super) async fn sandbox(runtime: &Runtime<Bank>) -> Self {
        Self::sandbox_in(runtime, harmos_runtime::operations::Lane::default()).await
    }
}

impl Lifecycle {
    pub(super) fn schematic(runtime: &Runtime<Bank>) -> Self {
        Self::for_component(schematic_catalog(), runtime, super::schematic(), harmos_runtime::operations::Lane::default())
    }
}

impl Lifecycle {
    // The Sandbox Requires Host Publication and an Embedded Provider in Its Own Lane
    pub(super) async fn sandbox_in(runtime: &Runtime<Bank>, lane: harmos_runtime::operations::Lane) -> Self {
        runtime.operations.register_catalog(PUBLICATION_OPERATIONS_CATALOG, lane).await.unwrap();
        let mut lifecycle = Self::for_component(guest_catalog(), runtime, super::sandbox(), lane);
        let contract = wire::PROVIDER_ECHO_OPERATION_OPERATION.wire().contract().unwrap();
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

impl Lifecycle {
    pub(super) fn for_component(
        catalog: harmos_runtime::operations::ComponentCatalog, runtime: &Runtime<Bank>, bytes: Vec<u8>,
        lane: harmos_runtime::operations::Lane,
    ) -> Self {
        Self {
            loader: Host::new(runtime).unwrap(),
            operations: runtime.operations.clone(),
            prepared: None,
            bytes,
            lane,
            provider: None,
            catalog,
        }
    }
    pub(super) fn guest_seed(&mut self) -> Resources {
        Resources {
            registered: Some(self.prepared.take().expect("the original collected store is retained before preparation")),
            ..Resources::default()
        }
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

// Serves the Sandbox's Requirements from the Host When No Plugin Provides Them
pub(super) async fn serve_host(runtime: &Runtime<Bank>, lane: harmos_runtime::operations::Lane) {
    runtime.operations.register_catalog(PUBLICATION_OPERATIONS_CATALOG, lane).await.unwrap();
    runtime.operations.register_catalog(ECHO_OPERATIONS_CATALOG, lane).await.unwrap();
}

fn guest_error(source: harmos_runtime::guest::Error) -> harmos_runtime::Error {
    harmos_runtime::Error::OperationFailed {
        operation: "sandbox_guest".into(),
        source: Arc::new(source),
    }
}

fn guest_scope<'a>(context: &'a Context<'_>) -> harmos_runtime::Result<&'a PluginScope> {
    context.caller_scope().ok_or(harmos_runtime::Error::InvalidInput {
        field: "guest caller",
        reason: "the original guest scope is required",
    })
}

/* === Tests === */

pub(super) fn schematic_catalog() -> harmos_runtime::operations::ComponentCatalog {
    harmos_runtime::operations::ComponentCatalog::new()
        .operation(schematic_operations::guest::VALIDATE_SCHEMATIC_OPERATION)
        .unwrap()
        .operation(schematic_operations::guest::PROPOSE_ROUTE_OPERATION)
        .unwrap()
        .operation(schematic_operations::guest::ANALYZE_SCHEMATIC_OPERATION)
        .unwrap()
}
