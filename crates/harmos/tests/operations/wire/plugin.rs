/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// Third Party Imports

// Local Imports
use super::wire::Text;
use super::{
    Compute, ComputeCall, Failed, Implemented, Lanes, Operations, Owner, Parent, ParentCall,
    ParentOwner, Request, Response, SlowCall, State, request, runtime,
};
use harmos::operations::{
    CacheKey, Context, FailureKind, OperationDeclaration, OperationPolicy, PluginRegistration,
    Visibility, WireDeclaration, WireOperation,
};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition,
    PluginDefinitions, PluginEndpoint, PluginLifecycle, PluginManager, PluginScope,
    PluginTransition, Requirement, Router,
};
use harmos::{Error, Message, Result, operations::OperationManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<Resources, Resources, Resources>;

/* === Structs === */
struct Selected;

struct SelectedOwner(&'static str);

struct SelectedHost {
    operations: OperationManager,
    scopes: BTreeMap<String, PluginScope>,
    callers: BTreeMap<String, PluginCaller>,
}

struct Shared;

struct Cleanup;

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
    operations: OperationManager,
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

/* === Enums === */
/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Selected {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.selected_provider";
    const POLICY: OperationPolicy = super::wire::Echo::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl WireOperation for Selected {
    const VERSION: u32 = 1;
}

#[harmos::catalog]
enum SelectingOperations {
    Call(Selecting),
}

struct Selecting {
    resources: Arc<SelectedOwner>,
}

impl harmos::operations::Operation for Selecting {
    type Declaration = Selected;
    type Resources = SelectedOwner;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = Failed;
    async fn initialize<'ctx>(
        resources: Arc<SelectedOwner>,
        _: Self::Context<'ctx>,
    ) -> std::result::Result<Self, Failed> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::Response<String>, Failed> {
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
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        _: &harmos::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        resources.endpoint = Some(router.enter(&definition.id).unwrap());
        let endpoint = resources.endpoint.as_ref().unwrap();
        let scope = endpoint.scope().unwrap();
        self.scopes.insert(definition.id.clone(), scope.clone());
        self.callers
            .insert(definition.id.clone(), endpoint.caller());
        let owner = match definition.id.as_str() {
            "first" => SelectedOwner("first"),
            "second" => SelectedOwner("second"),
            "client" => return Ok(()),
            _ => unreachable!("closed fixture owner set"),
        };
        resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings
                .register_wire_operations::<SelectingOperations, _>(Lanes::Open, Arc::new(owner))
        })?);
        Ok(())
    }
    async fn stop(
        _: &PluginDefinition,
        _: &harmos::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        if let Some(registration) = &mut resources.registration {
            registration.shutdown().await?;
        }
        resources.registration.take();
        resources.endpoint.take();
        Ok(())
    }
}

impl OperationDeclaration for Shared {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "test.shared";
    const POLICY: OperationPolicy = Compute::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn key(request: &Request) -> Option<CacheKey> {
        Compute::key(request)
    }
}

struct SharedCall {
    owner: Arc<Owner>,
    state: State,
}

impl harmos::operations::Operation for SharedCall {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Shared;
    type Resources = Owner;
    type Error = Failed;
    async fn initialize<'ctx>(
        owner: Arc<Owner>,
        _: Self::Context<'ctx>,
    ) -> std::result::Result<Self, Failed> {
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        let state = super::State { attempts: 0 };
        Ok(Self { owner, state })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Request,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::Response<Response>, Failed> {
        let result: ::core::result::Result<Response, Self::Error> =
            async move { self.owner.compute(request, &mut self.state, context).await }.await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Cleanup {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.callback_cleanup";
    const POLICY: OperationPolicy = super::wire::Echo::POLICY;
}

impl WireOperation for Cleanup {
    const VERSION: u32 = 1;
}

#[harmos::catalog]
enum CleaningOperations {
    Call(Cleaning),
}

struct Cleaning {
    resources: Arc<CleanupOwner>,
    wait_cleanup: bool,
}

impl harmos::operations::Operation for Cleaning {
    type Declaration = Cleanup;
    type Resources = CleanupOwner;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = Failed;
    async fn initialize<'ctx>(
        resources: Arc<CleanupOwner>,
        _: Self::Context<'ctx>,
    ) -> std::result::Result<Self, Failed> {
        // Exercise Deadline Expiry During Synchronous Construction.
        let wait_cleanup = resources.expire.load(Ordering::SeqCst);
        if wait_cleanup {
            std::thread::sleep(Cleanup::POLICY.timeout);
        }
        Ok(Self {
            resources,
            wait_cleanup,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::Response<String>, Failed> {
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

    async fn fence(
        &mut self,
        _transition: PluginTransition,
        _affected: &[PluginDefinition],
    ) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        _authority: &harmos::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        resources.endpoint = Some(router.enter(&definition.id).unwrap());
        let caller = resources.endpoint.as_ref().unwrap().caller();
        self.callers.insert(definition.id.clone(), caller.clone());
        if definition.id != "fixture" {
            if self.call_during_setup && definition.id == "client" {
                // Source is staged; its declared provider was committed earlier.
                assert_eq!(
                    self.operations
                        .caller(&caller)?
                        .call_as::<Shared>(request())
                        .await?
                        .value,
                    7
                );
            }
            return Ok(());
        }
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        self.scope = Some(scope.clone());
        if self.skip_operations {
            return Ok(());
        }
        resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register_operations::<Implemented, _>(Lanes::Open, self.owner.clone())?;
            bindings.register_wire_operations::<super::wire::Implemented, _>(
                Lanes::Open,
                self.owner.clone(),
            )?;
            if let Some(cleanup) = &self.cleanup {
                bindings.register_wire_operations::<CleaningOperations, _>(
                    Lanes::Open,
                    cleanup.clone(),
                )?;
            }
            bindings.register_operation::<SharedCall>(Lanes::Serial, self.owner.clone())?;
            bindings.register_operation::<ParentCall>(Lanes::Serial, Arc::new(ParentOwner))
        })?);

        assert!(self.operations.definitions().is_empty());
        assert!(matches!(
            self.operations.definition(Compute::ID),
            Err(Error::NotReady { .. })
        ));
        assert!(matches!(
            self.operations.submit_as::<Compute>(request()),
            Err(Error::NotReady { .. })
        ));
        if self.unregister_before_commit {
            self.operations.unregister(Compute::ID).await?;
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
        _definition: &PluginDefinition,
        _authority: &harmos::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        if !resources
            .drop_receipt
            .load(std::sync::atomic::Ordering::SeqCst)
            && let Some(registration) = resources.registration.as_mut()
        {
            registration.shutdown().await?;
        }
        resources.registration.take();
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn selected_candidate(provider: &str, first: bool) -> PluginDefinitions {
    let contract = WireDeclaration::of::<Selected>().contract().unwrap();
    let owners = if first {
        vec!["first", "second", "client"]
    } else {
        vec!["second", "client"]
    };
    let definitions = owners.into_iter().map(|id| PluginDefinition {
        id: id.into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded {
            revision: "fixture".into(),
        },
        requires: vec![],
        configuration: b"null".to_vec(),
        contributions: if id == "client" {
            PluginContributions {
                required_contracts: Some(vec![harmos_contract::inventory::contracts::Required {
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
        [harmos::plugin::OperationProvider {
            consumer: "client".into(),
            operation: harmos::Identifier::new(Selected::ID).unwrap(),
            provider: provider.into(),
        }],
    )
    .unwrap()
}

fn candidate() -> PluginDefinitions {
    let mut operations = harmos::operations::Declarations::of::<Operations>().unwrap();
    operations.extend(harmos::operations::Declarations::of::<super::wire::Operations>().unwrap());
    operations.push(harmos::operations::Definition::of::<Shared>().unwrap());
    operations.push(harmos::operations::Definition::of::<Parent>().unwrap());
    PluginDefinitions::new(
        [PluginDefinition {
            id: "fixture".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::Embedded {
                revision: "fixture".into(),
            },
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
        (
            "client",
            if client_requires_provider {
                vec!["fixture"]
            } else {
                Vec::new()
            },
        ),
        ("transitive", vec!["client"]),
        ("unrelated", Vec::new()),
    ]
    .into_iter()
    .map(|(id, requires)| PluginDefinition {
        id: id.into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded {
            revision: "fixture".into(),
        },
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

fn host(operations: OperationManager) -> Host {
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
    definition
        .contributions
        .operations
        .push(harmos::operations::Definition::of::<Cleanup>().unwrap());
    PluginDefinitions::new([definition], []).unwrap()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("callback_requires_budget", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = harmos::Runtime::<super::support::Bank>::builder(
                        super::support::Ledger::default(),
                        (),
                    )
                    .register(super::support::catalog())
                    .register(Lanes::catalog())
                    .runtime()
                    .await
                    .unwrap();
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let contract = WireDeclaration::of::<super::wire::Echo>()
                        .contract()
                        .unwrap();
                    let target: harmos::operations::PluginCallback = runtime
                        .operations
                        .capture_callback(host.scope.as_ref().unwrap(), &contract)
                        .unwrap();
                    let encoded = AtomicBool::new(false);
                    let refused = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                        encoded.store(true, Ordering::SeqCst);
                        Ok(Text::from("value").encode_to_vec())
                    });
                    assert!(matches!(
                        refused,
                        Err(Error::InvalidInput {
                            field: "operation preparation bytes",
                            ..
                        })
                    ));
                    assert!(!encoded.load(Ordering::SeqCst));
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("callback_contract_generation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let contract = WireDeclaration::of::<super::wire::Echo>()
                        .contract()
                        .unwrap();
                    let target = runtime
                        .operations
                        .capture_callback(host.scope.as_ref().unwrap(), &contract)
                        .unwrap();
                    let mut changed = WireDeclaration::of::<super::wire::Echo>();
                    changed.version = 2;
                    assert!(matches!(
                        runtime.operations.capture_callback(
                            host.scope.as_ref().unwrap(),
                            &changed.contract().unwrap()
                        ),
                        Err(Error::InvalidInput {
                            field: "callback contract",
                            ..
                        })
                    ));
                    let mut definitions: Vec<_> = candidate().definitions().cloned().collect();
                    definitions[0].configuration = br#"{"generation":2}"#.to_vec();
                    manager
                        .reload(PluginDefinitions::new(definitions, []).unwrap(), &mut host)
                        .await
                        .unwrap();
                    let encoded = AtomicBool::new(false);
                    assert!(
                        target
                            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                                encoded.store(true, Ordering::SeqCst);
                                Ok(Text::from("stale").encode_to_vec())
                            })
                            .is_err()
                    );
                    assert!(!encoded.load(Ordering::SeqCst));
                    let replacement = runtime
                        .operations
                        .capture_callback(host.scope.as_ref().unwrap(), &contract)
                        .unwrap();
                    let bytes = replacement
                        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            Ok(Text::from("live").encode_to_vec())
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&bytes).unwrap(), "live:1");
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("encoder_panic_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let contract = WireDeclaration::of::<super::wire::Echo>()
                        .contract()
                        .unwrap();
                    let target = runtime
                        .operations
                        .capture_callback(host.scope.as_ref().unwrap(), &contract)
                        .unwrap();
                    assert!(matches!(
                        target.submit_with(NonZeroUsize::new(64).unwrap(), |_| panic!(
                            "fixture encoder panicked"
                        )),
                        Err(Error::OperationPanicked { .. })
                    ));
                    let bytes = target
                        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            Ok(Text::from("released").encode_to_vec())
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&bytes).unwrap(), "released:1");
                    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 1);
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("credit_survives_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
                    let mut manager = PluginManager::new(candidate_with_cleanup()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    let cleanup = Arc::new(CleanupOwner::default());
                    host.cleanup = Some(cleanup.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let scope = host.scope.as_ref().unwrap();
                    let cleanup_contract = WireDeclaration::of::<Cleanup>().contract().unwrap();
                    let target = runtime
                        .operations
                        .capture_callback(scope, &cleanup_contract)
                        .unwrap();
                    let echo_contract = WireDeclaration::of::<super::wire::Echo>()
                        .contract()
                        .unwrap();
                    let echo = runtime
                        .operations
                        .capture_callback(scope, &echo_contract)
                        .unwrap();
                    let invocation = target
                        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            Ok(Text::from("block").encode_to_vec())
                        })
                        .unwrap();
                    tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        cleanup.started.notified(),
                    )
                    .await
                    .unwrap();
                    let encoded = AtomicBool::new(false);
                    assert!(matches!(
                        echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            encoded.store(true, Ordering::SeqCst);
                            Ok(Text::from("active").encode_to_vec())
                        }),
                        Err(Error::ResourceExhausted {
                            resource: "operation preparation bytes"
                        })
                    ));
                    assert!(!encoded.load(Ordering::SeqCst));
                    invocation.cancel();
                    tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        cleanup.finalizing.notified(),
                    )
                    .await
                    .unwrap();
                    let blocked = echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                        encoded.store(true, Ordering::SeqCst);
                        Ok(Text::from("busy").encode_to_vec())
                    });
                    assert!(matches!(
                        blocked,
                        Err(Error::ResourceExhausted {
                            resource: "operation preparation bytes"
                        })
                    ));
                    assert!(!encoded.load(Ordering::SeqCst));
                    cleanup.fail.store(true, Ordering::SeqCst);
                    cleanup.release.notify_one();
                    assert!(matches!(
                        invocation.await,
                        Err(Error::OperationFinalization { .. })
                    ));
                    assert!(matches!(
                        echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            encoded.store(true, Ordering::SeqCst);
                            Ok(Text::from("still busy").encode_to_vec())
                        }),
                        Err(Error::ResourceExhausted {
                            resource: "operation preparation bytes"
                        })
                    ));
                    assert!(!encoded.load(Ordering::SeqCst));
                    cleanup.fail.store(false, Ordering::SeqCst);
                    cleanup.release.notify_one();
                    runtime.operations.unregister(Cleanup::ID).await.unwrap();
                    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 2);
                    let bytes = echo
                        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            Ok(Text::from("settled").encode_to_vec())
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&bytes).unwrap(), "settled:1");
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("catalog_waits_manager", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    assert_eq!(runtime.operations.definitions().len(), 8);
                    assert_eq!(
                        runtime.operations.owner(Compute::ID).unwrap().as_str(),
                        "fixture"
                    );
                    assert!(matches!(
                        runtime
                            .operations
                            .stage_plugin(host.scope.as_ref().unwrap(), |_| Ok(())),
                        Err(Error::InvalidInput { .. })
                    ));
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Compute>(request())
                            .await
                            .unwrap()
                            .value,
                        7
                    );

                    manager.shutdown(&mut host).await.unwrap();
                    assert!(runtime.operations.definitions().is_empty());
                    assert!(matches!(
                        runtime
                            .operations
                            .stage_plugin(host.scope.as_ref().unwrap(), |_| Ok(())),
                        Err(Error::Closing { .. })
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("setup_conflicts_release", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    host.fail_start = true;
                    assert!(manager.initialize(&mut host).await.is_err());
                    assert!(runtime.operations.definitions().is_empty());
                    assert_eq!(
                        host.owner
                            .prepared
                            .load(std::sync::atomic::Ordering::SeqCst),
                        0
                    );
                    assert!(
                        runtime
                            .operations
                            .register_operation::<ComputeCall>(
                                "fixture",
                                Lanes::Open,
                                Arc::new(Owner::default())
                            )
                            .is_ok()
                    );

                    host.fail_start = false;
                    assert!(manager.initialize(&mut host).await.is_err());
                    // The second binding was never reserved when the complete candidate collided.
                    assert!(
                        runtime
                            .operations
                            .register_operation::<SlowCall>(
                                "fixture",
                                Lanes::Open,
                                Arc::new(Owner::default())
                            )
                            .is_ok()
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("receipt_keeps_running", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    host.drop_receipt
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    manager.initialize(&mut host).await.unwrap();
                    let opened = Arc::new(tokio::sync::Notify::new());
                    let gate = Arc::new(tokio::sync::Notify::new());
                    let invocation = runtime
                        .operations
                        .submit_as::<Compute>(super::Request {
                            gate: Some((opened.clone(), gate.clone())),
                            ..request()
                        })
                        .unwrap();
                    tokio::time::timeout(std::time::Duration::from_secs(2), opened.notified())
                        .await
                        .unwrap();
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(runtime.operations.definitions().is_empty());
                    assert!(matches!(
                        runtime.operations.register_operation::<ComputeCall>(
                            "fixture",
                            Lanes::Open,
                            Arc::new(Owner::default())
                        ),
                        Err(Error::AlreadyRegistered { .. })
                    ));

                    gate.notify_one();
                    assert_eq!(invocation.await.unwrap().value, 7);
                    runtime
                        .operations
                        .register_operation::<ComputeCall>(
                            "fixture",
                            Lanes::Open,
                            Arc::new(Owner::default()),
                        )
                        .unwrap();
                    assert_eq!(
                        runtime.operations.owner(Compute::ID).unwrap().as_str(),
                        "fixture"
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("receipt_drop_preserves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(candidate()).unwrap();
                    let mut host = host(runtime.operations.clone());
                    host.drop_receipt
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    manager.initialize(&mut host).await.unwrap();
                    let opened = Arc::new(tokio::sync::Notify::new());
                    let gate = Arc::new(tokio::sync::Notify::new());
                    let invocation = runtime
                        .operations
                        .submit_as::<Compute>(super::Request {
                            gate: Some((opened.clone(), gate.clone())),
                            ..request()
                        })
                        .unwrap();
                    tokio::time::timeout(std::time::Duration::from_secs(2), opened.notified())
                        .await
                        .unwrap();
                    assert!(
                        tokio::time::timeout(
                            std::time::Duration::from_millis(10),
                            runtime.operations.unregister(Compute::ID)
                        )
                        .await
                        .is_err()
                    );
                    manager.shutdown(&mut host).await.unwrap();

                    gate.notify_one();
                    assert_eq!(invocation.await.unwrap().value, 7);
                    assert!(matches!(
                        runtime.operations.register_operation::<ComputeCall>(
                            "fixture",
                            Lanes::Open,
                            Arc::new(Owner::default())
                        ),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    runtime.operations.unregister(Compute::ID).await.unwrap();
                    runtime
                        .operations
                        .register_operation::<ComputeCall>(
                            "fixture",
                            Lanes::Open,
                            Arc::new(Owner::default()),
                        )
                        .unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("caller_permissions_precede", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(with_clients(true)).unwrap();
                    let mut host = host(runtime.operations.clone());
                    host.call_during_setup = true;
                    manager.initialize(&mut host).await.unwrap();
                    let caller = runtime.operations.caller(&host.callers["client"]).unwrap();
                    assert_eq!(caller.call_as::<Shared>(request()).await.unwrap().value, 7);
                    // Parent is shared; its resource-owner context can call its private child.
                    assert_eq!(caller.call_as::<Parent>(request()).await.unwrap().value, 7);
                    let (requests, mut decoded) =
                        harmos::operations::stream(harmos::operations::StreamSettings {
                            items: NonZeroUsize::new(1).unwrap(),
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        })
                        .unwrap();
                    requests.send_with(4096, || Ok(request())).await.unwrap();
                    let charged = decoded.recv().await.unwrap();
                    assert_eq!(
                        caller
                            .submit_charged::<Shared, _>(charged, ())
                            .unwrap()
                            .await
                            .unwrap()
                            .value,
                        7
                    );
                    drop(requests.reserve(4096).await.unwrap());
                    #[cfg(feature = "grpc")]
                    {
                        let response = grpc_call::<super::wire::Echo>(
                            harmos::grpc::server::Authority::Plugin(caller.clone()),
                        )
                        .await;
                        use http_body_util::BodyExt as _;
                        let body = response.into_body().collect().await.unwrap();
                        assert_eq!(body.trailers().unwrap()["grpc-status"], "0");
                        let bytes = body.to_bytes();
                        assert_eq!(bytes[0], 0);
                        assert_eq!(
                            u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize,
                            bytes.len() - 5
                        );
                        assert_eq!(
                            <String as harmos::Protobuf>::decode(&bytes[5..]).unwrap(),
                            "grpc:1"
                        );
                        let response = grpc_call::<super::wire::PrivateEcho>(
                            harmos::grpc::server::Authority::Plugin(caller.clone()),
                        )
                        .await;
                        assert_eq!(response.headers()["grpc-status"], "7");
                    }
                    let prepared = host
                        .owner
                        .prepared
                        .load(std::sync::atomic::Ordering::SeqCst);
                    for source in ["client", "transitive", "unrelated"] {
                        let source = runtime.operations.caller(&host.callers[source]).unwrap();
                        assert!(matches!(
                            source.submit_as::<Compute>(request()),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                        let (output, input) =
                            harmos::operations::stream::<u32>(harmos::operations::StreamSettings {
                                items: NonZeroUsize::new(1).unwrap(),
                                bytes: NonZeroUsize::new(4).unwrap(),
                            })
                            .unwrap();
                        assert!(matches!(
                            source.submit_endpoints::<Compute, _>(request(), input),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                        assert!(output.is_closed());
                        requests.send_with(4096, || Ok(request())).await.unwrap();
                        let charged = decoded.recv().await.unwrap();
                        assert!(matches!(
                            source.submit_charged::<Compute, _>(charged, ()),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                        drop(requests.reserve(4096).await.unwrap());
                    }
                    for source in ["transitive", "unrelated"] {
                        let source = runtime.operations.caller(&host.callers[source]).unwrap();
                        assert!(matches!(
                            source.submit_as::<Shared>(request()),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                    }
                    assert_eq!(
                        host.owner
                            .prepared
                            .load(std::sync::atomic::Ordering::SeqCst),
                        prepared
                    );
                    assert_eq!(
                        runtime
                            .operations
                            .caller(&host.callers["fixture"])
                            .unwrap()
                            .call_as::<Compute>(request())
                            .await
                            .unwrap()
                            .value,
                        7
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(matches!(
                        caller.submit_as::<Shared>(request()),
                        Err(Error::OperationCaller { .. })
                    ));
                    let (output, input) =
                        harmos::operations::stream::<u32>(harmos::operations::StreamSettings {
                            items: NonZeroUsize::new(1).unwrap(),
                            bytes: NonZeroUsize::new(4).unwrap(),
                        })
                        .unwrap();
                    assert!(matches!(
                        caller.submit_endpoints::<Shared, _>(request(), input),
                        Err(Error::OperationCaller { .. })
                    ));
                    assert!(output.is_closed());
                    requests.send_with(4096, || Ok(request())).await.unwrap();
                    let charged = decoded.recv().await.unwrap();
                    assert!(matches!(
                        caller.submit_charged::<Shared, _>(charged, ()),
                        Err(Error::OperationCaller { .. })
                    ));
                    drop(requests.reserve(4096).await.unwrap());
                    #[cfg(feature = "grpc")]
                    {
                        let response = grpc_call::<super::wire::Echo>(
                            harmos::grpc::server::Authority::Plugin(caller.clone()),
                        )
                        .await;
                        assert_eq!(response.headers()["grpc-status"], "16");
                    }
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("operation_cache_partitioned", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(with_clients(true)).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let caller = runtime.operations.caller(&host.callers["client"]).unwrap();
                    let keyed = || Request {
                        keyed: true,
                        ..request()
                    };
                    let host_response =
                        runtime.operations.call_as::<Shared>(keyed()).await.unwrap();
                    let first = caller.call_as::<Shared>(keyed()).await.unwrap();
                    let again = caller.call_as::<Shared>(keyed()).await.unwrap();
                    assert!(!host_response.ptr_eq(&first));
                    assert!(first.ptr_eq(&again));
                    assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 2);

                    // Only the client generation changes; the provider binding/cache stays live.
                    let mut next = with_clients(true)
                        .definitions()
                        .cloned()
                        .collect::<Vec<_>>();
                    next.iter_mut()
                        .find(|definition| definition.id == "client")
                        .unwrap()
                        .configuration = br#"{"revision":2}"#.to_vec();
                    manager
                        .reload(PluginDefinitions::new(next, []).unwrap(), &mut host)
                        .await
                        .unwrap();
                    assert!(matches!(
                        caller.submit_as::<Shared>(keyed()),
                        Err(Error::OperationCaller { .. })
                    ));
                    let replacement = runtime.operations.caller(&host.callers["client"]).unwrap();
                    let next = replacement.call_as::<Shared>(keyed()).await.unwrap();
                    assert!(!first.ptr_eq(&next));
                    assert!(
                        host_response
                            .ptr_eq(&runtime.operations.call_as::<Shared>(keyed()).await.unwrap())
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("requirement_revokes_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(with_clients(true)).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let old = runtime.operations.caller(&host.callers["client"]).unwrap();
                    manager
                        .reload(with_clients(false), &mut host)
                        .await
                        .unwrap();
                    assert!(matches!(
                        old.submit_as::<Shared>(request()),
                        Err(Error::OperationCaller { .. })
                    ));
                    let replacement = runtime.operations.caller(&host.callers["client"]).unwrap();
                    assert!(matches!(
                        replacement.submit_as::<Shared>(request()),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Shared>(request())
                            .await
                            .unwrap()
                            .value,
                        7
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("queued_operations_recheck", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(with_clients(true)).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let source = runtime.operations.caller(&host.callers["client"]).unwrap();
                    let opened = Arc::new(tokio::sync::Notify::new());
                    let gate = Arc::new(tokio::sync::Notify::new());
                    let held = runtime
                        .operations
                        .submit_as::<Shared>(Request {
                            gate: Some((opened.clone(), gate.clone())),
                            ..request()
                        })
                        .unwrap();
                    tokio::time::timeout(std::time::Duration::from_secs(2), opened.notified())
                        .await
                        .unwrap();
                    let queued = source.submit_as::<Shared>(request()).unwrap();
                    manager
                        .reload(with_clients(false), &mut host)
                        .await
                        .unwrap();

                    gate.notify_one();
                    held.await.unwrap();
                    assert!(matches!(queued.await, Err(Error::OperationCaller { .. })));
                    assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 1);
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("instances_lacks_authority", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let router = Router::new(candidate().graph().clone());
                    let endpoint = router.enter("fixture").unwrap();
                    assert!(endpoint.scope().is_none());
                    assert!(matches!(
                        runtime.operations.caller(&endpoint.caller()),
                        Err(Error::OperationCaller { .. })
                    ));
                    drop(endpoint);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("calls_retain_permissions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use super::wire::{Echo, Nested, PrivateEcho};
                    use harmos::Message;
                    let runtime = runtime().await;
                    let mut manager = PluginManager::new(with_clients(true)).unwrap();
                    let mut host = host(runtime.operations.clone());
                    manager.initialize(&mut host).await.unwrap();
                    let client = runtime.operations.caller(&host.callers["client"]).unwrap();
                    let owner = runtime.operations.caller(&host.callers["fixture"]).unwrap();
                    assert_eq!(
                        String::decode(
                            &client
                                .call_encoded(Echo::ID, Text::from("hello").encode_to_vec())
                                .await
                                .unwrap()
                        )
                        .unwrap(),
                        "hello:1"
                    );
                    assert_eq!(
                        String::decode(
                            &owner
                                .call_encoded(PrivateEcho::ID, Text::from("own").encode_to_vec())
                                .await
                                .unwrap()
                        )
                        .unwrap(),
                        "own:1"
                    );
                    // The client's shared parent invokes its own private child as the bound owner.
                    assert_eq!(
                        &*client.call_as::<Nested>("nested".into()).await.unwrap(),
                        "nested:1"
                    );
                    let prepared = host
                        .owner
                        .prepared
                        .load(std::sync::atomic::Ordering::SeqCst);
                    // Malformed payloads must not replace authorization failures with codec errors.
                    assert!(matches!(
                        client.submit_encoded(PrivateEcho::ID, vec![0xff]),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    for source in ["transitive", "unrelated"] {
                        let source = runtime.operations.caller(&host.callers[source]).unwrap();
                        assert!(matches!(
                            source.submit_encoded(Echo::ID, vec![0xff]),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                    }
                    assert_eq!(
                        host.owner
                            .prepared
                            .load(std::sync::atomic::Ordering::SeqCst),
                        prepared
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(matches!(
                        client.submit_encoded(Echo::ID, Vec::new()),
                        Err(Error::OperationCaller { .. })
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("staging_refuses_metadata", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for change in 0..3 {
                        let runtime = runtime().await;
                        let mut definition = candidate().definitions().next().unwrap().clone();
                        match change {
                            0 => {
                                definition.contributions.operations.pop();
                            }
                            1 => {
                                definition.contributions.operations[0].policy.timeout +=
                                    std::time::Duration::from_secs(1);
                            }
                            _ => {
                                definition.contributions.operations[0].visibility =
                                    harmos::operations::Visibility::Shared;
                            }
                        }
                        let mut manager =
                            PluginManager::new(PluginDefinitions::new([definition], []).unwrap())
                                .unwrap();
                        let mut host = host(runtime.operations.clone());
                        assert!(manager.initialize(&mut host).await.is_err());
                        assert!(runtime.operations.definitions().is_empty());
                        assert_eq!(
                            host.owner
                                .prepared
                                .load(std::sync::atomic::Ordering::SeqCst),
                            0
                        );
                        assert_eq!(host.owner.runs.load(std::sync::atomic::Ordering::SeqCst), 0);
                        manager.shutdown(&mut host).await.unwrap();
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("commit_requires_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for missing in 0..3 {
                        let runtime = runtime().await;
                        let mut manager = PluginManager::new(candidate()).unwrap();
                        let mut host = host(runtime.operations.clone());
                        host.skip_operations = missing == 0;
                        host.drop_before_commit = missing == 1;
                        host.unregister_before_commit = missing == 2;
                        let error = manager.initialize(&mut host).await.unwrap_err();
                        let harmos::Error::PluginLifecycle { source } = &error else {
                            panic!("plugin lifecycle failure is retained: {error:?}");
                        };
                        let harmos::plugin::PluginFailure::Start { source, .. } = source.as_ref()
                        else {
                            panic!("initialization failure is retained: {error:?}");
                        };
                        let harmos::Error::PluginLifecycle { source } = source else {
                            panic!("typed routing cause is retained: {error:?}");
                        };
                        if missing == 1 {
                            assert!(
                                matches!(
                                    source.as_ref(),
                                    harmos::plugin::PluginFailure::Routing(
                                        harmos::plugin::DispatchError::Unavailable
                                    )
                                ),
                                "dropping the original receipt closes staging: {error:?}"
                            );
                        } else {
                            assert!(
                                matches!(source.as_ref(), harmos::plugin::PluginFailure::Routing(
                harmos::plugin::DispatchError::IncompleteOperations { owner }
            ) if owner == "fixture"),
                                "{error:?}"
                            );
                        }
                        assert!(runtime.operations.definitions().is_empty());
                        assert!(!host.scope.as_ref().unwrap().is_ready());
                        manager.shutdown(&mut host).await.unwrap();
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("credit_survives_post", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
    let mut manager = PluginManager::new(candidate_with_cleanup()).unwrap();
    let mut host = host(runtime.operations.clone());
    let cleanup = Arc::new(CleanupOwner::default());
    cleanup.fail.store(true, Ordering::SeqCst);
    cleanup.expire.store(true, Ordering::SeqCst);
    host.cleanup = Some(cleanup.clone());
    manager.initialize(&mut host).await.unwrap();
    let scope = host.scope.as_ref().unwrap();
    let contract = WireDeclaration::of::<Cleanup>().contract().unwrap();
    let target = runtime
        .operations
        .capture_callback(scope, &contract)
        .unwrap();
    let contract = WireDeclaration::of::<super::wire::Echo>()
        .contract()
        .unwrap();
    let echo = runtime
        .operations
        .capture_callback(scope, &contract)
        .unwrap();
    let invocation = target
        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            Ok(Text::from("expire").encode_to_vec())
        })
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        cleanup.finalizing.notified(),
    )
    .await
    .unwrap();
    assert_eq!(cleanup.attempted.load(Ordering::SeqCst), 0);
    let encoded = AtomicBool::new(false);
    let refused = || {
        echo.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            encoded.store(true, Ordering::SeqCst);
            Ok(Text::from("busy").encode_to_vec())
        })
    };
    assert!(matches!(
        refused(),
        Err(Error::ResourceExhausted {
            resource: "operation preparation bytes"
        })
    ));
    assert!(!encoded.load(Ordering::SeqCst));
    cleanup.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(2), cleanup.failed.notified())
        .await
        .unwrap();
    assert!(matches!(
        refused(),
        Err(Error::ResourceExhausted {
            resource: "operation preparation bytes"
        })
    ));
    assert!(!encoded.load(Ordering::SeqCst));
    let outcome = invocation.await;
    assert!(matches!(outcome,
        Err(Error::OperationFinalization { outcome: Some(ref error), .. })
            if matches!(error.as_ref(), Error::OperationRefused(harmos::Refusal::TimedOut))
    ));
    cleanup.fail.store(false, Ordering::SeqCst);
    cleanup.release.notify_one();
    runtime.operations.unregister(Cleanup::ID).await.unwrap();
    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 2);
    let bytes = echo
        .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            Ok(Text::from("settled").encode_to_vec())
        })
        .unwrap()
        .await
        .unwrap();
    assert_eq!(String::decode(&bytes).unwrap(), "settled:1");
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("reload_preserves_callback", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut host = SelectedHost {
                        operations: runtime.operations.clone(),
                        scopes: BTreeMap::new(),
                        callers: BTreeMap::new(),
                    };
                    let mut manager =
                        PluginManager::new(selected_candidate("first", true)).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let contract = WireDeclaration::of::<Selected>().contract().unwrap();
                    let first = runtime
                        .operations
                        .capture_callback(&host.scopes["first"], &contract)
                        .unwrap();
                    let second = runtime
                        .operations
                        .capture_callback(&host.scopes["second"], &contract)
                        .unwrap();
                    let old_client = runtime.operations.caller(&host.callers["client"]).unwrap();
                    assert_eq!(
                        old_client
                            .call_as::<Selected>("before".into())
                            .await
                            .unwrap()
                            .as_str(),
                        "first:before"
                    );
                    assert!(matches!(
                        runtime
                            .operations
                            .call_as::<Selected>("ambiguous".into())
                            .await,
                        Err(Error::InvalidInput {
                            field: "operation provider",
                            ..
                        })
                    ));
                    let first_generation = host.scopes["first"].generation();
                    let second_generation = host.scopes["second"].generation();

                    manager
                        .reload(selected_candidate("second", true), &mut host)
                        .await
                        .unwrap();
                    assert!(matches!(
                        old_client.submit_as::<Selected>("stale".into()),
                        Err(Error::OperationCaller { .. })
                    ));
                    assert_eq!(host.scopes["first"].generation(), first_generation);
                    assert_eq!(host.scopes["second"].generation(), second_generation);
                    let client = runtime.operations.caller(&host.callers["client"]).unwrap();
                    assert_eq!(
                        client
                            .call_as::<Selected>("after".into())
                            .await
                            .unwrap()
                            .as_str(),
                        "second:after"
                    );
                    let first_payload = first
                        .submit_with(NonZeroUsize::new(128).unwrap(), |_| {
                            Ok(Text::from("callback").encode_to_vec())
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&first_payload).unwrap(), "first:callback");

                    manager
                        .reload(selected_candidate("second", false), &mut host)
                        .await
                        .unwrap();
                    assert!(matches!(
                        first.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from(
                            "retired"
                        )
                        .encode_to_vec())),
                        Err(Error::Closing { .. })
                    ));
                    let second_payload = second
                        .submit_with(NonZeroUsize::new(128).unwrap(), |_| {
                            Ok(Text::from("surviving").encode_to_vec())
                        })
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&second_payload).unwrap(), "second:surviving");
                    assert_eq!(host.scopes["second"].generation(), second_generation);
                    assert_eq!(
                        client
                            .call_as::<Selected>("unchanged".into())
                            .await
                            .unwrap()
                            .as_str(),
                        "second:unchanged"
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(matches!(
                        second.submit_with(NonZeroUsize::new(128).unwrap(), |_| Ok(Text::from(
                            "closed"
                        )
                        .encode_to_vec())),
                        Err(Error::Closing { .. })
                    ));
                    drop((first, second, client, old_client, manager, host));
                    runtime.stop().await.unwrap();
                });
        });
    }
}

#[cfg(feature = "grpc")]
async fn grpc_call<O>(
    authority: harmos::grpc::server::Authority,
) -> tonic::codegen::http::Response<tonic::body::Body>
where
    O: OperationDeclaration<Request = Text, Response = String>,
{
    use tonic::codec::Codec as _;
    let capacity = NonZeroUsize::new(4096).unwrap();
    let decode = harmos::grpc::DecodeSettings {
        message_bytes: capacity,
        retained_bytes: capacity,
    };
    let encode = harmos::grpc::EncodeSettings {
        message_bytes: capacity,
        buffer_bytes: capacity,
    };
    let channel = harmos::operations::StreamSettings {
        items: NonZeroUsize::new(1).unwrap(),
        bytes: capacity,
    };
    let settings = harmos::grpc::server::Settings {
        calls: NonZeroUsize::new(8).unwrap(),
        decode,
        encode,
        input: channel,
        output: channel,
        opening_timeout: std::time::Duration::from_secs(1),
    };
    let mut codec =
        harmos::grpc::Codec::<Text, harmos::operations::Response<Text>>::new(decode, encode)
            .unwrap();
    let message =
        futures_util::stream::iter([Ok(harmos::operations::Response::new(Text::from("grpc")))]);
    let body = tonic::codec::EncodeBody::new_client(codec.encoder(), message, None, Some(4096));
    let request = tonic::codegen::http::Request::builder()
        .header("content-type", "application/grpc")
        .body(tonic::body::Body::new(body))
        .unwrap();
    harmos::grpc::server::unary::<O>(authority, settings, request).await
}

/* === Tests === */
