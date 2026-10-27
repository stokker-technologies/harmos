/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../plugin/activation.rs"]
mod activation;
#[path = "../plugin/reload.rs"]
mod reload;
#[path = "../plugin/retirement.rs"]
mod retirement;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

use harmos_runtime::catalog::{Catalog, Member};
use harmos_runtime::operations::{
    Bindings, Handler, OperationDeclaration, OperationDefinition as Definition, OperationManager, OperationPolicy, PluginRegistration,
    RetryPolicy, Visibility,
};
use harmos_runtime::plugin::*;
use harmos_runtime::plugin::{PluginEndpoint, PluginKind, Requirement};
use harmos_runtime::{Error, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Activation {
    endpoint: Option<PluginEndpoint>,
    registration: Option<PluginRegistration>,
    dependencies_stopped: bool,
    stopping: Option<StopContext>,
}

type Backends = PluginBackend<Activation, Activation, Activation>;
type Manager = PluginManager<Activation, Activation, Activation>;

struct StopContext {
    owner: String,
    entered: Arc<tokio::sync::Notify>,
    operations: OperationManager<Lanes>,
    control: Arc<Mutex<Control>>,
    release: Arc<tokio::sync::Notify>,
}

struct StopWait {
    owner: String,
    control: Arc<Mutex<Control>>,
}

impl Drop for StopWait {
    fn drop(&mut self) {
        let mut control = self.control.lock().unwrap();
        *control.future_drops.entry(self.owner.clone()).or_default() += 1;
    }
}

impl Drop for StopContext {
    fn drop(&mut self) {
        let mut control = self.control.lock().unwrap();
        *control.backend_drops.entry(self.owner.clone()).or_default() += 1;
        drop(control);
        self.entered.notify_one();
    }
}

#[derive(Default)]
struct Control {
    stop_calls: BTreeMap<String, usize>,
    future_drops: BTreeMap<String, usize>,
    backend_drops: BTreeMap<String, usize>,
    panic_stop: Option<String>,
    events: Vec<String>,
    callers: BTreeMap<String, PluginCaller>,
    fail_stop: Option<String>,
    block_stop: Option<String>,
    skip_dependency_calls: bool,
}

#[derive(Default)]
struct Host {
    runtime: Option<harmos_runtime::Runtime<Fixture>>,
    entered: Arc<tokio::sync::Notify>,
    control: Arc<Mutex<Control>>,
    release: Arc<tokio::sync::Notify>,
    wrong_backend: Option<String>,
    fail_start: Option<String>,
    block: Option<String>,
    started: Option<tokio::sync::watch::Sender<String>>,
    block_fence: bool,
    fail_fence: bool,
    transitions: Vec<PluginTransition>,
}

struct Fixture;
struct Owner;
struct Shutdown;

impl PluginLifecycle for Shutdown {
    type Guest = Activation;
    type Sidecar = Activation;
    type Embedded = Activation;

    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        panic!("shutdown cannot prepare an activation")
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(&mut self, _: &PluginDefinition, _: &Router, _: &PluginLifecycleAuthority<'_>, _: &mut Backends) -> Result<()> {
        panic!("shutdown cannot start an activation")
    }
    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, _: &mut Backends) -> Result<()> {
        panic!("shutdown must retain the original activation adapter")
    }
}
#[harmos_runtime::message(harmos_path = harmos_runtime)]
struct RunRequest {
    value: String,
}

// One Run Declaration Per Owner, Timeout And Visibility; Each Encodes As A RunRequest.
struct Run<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> {
    value: String,
}
struct Running<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> {
    _resources: Arc<Owner>,
}
struct Operations<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool>;

#[harmos_runtime::lanes(harmos_path = harmos_runtime)]
enum Lanes {
    #[default]
    Open,
}

impl harmos_runtime::Application for Fixture {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

const RUN_NAME: &str = concat!(module_path!(), "::Run");
const RUN_MARKER: &[u8] = &harmos_runtime::schema::declaration::encode::<
    { harmos_runtime::schema::declaration::encoded_len(RUN_NAME, <RunRequest as harmos_runtime::schema::Described>::FIELDS) },
>(RUN_NAME, <RunRequest as harmos_runtime::schema::Described>::FIELDS);

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> Run<OWNER, MILLIS, PRIVATE> {
    fn wire(&self) -> RunRequest {
        RunRequest { value: self.value.clone() }
    }
}

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> harmos_runtime::Message for Run<OWNER, MILLIS, PRIVATE> {
    const CODEC: &'static str = <RunRequest as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.wire())
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.wire())
    }
    fn decoding_memory(bytes: &[u8]) -> ::core::result::Result<usize, harmos_runtime::MessageError> {
        <RunRequest as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> ::core::result::Result<Self, harmos_runtime::MessageError> {
        <RunRequest as harmos_runtime::Message>::decode(bytes).map(|request| Self { value: request.value })
    }
}

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> harmos_runtime::MemorySize for Run<OWNER, MILLIS, PRIVATE> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.value.capacity()
    }
}

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> harmos_runtime::schema::Described for Run<OWNER, MILLIS, PRIVATE> {
    const NAME: &'static str = RUN_NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <RunRequest as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = RUN_MARKER;
}

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> OperationDeclaration for Run<OWNER, MILLIS, PRIVATE> {
    type Request = Self;
    type Response = String;
    type Input = harmos_runtime::operations::NoStream;
    type Output = harmos_runtime::operations::NoStream;
    const ID: &'static str = "run";
    const VISIBILITY: Visibility = if PRIVATE { Visibility::Private } else { Visibility::Shared };
    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_millis(MILLIS),
        attempt_timeout: Duration::from_millis(MILLIS),
        retries: RetryPolicy::Never,
    };
}

impl<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> Handler for Running<OWNER, MILLIS, PRIVATE> {
    type Request = Run<OWNER, MILLIS, PRIVATE>;
    type Response = String;
    type Error = std::io::Error;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::ExecutionContext) -> std::io::Result<Self> {
        Ok(Self {
            _resources: context.resource::<Owner>().expect("fixture owner"),
        })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, request: &mut Self::Request, _: &mut (),
    ) -> std::io::Result<harmos_runtime::Response<String>> {
        Ok(request.value.clone().into())
    }
}

impl<V, const OWNER: usize, const MILLIS: u64, const PRIVATE: bool> Catalog<V> for Operations<OWNER, MILLIS, PRIVATE>
where
    V: Member<Run<OWNER, MILLIS, PRIVATE>>,
{
    type Entries = [V::Entry; 1];
    const ENTRIES: Self::Entries = [V::ENTRY];
}

fn registration<const OWNER: usize, const MILLIS: u64, const PRIVATE: bool>() -> harmos_runtime::operations::Registration<Lanes> {
    use harmos_runtime::RegistrationExt;
    let mut dependencies = harmos_runtime::Dependencies::new();
    dependencies.insert(Arc::new(Owner)).unwrap();
    harmos_runtime::RegistrationBatch::new(dependencies)
        .implementation::<Run<OWNER, MILLIS, PRIVATE>, Running<OWNER, MILLIS, PRIVATE>>()
        .lane(Lanes::Open)
}

fn request_payload() -> Vec<u8> {
    harmos_runtime::Message::encode(&RunRequest { value: String::new() })
}

fn bind<const OWNER: usize>(bindings: &mut Bindings<'_>, definition: &Definition) -> Result<()> {
    match (definition.policy.timeout.as_millis(), definition.visibility) {
        (100, Visibility::Shared) => bindings.register(registration::<OWNER, 100, false>()),
        (100, Visibility::Private) => bindings.register(registration::<OWNER, 100, true>()),
        (200, Visibility::Shared) => bindings.register(registration::<OWNER, 200, false>()),
        (200, Visibility::Private) => bindings.register(registration::<OWNER, 200, true>()),
        _ => panic!("fixture has no authored handler for this operation policy"),
    }
}

impl Host {
    async fn new() -> Self {
        let runtime = harmos_runtime::Runtime::builder((), ())
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .register(Lanes::catalog())
            .runtime()
            .await
            .unwrap();
        Self {
            runtime: Some(runtime),
            ..Self::default()
        }
    }
    fn operations(&self) -> &OperationManager<Lanes> {
        &self.runtime.as_ref().unwrap().operations
    }
    async fn call(&self, source: &PluginCaller, method: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.operations().caller(source)?.call_encoded(method, payload).await
    }
}

impl PluginLifecycle for Host {
    type Guest = Activation;
    type Sidecar = Activation;
    type Embedded = Activation;

    fn prepare(&mut self, definition: &PluginDefinition) -> Result<Backends> {
        let activation = Activation {
            stopping: Some(StopContext {
                owner: definition.id.clone(),
                entered: self.entered.clone(),
                operations: self.operations().clone(),
                control: self.control.clone(),
                release: self.release.clone(),
            }),
            ..Activation::default()
        };
        if self.wrong_backend.as_ref() == Some(&definition.id) {
            return Ok(PluginBackend::Guest(activation));
        }
        Ok(match definition.kind() {
            PluginKind::Guest => PluginBackend::Guest(activation),
            PluginKind::Sidecar => PluginBackend::Sidecar(activation),
            PluginKind::Embedded => PluginBackend::Embedded(activation),
        })
    }

    async fn fence(&mut self, transition: PluginTransition, affected: &[PluginDefinition]) -> Result<()> {
        self.transitions.push(transition);
        self.control
            .lock()
            .unwrap()
            .events
            .push(format!("fence:{}", affected.iter().map(|artifact| artifact.id.as_str()).collect::<Vec<_>>().join(",")));
        if self.block_fence {
            std::future::pending::<()>().await;
        }
        if self.fail_fence {
            return Err(Error::ResourceExhausted { resource: "plugin execution drain" });
        }
        Ok(())
    }
    async fn start(
        &mut self, artifact: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        self.control.lock().unwrap().events.push(format!("start:{}", artifact.id));
        let owned = activation(backend);
        owned.endpoint = Some(router.enter(&artifact.id).unwrap());
        let instance = owned.endpoint.as_ref().unwrap();
        let scope = instance.scope().unwrap();
        assert!(authority.matches_scope(scope, artifact.kind(), PluginLifecyclePhase::Initialize));
        assert!(!authority.matches_scope(scope, artifact.kind(), PluginLifecyclePhase::Shutdown));
        let caller = instance.caller();
        self.control.lock().unwrap().callers.insert(artifact.id.clone(), caller.clone());
        for dependency in artifact.requires.iter().filter(|_| !self.control.lock().unwrap().skip_dependency_calls) {
            self.call(&caller, &format!("{}.run", dependency.id), request_payload()).await?;
        }
        let registration = self.operations().stage_plugin(instance.scope().unwrap(), |bindings| {
            for definition in &artifact.contributions.operations {
                match artifact.id.as_str() {
                    "provider" => bind::<0>(bindings, definition)?,
                    "client" => bind::<1>(bindings, definition)?,
                    "other" => bind::<2>(bindings, definition)?,
                    _ => panic!("unknown fixture owner"),
                }
            }
            Ok(())
        })?;
        owned.registration = Some(registration);
        if let Some(started) = &self.started {
            started.send_replace(artifact.id.clone());
        }
        if self.block.as_ref() == Some(&artifact.id) {
            std::future::pending::<()>().await;
        }
        if self.fail_start.as_ref() == Some(&artifact.id) {
            return Err(Error::backend(
                "initialize plugin",
                harmos_runtime::BackendFailure::Permanent,
                std::io::Error::other("initialization failed"),
            ));
        }
        Ok(())
    }
    async fn stop(artifact: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        assert_eq!(authority.owner(), artifact.id);
        assert_eq!(authority.phase(), PluginLifecyclePhase::Shutdown);
        assert_eq!(authority.backend_kind(), backend.kind());
        let owned = activation(backend);
        let context = owned.stopping.as_ref().unwrap();
        let _wait = StopWait {
            owner: artifact.id.clone(),
            control: context.control.clone(),
        };
        {
            let mut control = context.control.lock().unwrap();
            control.events.push(format!("stop:{}", artifact.id));
            *control.stop_calls.entry(artifact.id.clone()).or_default() += 1;
        }
        context.entered.notify_one();
        if let Some(instance) = &owned.endpoint {
            assert!(authority.matches_scope(instance.scope().unwrap(), artifact.kind(), PluginLifecyclePhase::Shutdown));
        }
        if !owned.dependencies_stopped {
            if let Some(instance) = &owned.endpoint {
                let skip = context.control.lock().unwrap().skip_dependency_calls;
                for dependency in artifact.requires.iter().filter(|_| !skip) {
                    context
                        .operations
                        .caller(&instance.caller())?
                        .call_encoded(&format!("{}.run", dependency.id), request_payload())
                        .await?;
                }
            }
            owned.dependencies_stopped = true;
        }
        loop {
            let released = context.release.notified();
            let blocked = context.control.lock().unwrap().block_stop.as_ref() == Some(&artifact.id);
            if !blocked {
                break;
            }
            released.await;
        }
        let panics = context.control.lock().unwrap().panic_stop.as_ref() == Some(&artifact.id);
        assert!(!panics, "scripted physical stop panic");
        if context.control.lock().unwrap().fail_stop.as_ref() == Some(&artifact.id) {
            return Err(Error::backend(
                "shutdown plugin",
                harmos_runtime::BackendFailure::Permanent,
                std::io::Error::other("cleanup failed"),
            ));
        }
        if let Some(registration) = &mut owned.registration {
            registration.shutdown().await?;
        }
        owned.registration.take();
        owned.endpoint.take();
        context.control.lock().unwrap().callers.remove(&artifact.id);
        Ok(())
    }
}

fn activation(backend: &mut Backends) -> &mut Activation {
    match backend {
        PluginBackend::Guest(endpoint) | PluginBackend::Sidecar(endpoint) | PluginBackend::Embedded(endpoint) => endpoint,
    }
}

fn plan() -> PluginGraph {
    PluginGraph::resolve(
        [
            ("provider", PluginKind::Embedded, vec![]),
            ("client", PluginKind::Sidecar, vec!["provider"]),
            ("other", PluginKind::Guest, vec![]),
        ]
        .into_iter()
        .map(|(id, kind, requires)| PluginDefinition {
            id: id.to_owned(),
            version: "1.0.0".parse().unwrap(),
            artifact: match kind {
                PluginKind::Guest => harmos_runtime::plugin::PluginArtifact::Guest { sha256: [0; 32] },
                PluginKind::Sidecar => harmos_runtime::plugin::PluginArtifact::LocalSidecar { revision: "fixture".into() },
                PluginKind::Embedded => harmos_runtime::plugin::PluginArtifact::Embedded { revision: "fixture".into() },
            },
            configuration: b"null".to_vec(),
            contributions: harmos_runtime::plugin::PluginContributions::default(),
            requires: requires
                .into_iter()
                .map(|id| Requirement {
                    id: id.to_owned(),
                    version: "*".parse().unwrap(),
                })
                .collect(),
        }),
    )
    .unwrap()
}

fn inventories(plan: &PluginGraph) -> Vec<(String, Vec<Definition>)> {
    plan.ordered()
        .iter()
        .map(|artifact| {
            let operation = match artifact.id.as_str() {
                "provider" => Definition::of::<Run<0, 100, false>>().unwrap(),
                "client" => Definition::of::<Run<1, 100, false>>().unwrap(),
                "other" => Definition::of::<Run<2, 100, false>>().unwrap(),
                _ => panic!("unknown fixture owner"),
            };
            (artifact.id.clone(), vec![operation])
        })
        .collect()
}

fn failure(error: Error) -> PluginFailure {
    match error {
        Error::PluginLifecycle { source } => *source,
        error => panic!("expected typed plugin failure, got {error}"),
    }
}

trait ReloadParts {
    async fn reload_parts<L: PluginLifecycle<Guest = Activation, Sidecar = Activation, Embedded = Activation>>(
        &mut self, graph: PluginGraph, inventories: impl IntoIterator<Item = (String, Vec<Definition>)>,
        rebuilt: impl IntoIterator<Item = String>, lifecycle: &mut L,
    ) -> Result<()>;
}

impl ReloadParts for Manager {
    async fn reload_parts<L: PluginLifecycle<Guest = Activation, Sidecar = Activation, Embedded = Activation>>(
        &mut self, graph: PluginGraph, inventories: impl IntoIterator<Item = (String, Vec<Definition>)>,
        rebuilt: impl IntoIterator<Item = String>, lifecycle: &mut L,
    ) -> Result<()> {
        let candidate = candidate_parts(&graph, inventories, rebuilt)?;
        self.reload(candidate, lifecycle).await
    }
}

fn candidate_parts(
    graph: &PluginGraph, inventories: impl IntoIterator<Item = (String, Vec<Definition>)>, rebuilt: impl IntoIterator<Item = String>,
) -> Result<PluginDefinitions> {
    let mut inventories: BTreeMap<_, _> = inventories.into_iter().collect();
    let definitions = graph.ordered().iter().cloned().map(|mut definition| {
        definition.contributions.operations = inventories.remove(&definition.id).unwrap_or_default();
        definition
    });
    PluginDefinitions::new(definitions, rebuilt)
}

fn session() -> Manager {
    let plan = plan();
    PluginManager::new(candidate_parts(&plan, inventories(&plan), []).unwrap()).unwrap()
}

fn candidate(rebuilt: impl IntoIterator<Item = String>) -> PluginDefinitions {
    let graph = plan();
    candidate_parts(&graph, inventories(&graph), rebuilt).unwrap()
}
