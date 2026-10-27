/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "plugin_activation.rs"]
mod plugin_activation;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// Third Party Imports

// Local Imports
use super::cases::Text;
use super::{
    Compute, ComputeCall, Failed, Lanes, Operations, Owner, Parent, ParentCall, ParentOwner, Request, Response, Slow, SlowCall, State,
    request, runtime,
};
use harmos_runtime::operations::{FailureKind, OperationDeclaration, PluginRegistration, WireDeclaration};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginManager, PluginScope, PluginTransition, Requirement, Router,
};
use harmos_runtime::{Error, Message, Result, operations::OperationManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources, Resources>;

/* === Structs === */

#[harmos_runtime::operation(id = "fixture_selected_provider",
    response = String,
    visibility = Shared,
    policy = super::cases::EchoDeclaration::POLICY
)]
struct SelectedDeclaration {
    value: String,
}

struct SelectedOwner(&'static str);

struct SelectedHost {
    operations: OperationManager<Lanes>,
    scopes: BTreeMap<String, PluginScope>,
    callers: BTreeMap<String, PluginCaller>,
}

// Shares The Compute Request, Which Stays In Process.
#[harmos_runtime::operation(codec = manual, id = "test_shared",
    response = Response,
    visibility = Shared,
    policy = Compute::POLICY,
    key = |request: &Shared| Compute::key(&request.0),
)]
struct Shared(Request);

#[harmos_runtime::operation(id = "fixture_callback_cleanup",
    response = String,
    policy = super::cases::EchoDeclaration::POLICY
)]
struct CleanupDeclaration {
    value: String,
}

#[derive(Default)]
struct CleanupOwner {
    expire: AtomicBool,
    started: tokio::sync::Notify,
    finalizing: tokio::sync::Notify,
    release: tokio::sync::Notify,
    fail: AtomicBool,
    finalized: AtomicUsize,
    attempted: AtomicUsize,
    failed: tokio::sync::Notify,
}

#[derive(Default)]
struct Resources {
    drop_receipt: Arc<std::sync::atomic::AtomicBool>,
    endpoint: Option<PluginEndpoint>,
    registration: Option<PluginRegistration>,
}

struct Host {
    operations: OperationManager<Lanes>,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    fail_start: bool,
    skip_operations: bool,
    drop_before_commit: bool,
    unregister_before_commit: bool,
    drop_receipt: Arc<std::sync::atomic::AtomicBool>,
    callers: BTreeMap<String, PluginCaller>,
    call_during_setup: bool,
    cleanup: Option<Arc<CleanupOwner>>,
}

struct Credit {
    runtime: harmos_runtime::Runtime<super::Bank>,
    manager: PluginManager<Resources, Resources, Resources>,
    host: Host,
    cleanup: Arc<CleanupOwner>,
    target: harmos_runtime::operations::PluginCallback,
    echo: harmos_runtime::operations::PluginCallback,
}

struct Running {
    runtime: harmos_runtime::Runtime<super::Bank>,
    manager: PluginManager<Resources, Resources, Resources>,
    host: Host,
    gate: Arc<tokio::sync::Notify>,
    invocation: harmos_runtime::operations::Running<Compute>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

super::in_process!(Shared);

impl harmos_runtime::MemorySize for Shared {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl From<&str> for SelectedDeclaration {
    fn from(value: &str) -> Self {
        Self { value: value.to_owned() }
    }
}

struct Selecting {
    resources: Arc<SelectedOwner>,
}

impl harmos_runtime::operations::Handler for Selecting {
    type Request = SelectedDeclaration;
    type Response = <SelectedDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type StreamEndpoints = ();
    type Error = Failed;
    async fn initialize(context: harmos_runtime::ExecutionContext) -> std::result::Result<Self, Failed> {
        let resources = context.resource::<SelectedOwner>().expect("fixture registers its dependency");
        Ok(Self { resources })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, request: &mut SelectedDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> =
            async move { Ok(format!("{}:{}", self.resources.0, request.value)) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl PluginLifecycle for SelectedHost {
    type Guest = Resources;
    type Sidecar = Resources;
    type Embedded = Resources;
    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Resources::default()))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        resources.endpoint = Some(router.enter(&definition.id).unwrap());
        let endpoint = resources.endpoint.as_ref().unwrap();
        let scope = endpoint.scope().unwrap();
        self.scopes.insert(definition.id.clone(), scope.clone());
        self.callers.insert(definition.id.clone(), endpoint.caller());
        let owner = match definition.id.as_str() {
            "first" => SelectedOwner("first"),
            "second" => SelectedOwner("second"),
            "client" => return Ok(()),
            _ => unreachable!("closed fixture owner set"),
        };
        resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(Arc::new(owner)).unwrap();
                    dependencies
                })
                .implementation::<SelectedDeclaration, Selecting>(),
                Lanes::Open,
            ))
        })?);
        Ok(())
    }
    async fn stop(_: &PluginDefinition, _: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        if let Some(registration) = &mut resources.registration {
            registration.shutdown().await?;
        }
        resources.registration.take();
        resources.endpoint.take();
        Ok(())
    }
}

struct SharedCall {
    owner: Arc<Owner>,
    state: State,
}

impl harmos_runtime::operations::Handler for SharedCall {
    type StreamEndpoints = ();
    type Request = Shared;
    type Response = <Shared as harmos_runtime::OperationDeclaration>::Response;
    type Error = Failed;
    async fn initialize(context: harmos_runtime::ExecutionContext) -> std::result::Result<Self, Failed> {
        let owner = context.resource::<Owner>().expect("fixture registers its dependency");
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        let state = super::State { attempts: 0 };
        Ok(Self { owner, state })
    }
    async fn run(
        &mut self, context: harmos_runtime::ExecutionContext, request: &mut Shared, _endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<Response>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<Response, Self::Error> =
            async move { self.owner.compute(&request.0, &mut self.state, context).await }.await;
        result.map(::core::convert::Into::into)
    }
}

struct Cleaning {
    resources: Arc<CleanupOwner>,
    wait_cleanup: bool,
}

impl harmos_runtime::operations::Handler for Cleaning {
    type Request = CleanupDeclaration;
    type Response = <CleanupDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type StreamEndpoints = ();
    type Error = Failed;
    async fn initialize(context: harmos_runtime::ExecutionContext) -> std::result::Result<Self, Failed> {
        let resources = context.resource::<CleanupOwner>().expect("fixture registers its dependency");
        // The Deadline Passes Before Construction Returns (Paused Clock Only).
        let wait_cleanup = resources.expire.load(Ordering::SeqCst);
        if wait_cleanup {
            tokio::time::advance(CleanupDeclaration::POLICY.timeout).await;
        }
        Ok(Self { resources, wait_cleanup })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, request: &mut CleanupDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            self.wait_cleanup |= request.value == "block";
            self.resources.attempted.fetch_add(1, Ordering::SeqCst);
            if request.value == "block" {
                self.resources.started.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(request.value.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> std::result::Result<(), Failed> {
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        if self.wait_cleanup {
            self.resources.finalizing.notify_one();
            self.resources.release.notified().await;
        }
        if self.resources.fail.load(Ordering::SeqCst) {
            self.resources.failed.notify_one();
            return Err(Failed::Scripted(FailureKind::Permanent));
        }
        Ok(())
    }
}

impl PluginLifecycle for Host {
    type Guest = Resources;
    type Sidecar = Resources;
    type Embedded = Resources;

    fn prepare(&mut self, _definition: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Resources {
            drop_receipt: self.drop_receipt.clone(),
            ..Resources::default()
        }))
    }

    async fn fence(&mut self, _transition: PluginTransition, _affected: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        resources.endpoint = Some(router.enter(&definition.id).unwrap());
        let caller = resources.endpoint.as_ref().unwrap().caller();
        self.callers.insert(definition.id.clone(), caller.clone());
        if definition.id != "fixture" {
            if self.call_during_setup && definition.id == "client" {
                // Source is staged; its declared provider was committed earlier.
                assert_eq!(self.operations.caller(&caller)?.call::<Shared>(Shared(request())).await?.value, 7);
            }
            return Ok(());
        }
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        self.scope = Some(scope.clone());
        if self.skip_operations {
            return Ok(());
        }
        resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(self.owner.clone()).unwrap();
                    dependencies
                })
                .implementation::<Compute, ComputeCall>()
                .implementation::<Slow, SlowCall>(),
                Lanes::Open,
            ))?;
            bindings.register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(self.owner.clone()).unwrap();
                    dependencies
                })
                .implementation::<super::cases::EchoDeclaration, super::cases::Echoing>()
                .implementation::<super::cases::PrivateEchoDeclaration, super::cases::PrivateEchoing>()
                .implementation::<super::cases::NestedDeclaration, super::cases::Nesting<true>>()
                .implementation::<super::cases::DetachedDeclaration, super::cases::Detaching>(),
                Lanes::Open,
            ))?;
            if let Some(cleanup) = &self.cleanup {
                bindings.register(harmos_runtime::RegistrationExt::lane(
                    harmos_runtime::RegistrationBatch::new({
                        let mut dependencies = harmos_runtime::Dependencies::new();
                        dependencies.insert(cleanup.clone()).unwrap();
                        dependencies
                    })
                    .implementation::<CleanupDeclaration, Cleaning>(),
                    Lanes::Open,
                ))?;
            }
            bindings.register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(self.owner.clone()).unwrap();
                    dependencies
                })
                .implementation::<Shared, SharedCall>(),
                Lanes::Serial,
            ))?;
            bindings.register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(Arc::new(ParentOwner)).unwrap();
                    dependencies
                })
                .implementation::<Parent, ParentCall>(),
                Lanes::Serial,
            ))
        })?);

        assert!(self.operations.definitions().is_empty());
        assert!(matches!(self.operations.definition(Compute::ID), Err(Error::NotReady { .. })));
        assert!(matches!(self.operations.submit::<Compute>(request()), Err(Error::NotReady { .. })));
        if self.unregister_before_commit {
            self.operations.unregister(&harmos_runtime::QualifiedId::qualify("fixture", Compute::ID).unwrap()).await?;
        }
        if self.drop_before_commit {
            resources.registration.take();
        }
        if self.fail_start {
            return Err(Error::InvalidInput {
                field: "test activation",
                reason: "scripted initialization failure",
            });
        }
        Ok(())
    }

    async fn stop(
        _definition: &PluginDefinition, _authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        if !resources.drop_receipt.load(std::sync::atomic::Ordering::SeqCst)
            && let Some(registration) = resources.registration.as_mut()
        {
            registration.shutdown().await?;
        }
        resources.registration.take();
        resources.endpoint.take();
        Ok(())
    }
}

impl Credit {
    async fn start(expire: bool) -> Self {
        let runtime = super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
        let mut manager = PluginManager::new(candidate_with_cleanup()).unwrap();
        let mut host = host(runtime.operations.clone());
        let cleanup = Arc::new(CleanupOwner::default());
        cleanup.fail.store(expire, Ordering::SeqCst);
        cleanup.expire.store(expire, Ordering::SeqCst);
        host.cleanup = Some(cleanup.clone());
        manager.initialize(&mut host).await.unwrap();
        let scope = host.scope.as_ref().unwrap();
        let target = runtime.operations.capture_callback(scope, &WireDeclaration::of::<CleanupDeclaration>().contract().unwrap()).unwrap();
        let echo = runtime
            .operations
            .capture_callback(scope, &WireDeclaration::of::<super::cases::EchoDeclaration>().contract().unwrap())
            .unwrap();
        Self {
            runtime,
            manager,
            host,
            cleanup,
            target,
            echo,
        }
    }

    fn assert_held(&self) {
        let encoded = AtomicBool::new(false);
        assert!(matches!(
            self.echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                encoded.store(true, Ordering::SeqCst);
                Ok(Text::from("busy").encode())
            }),
            Err(Error::ResourceExhausted { resource: "operation preparation bytes" })
        ));
        assert!(!encoded.load(Ordering::SeqCst));
    }

    async fn settle(mut self) {
        self.cleanup.fail.store(false, Ordering::SeqCst);
        self.cleanup.release.notify_one();
        self.runtime
            .operations
            .unregister(&harmos_runtime::QualifiedId::qualify("fixture", CleanupDeclaration::ID).unwrap())
            .await
            .unwrap();
        assert_eq!(self.cleanup.finalized.load(Ordering::SeqCst), 2);
        let bytes = self.echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("settled").encode())).unwrap().await.unwrap();
        assert_eq!(String::decode(&bytes).unwrap(), "settled:1");
        self.manager.shutdown(&mut self.host).await.unwrap();
        self.runtime.stop().await.unwrap();
    }
}

impl Running {
    async fn start() -> Self {
        let runtime = runtime().await;
        let mut manager = PluginManager::new(candidate()).unwrap();
        let mut host = host(runtime.operations.clone());
        host.drop_receipt.store(true, Ordering::SeqCst);
        manager.initialize(&mut host).await.unwrap();
        let opened = Arc::new(tokio::sync::Notify::new());
        let gate = Arc::new(tokio::sync::Notify::new());
        let invocation = runtime
            .operation(super::Request {
                gate: Some((opened.clone(), gate.clone())),
                ..request()
            })
            .start()
            .await
            .unwrap();
        opened.notified().await;
        Self { runtime, manager, host, gate, invocation }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn selected_candidate(provider: &str, first: bool) -> PluginDefinitions {
    let contract = WireDeclaration::of::<SelectedDeclaration>().contract().unwrap();
    let owners = if first { vec!["first", "second", "client"] } else { vec!["second", "client"] };
    let definitions = owners.into_iter().map(|id| PluginDefinition {
        id: id.into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded { revision: "fixture".into() },
        requires: vec![],
        configuration: b"null".to_vec(),
        contributions: if id == "client" {
            PluginContributions {
                required_contracts: Some(vec![harmos_domain::inventory::contracts::Required {
                    contract: contract.clone(),
                    grants: Vec::new(),
                }]),
                ..PluginContributions::default()
            }
        } else {
            PluginContributions {
                operations: vec![contract.operation().clone()],
                wire_contracts: vec![contract.clone()],
                required_contracts: Some(vec![]),
                ..PluginContributions::default()
            }
        },
    });
    PluginDefinitions::new_with_providers(
        definitions,
        [],
        [harmos_runtime::plugin::OperationProvider {
            consumer: "client".into(),
            operation: harmos_runtime::Identifier::new(SelectedDeclaration::ID).unwrap(),
            provider: provider.into(),
        }],
    )
    .unwrap()
}

fn candidate() -> PluginDefinitions {
    let mut operations = harmos_runtime::operations::Declarations::of::<Operations>().unwrap();
    operations.extend(harmos_runtime::operations::Declarations::of::<super::cases::Operations>().unwrap());
    operations.push(harmos_runtime::operations::OperationDefinition::of::<Shared>().unwrap());
    operations.push(harmos_runtime::operations::OperationDefinition::of::<Parent>().unwrap());
    PluginDefinitions::new(
        [PluginDefinition {
            id: "fixture".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::Embedded { revision: "fixture".into() },
            requires: Vec::new(),
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                operations,
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

fn with_clients(client_requires_provider: bool) -> PluginDefinitions {
    let provider = candidate().definitions().next().unwrap().clone();
    let clients = [
        ("client", if client_requires_provider { vec!["fixture"] } else { Vec::new() }),
        ("transitive", vec!["client"]),
        ("unrelated", Vec::new()),
    ]
    .into_iter()
    .map(|(id, requires)| PluginDefinition {
        id: id.into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded { revision: "fixture".into() },
        requires: requires
            .into_iter()
            .map(|id| Requirement {
                id: id.into(),
                version: "*".parse().unwrap(),
            })
            .collect(),
        configuration: b"null".to_vec(),
        contributions: PluginContributions::default(),
    });
    PluginDefinitions::new(std::iter::once(provider).chain(clients), []).unwrap()
}

fn host(operations: OperationManager<Lanes>) -> Host {
    Host {
        operations,
        owner: Arc::new(Owner::default()),
        scope: None,
        fail_start: false,
        skip_operations: false,
        drop_before_commit: false,
        unregister_before_commit: false,
        drop_receipt: Arc::default(),
        callers: BTreeMap::new(),
        call_during_setup: false,
        cleanup: None,
    }
}

fn candidate_with_cleanup() -> PluginDefinitions {
    let mut definition = candidate().definitions().next().unwrap().clone();
    definition.contributions.operations.push(harmos_runtime::operations::OperationDefinition::of::<CleanupDeclaration>().unwrap());
    PluginDefinitions::new([definition], []).unwrap()
}

/* === Tests === */
