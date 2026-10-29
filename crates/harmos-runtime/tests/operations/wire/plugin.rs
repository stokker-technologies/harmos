/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[path = "plugin_activation.rs"]
mod plugin_activation;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::cases::Text;
use super::{Compute, Failed, Lanes, Operations, Owner, Parent, ParentOwner, Request, Response, request};
use harmos_runtime::operations::{OperationDeclaration, PluginRegistration, WireDeclaration};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginManager, PluginScope, PluginTransition, Requirement, Router,
};
use harmos_runtime::{Error, Message, Result, operations::OperationManager};

type Backends = PluginBackend<Resources, Resources, Resources>;

#[harmos_runtime::operation(resources = [SelectedOwner], id = "fixture.selected_provider",
    response = String,
    visibility = Public,
    policy = super::cases::EchoDeclaration::POLICY
)]
struct SelectedDeclaration {
    value: String,
}

struct SelectedOwner(&'static str);

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "fixture.discover",
    response = Vec<u8>, policy = super::cases::EchoDeclaration::POLICY
)]
struct Discover;

struct SelectedHost {
    operations: OperationManager<Lanes>,
    scopes: BTreeMap<String, PluginScope>,
    callers: BTreeMap<String, PluginCaller>,
}

// Shares The Compute Request, Which Stays In Process.
#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "test.public",
    response = Response,
    visibility = Public,
    policy = Compute::POLICY,
    key = |request: &Public| Compute::key(&request.0),
)]
#[derive(Clone)]
struct Public(Request);

#[harmos_runtime::operation(resources = [CleanupOwner], id = "fixture.callback_cleanup",
    response = String,
    policy = super::cases::EchoDeclaration::POLICY
)]
struct CleanupDeclaration {
    value: String,
}

#[derive(Default)]
struct CleanupOwner {
    started: tokio::sync::Notify,
    finalized: AtomicUsize,
    attempted: AtomicUsize,
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
    discovery: bool,
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

super::in_process!(Public);

impl harmos_runtime::MemorySize for Public {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl From<&str> for SelectedDeclaration {
    fn from(value: &str) -> Self {
        Self { value: value.to_owned() }
    }
}

impl harmos_runtime::Operation for SelectedDeclaration {
    type Error = Failed;

    async fn run(
        self, ctx: harmos_runtime::Context<'_, Self>,
    ) -> std::result::Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = ctx.resource::<SelectedOwner>();
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(format!("{}:{}", resources.0, self.value)) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for Discover {
    type Error = harmos_operations::Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<Vec<u8>>, Self::Error> {
        use harmos_sdk::operations::access::Discovery;
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let (entries, next) = host.discover(None, 256, 4 * 1024 * 1024).map_err(harmos_operations::Error::Access)?;
        assert!(next.is_none());
        Ok(entries.into())
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
        match definition.id.as_str() {
            "first" | "second" => {}
            "client" => return Ok(()),
            _ => unreachable!("closed fixture owner set"),
        }
        resources.registration = Some(
            self.operations
                .stage_plugin(scope, |bindings| bindings.operations::<crate::operations::Single<SelectedDeclaration>>(Lanes::Open))?,
        );
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

impl harmos_runtime::Operation for Public {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<Response>, Failed> {
        let owner = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            owner.prepared.fetch_add(1, Ordering::SeqCst);
        }
        owner.compute(&self.0, context).await.map(Into::into)
    }
}

struct CleanupGuard(Arc<CleanupOwner>);
impl harmos_runtime::Operation for CleanupDeclaration {
    type Error = Failed;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<String>, Self::Error> {
        let resources = ctx.resource::<CleanupOwner>();
        let _guard = CleanupGuard(resources.clone());
        resources.attempted.fetch_add(1, Ordering::SeqCst);
        resources.started.notify_one();
        if matches!(self.value.as_str(), "block" | "expire") {
            std::future::pending::<()>().await;
        }
        Ok(self.value.into())
    }
}
impl Drop for CleanupGuard {
    fn drop(&mut self) {
        self.0.finalized.fetch_add(1, Ordering::SeqCst);
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
                assert_eq!(self.operations.caller(&caller)?.call::<Public>(Public(request())).await?.value, 7);
            }
            return Ok(());
        }
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        self.scope = Some(scope.clone());
        if self.skip_operations {
            return Ok(());
        }
        resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.operations::<Operations>(Lanes::Open)?;
            bindings.operations::<super::cases::Operations>(Lanes::Open)?;
            if self.cleanup.is_some() {
                bindings.operations::<crate::operations::Single<CleanupDeclaration>>(Lanes::Open)?;
            }
            if self.discovery {
                bindings.operations::<crate::operations::Single<Discover>>(Lanes::Open)?;
            }
            bindings.operations::<crate::operations::Single<Public>>(Lanes::Serial)?;
            bindings.operations::<crate::operations::Single<Parent>>(Lanes::Serial)
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
    async fn start() -> Self {
        let cleanup = Arc::new(CleanupOwner::default());
        let runtime = runtime_with(|setup| {
            setup.resource::<CleanupOwner>(cleanup.clone());
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(128).unwrap()),
                ..Default::default()
            });
        })
        .await;
        let mut manager = PluginManager::new(candidate_with_cleanup()).unwrap();
        let mut host = host(runtime.operations.clone());
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
        self.runtime
            .operations
            .unregister(&harmos_runtime::QualifiedId::qualify("fixture", CleanupDeclaration::ID).unwrap())
            .await
            .unwrap();
        assert_eq!(self.cleanup.finalized.load(Ordering::SeqCst), 1);
        let bytes = self.echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Text::from("settled").encode())).unwrap().await.unwrap();
        assert_eq!(String::decode(&bytes).unwrap(), "settled:1");
        self.manager.shutdown(&mut self.host).await.unwrap();
        self.runtime.shutdown().await.unwrap();
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

async fn runtime() -> harmos_runtime::Runtime<super::Bank> {
    runtime_with(|_| {}).await
}

async fn runtime_with(update: impl FnOnce(&mut harmos_runtime::Setup<super::Bank>)) -> harmos_runtime::Runtime<super::Bank> {
    super::runtime_with(|setup| {
        setup.resource(super::cases::Destination("fixture"));
        update(setup);
    })
    .await
}

async fn runtime_with_preparation(bytes: NonZeroUsize) -> harmos_runtime::Runtime<super::Bank> {
    runtime_with(|setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(bytes),
            ..Default::default()
        });
    })
    .await
}

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
    operations.push(harmos_runtime::operations::OperationDefinition::of::<Public>().unwrap());
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
    let owner = operations.context().resource::<Owner>().unwrap();
    Host {
        operations,
        owner,
        scope: None,
        fail_start: false,
        skip_operations: false,
        drop_before_commit: false,
        unregister_before_commit: false,
        drop_receipt: Arc::default(),
        callers: BTreeMap::new(),
        call_during_setup: false,
        cleanup: None,
        discovery: false,
    }
}

fn candidate_with_cleanup() -> PluginDefinitions {
    let mut definition = candidate().definitions().next().unwrap().clone();
    definition.contributions.operations.push(harmos_runtime::operations::OperationDefinition::of::<CleanupDeclaration>().unwrap());
    PluginDefinitions::new([definition], []).unwrap()
}
