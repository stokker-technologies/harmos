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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::lifecycle::{Lifecycle, Resources, guest_catalog};
use super::sandbox::{ECHO_OPERATION_OPERATION, EchoOperation, Echoed, PROVIDER_ECHO_OPERATION_OPERATION, ProviderEchoOperation};
use super::{Bank, component_error, retire_failed_guest};
use harmos_runtime::Runtime;
use harmos_runtime::operations::OperationHost as _;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type GuestResources = Resources;
type DependencyBackends = harmos_runtime::guest::PluginBackend<GuestResources, ProviderResources>;

/* === Structs === */

#[derive(Default)]
struct ProviderOwner {
    prepared: std::sync::atomic::AtomicUsize,
    started: tokio::sync::Notify,
    cancelled: Arc<tokio::sync::Notify>,
    observed: std::sync::Mutex<Vec<String>>,
}

struct Cancelled(Arc<tokio::sync::Notify>);

#[harmos_runtime::message(harmos_path = harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Echoed)]
#[derive(Clone)]
struct ForwardDeclaration {
    say: String,
}

static FORWARD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ForwardDeclaration>("client.forward", ECHO_OPERATION_OPERATION.entry().policy)
        .visibility(harmos_runtime::operations::Visibility::Public);

static FORWARD_DECLARATION_CALLEES: [harmos_runtime::operations::WireDeclaration; 1] = [ECHO_OPERATION_OPERATION.wire()];
const FORWARD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ForwardDeclaration> =
    harmos_runtime::operations::OperationSpec::<ForwardDeclaration>::new(&FORWARD_DECLARATION_DESCRIPTION)
        .calls(&FORWARD_DECLARATION_CALLEES);

struct NativeClient;

#[derive(Default)]
struct ProviderResources {
    endpoint: Option<harmos_runtime::guest::PluginEndpoint>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
}

struct DependencyHost {
    guest: Lifecycle,
    operations: harmos_runtime::operations::OperationManager,
}

/* === Enums === */

fn provider_operations_catalog() -> harmos_runtime::operations::Catalog {
    provider_operations_catalog_batch().named(concat!(module_path!(), "::ProviderOperations"))
}

fn provider_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(provider_read_operation().expect("representable operation"));
    batch
}

fn forward_operations_catalog() -> harmos_runtime::operations::Catalog {
    forward_operations_catalog_batch().named(concat!(module_path!(), "::ForwardOperations"))
}

fn forward_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(forward_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Echoed)]
#[derive(Clone)]
struct ProviderRead(ProviderEchoOperation);

static PROVIDER_READ_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ProviderRead>(
        PROVIDER_ECHO_OPERATION_OPERATION.entry().id,
        PROVIDER_ECHO_OPERATION_OPERATION.entry().policy,
    )
    .visibility(harmos_runtime::operations::Visibility::Public);

const PROVIDER_READ_OPERATION: harmos_runtime::operations::OperationSpec<ProviderRead> =
    harmos_runtime::operations::OperationSpec::<ProviderRead>::new(&PROVIDER_READ_DESCRIPTION);

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

async fn provider_read_handler(
    request: ProviderRead, ctx: harmos_runtime::operations::FunctionContext<ProviderRead>,
) -> Result<harmos_runtime::Response<Echoed>, harmos_domain::Refusal> {
    let resources = ctx.resource::<ProviderOwner>().expect("declared resource is registered");
    resources.prepared.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    resources.answer(&request.0).await.map(Into::into)
}

fn provider_read_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PROVIDER_READ_OPERATION,
        const { &[harmos_runtime::Resource::of::<ProviderOwner>()] },
        provider_read_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
async fn forward_declaration_handler(
    request: ForwardDeclaration, ctx: harmos_runtime::operations::FunctionContext<ForwardDeclaration>,
) -> Result<harmos_runtime::Response<Echoed>, harmos_domain::Refusal> {
    ctx.operation(ECHO_OPERATION_OPERATION, EchoOperation { say: request.say }).await.map_err(harmos_domain::Refusal::from)
}

fn forward_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        FORWARD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<NativeClient>()] },
        forward_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
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

impl DependencyHost {
    async fn new(runtime: &Runtime<Bank>) -> Self {
        Self {
            guest: Lifecycle::sandbox(runtime).await,
            operations: runtime.operations.clone(),
        }
    }
}

impl harmos_runtime::guest::PluginLifecycle for DependencyHost {
    type Guest = GuestResources;

    type Embedded = ProviderResources;
    fn preflight(&mut self, candidate: &harmos_runtime::guest::PluginDefinitions) -> harmos_runtime::Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, definition: &harmos_runtime::guest::PluginDefinition) -> harmos_runtime::Result<DependencyBackends> {
        Ok(match definition.kind() {
            harmos_runtime::guest::PluginKind::Guest => harmos_runtime::guest::PluginBackend::Guest(self.guest.guest_seed()),
            harmos_runtime::guest::PluginKind::Embedded => harmos_runtime::guest::PluginBackend::Embedded(ProviderResources::default()),
        })
    }
    async fn fence(
        &mut self, _: harmos_runtime::guest::PluginTransition, _: &[harmos_runtime::guest::PluginDefinition],
    ) -> harmos_runtime::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &harmos_runtime::guest::PluginDefinition, router: &harmos_runtime::guest::Router,
        authority: &harmos_runtime::guest::PluginLifecycleAuthority<'_>, backend: &mut DependencyBackends,
    ) -> harmos_runtime::Result<()> {
        match backend {
            harmos_runtime::guest::PluginBackend::Embedded(resources) => {
                resources.endpoint = Some(router.enter(&definition.id).unwrap());
                let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
                resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                    if definition.id == "provider" {
                        bindings.catalog(provider_operations_catalog(), harmos_runtime::operations::Lane::default())
                    } else {
                        bindings.catalog(forward_operations_catalog(), harmos_runtime::operations::Lane::default())
                    }
                })?);
            }
            harmos_runtime::guest::PluginBackend::Guest(resources) => {
                resources
                    .start(&guest_catalog(), &self.operations, definition, router, authority, harmos_runtime::operations::Lane::default())
                    .await?;
            }
        }
        Ok(())
    }
    async fn stop(
        _: &harmos_runtime::guest::PluginDefinition, authority: &harmos_runtime::guest::PluginLifecycleAuthority<'_>,
        backend: &mut DependencyBackends,
    ) -> harmos_runtime::Result<()> {
        match backend {
            harmos_runtime::guest::PluginBackend::Guest(resources) => resources.stop(authority).await?,
            harmos_runtime::guest::PluginBackend::Embedded(resources) => {
                if let Some(registration) = &mut resources.registration {
                    registration.shutdown().await?;
                }
                resources.registration.take();
                resources.endpoint.take();
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime_with_provider(provider: Arc<ProviderOwner>) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(super::support::Ledger::default(), (), |setup| {
        setup.resource::<ProviderOwner>(provider);
        setup.resource::<NativeClient>(Arc::new(NativeClient));
        setup.settings().register(super::support::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap()
}

async fn dependency_candidate(lifecycle: &mut DependencyHost, configuration: &str) -> harmos_runtime::guest::PluginDefinitions {
    lifecycle.guest.candidate(configuration).await
}

async fn dependency_candidate_with_client(lifecycle: &mut DependencyHost, configuration: &str) -> harmos_runtime::guest::PluginDefinitions {
    use harmos_runtime::guest::{PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitions, Requirement};
    let mut definitions: Vec<_> = dependency_candidate(lifecycle, configuration).await.definitions().cloned().collect();
    let provided = FORWARD_DECLARATION_OPERATION.wire().contract().unwrap();
    let required = super::sandbox::ECHO_OPERATION_OPERATION.wire().contract().unwrap();
    definitions.push(PluginDefinition {
        id: "client".into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded { revision: "fixture".into() },
        configuration: b"null".to_vec(),
        contributions: PluginContributions {
            operations: vec![provided.operation().clone()],
            wire_contracts: vec![provided],
            required_contracts: Some(vec![harmos_domain::inventory::contracts::Required { contract: required, grants: vec![] }]),
            ..Default::default()
        },
        requires: vec![Requirement {
            id: "sandbox_guest".into(),
            version: "*".parse().unwrap(),
        }],
    });
    PluginDefinitions::new(definitions, []).unwrap()
}

/* === Tests === */
