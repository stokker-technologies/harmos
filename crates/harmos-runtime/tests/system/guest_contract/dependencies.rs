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
use super::lifecycle::{GuestOperations, Lifecycle, Resources};
use super::sandbox::{EchoOperation, Echoed, ProviderEchoOperation};
use super::{Bank, GuestLanes, component_error, retire_failed_guest};
use harmos_runtime::Runtime;
use harmos_runtime::operations::{OperationDeclaration, OperationHost as _};

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

#[harmos_runtime::operation(resources = [NativeClient], operations = [EchoOperation], id = "client.forward",
    response = Echoed, policy = super::sandbox::EchoOperation::POLICY, visibility = Public)]
#[derive(Clone)]
struct ForwardDeclaration {
    say: String,
}

struct NativeClient;

#[derive(Default)]
struct ProviderResources {
    endpoint: Option<harmos_runtime::guest::PluginEndpoint>,
    registration: Option<harmos_runtime::operations::PluginRegistration>,
}

struct DependencyHost {
    guest: Lifecycle,
    operations: harmos_runtime::operations::OperationManager<GuestLanes>,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum ProviderOperations {
    EchoOperation(ProviderRead),
}

#[harmos_runtime::catalog]
enum ForwardOperations {
    ForwardDeclaration(ForwardDeclaration),
}

/* === Traits === */

/* === Implementations === */

#[harmos_runtime::operation(codec = manual, resources = [ProviderOwner], id = ProviderEchoOperation::ID, response = Echoed,
    policy = ProviderEchoOperation::POLICY, visibility = Public)]
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
        ctx.operation(EchoOperation { say: self.say }).await.map_err(Self::Error::from)
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
                        bindings.operations::<ProviderOperations>(GuestLanes::Open)
                    } else {
                        bindings.operations::<ForwardOperations>(GuestLanes::Open)
                    }
                })?);
            }
            harmos_runtime::guest::PluginBackend::Guest(resources) => {
                resources.start::<GuestOperations>(&self.operations, definition, router, authority, GuestLanes::Open).await?;
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
    let provided = harmos_runtime::operations::WireDeclaration::of::<ForwardDeclaration>().contract().unwrap();
    let required = harmos_runtime::operations::WireDeclaration::of::<EchoOperation>().contract().unwrap();
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
