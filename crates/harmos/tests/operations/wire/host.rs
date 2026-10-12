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
use harmos::operations::{
    Context, Operation, OperationDeclaration, OperationPolicy, PluginRegistration, Visibility,
    WireDeclaration, WireOperation,
};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition,
    PluginDefinitions, PluginEndpoint, PluginLifecycle, PluginLifecycleAuthority, PluginManager,
    PluginScope, PluginTransition, Requirement, Router,
};
use harmos::{Error, Message, Result, operations::OperationManager};
use harmos_contract::inventory::contracts::Required;
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<Resources, Resources, Resources>;

/* === Structs === */
struct Control;

struct Principal(&'static str);

#[derive(Default)]
struct CoreOwner {
    prepared: AtomicUsize,
    called: AtomicUsize,
}

#[derive(Default)]
struct Owner {
    prepared: AtomicUsize,
}

#[derive(Default)]
struct Resources {
    endpoint: Option<PluginEndpoint>,
    registration: Option<PluginRegistration>,
}

struct Host {
    operations: OperationManager,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    callers: BTreeMap<String, PluginCaller>,
}

/* === Enums === */
#[harmos::catalog]
enum Controls {
    Control(Control),
}

/* === Traits === */

/* === Implementations === */

struct CorePublication(Arc<CoreOwner>);

struct Surveying(Arc<CoreOwner>);

impl harmos::operations::Operation for Surveying {
    type Declaration = harmos::operations::host::Survey;
    type Resources = CoreOwner;
    type Error = super::Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(owner: Arc<CoreOwner>) -> std::result::Result<Self, Self::Error> {
        Ok(Self(owner))
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut harmos::operations::host::SurveyRequest,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::host::SurveyResponse, Self::Error> {
        self.0.called.fetch_add(1, Ordering::SeqCst);
        Ok(harmos::operations::host::SurveyResponse::default())
    }
}

impl harmos::operations::Operation for CorePublication {
    type Declaration = harmos::operations::host::Publish;
    type Resources = CoreOwner;
    type Error = super::Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(owner: Arc<CoreOwner>) -> std::result::Result<Self, Self::Error> {
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self(owner))
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        _request: &mut harmos::operations::host::Publication,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<(), Self::Error> {
        assert_eq!(context.context().require::<Principal>().unwrap().0, "alice");
        assert!(!context.is_host_call());
        assert_eq!(context.caller_scope().unwrap().owner(), "client");
        assert!(matches!(
            context.submit::<Control>("child".into()),
            Err(Error::OperationUnauthorized { .. })
        ));
        self.0.called.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl OperationDeclaration for Control {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.host_control";
    const POLICY: OperationPolicy = super::wire::Echo::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
}

impl WireOperation for Control {
    const VERSION: u32 = 1;
}

#[harmos::catalog]
enum ImplementedControls {
    Control(Controlling),
}

struct Controlling {
    _resources: Arc<Owner>,
}

impl Operation for Controlling {
    type Declaration = Control;
    type Resources = Owner;
    type Error = super::Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(resources: Arc<Owner>) -> std::result::Result<Self, Self::Error> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<String, Self::Error> {
        Ok(request.value.clone())
    }
}

impl PluginLifecycle for Host {
    type Guest = Resources;
    type Sidecar = Resources;
    type Embedded = Resources;
    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, definition: &PluginDefinition) -> Result<Backends> {
        Ok(match definition.kind() {
            harmos::plugin::PluginKind::Guest => PluginBackend::Guest(Resources::default()),
            harmos::plugin::PluginKind::Sidecar => PluginBackend::Sidecar(Resources::default()),
            harmos::plugin::PluginKind::Embedded => PluginBackend::Embedded(Resources::default()),
        })
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources)
            | PluginBackend::Sidecar(resources)
            | PluginBackend::Embedded(resources) => resources,
        };
        let endpoint = router.enter(&definition.id).unwrap();
        self.callers
            .insert(definition.id.clone(), endpoint.caller());
        if definition.id == "provider" {
            let scope = endpoint.scope().unwrap();
            self.scope = Some(scope.clone());
            resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                bindings.register_wire_operations::<ImplementedControls, _>(
                    super::Lanes::Open,
                    self.owner.clone(),
                )
            })?);
            assert!(matches!(
                self.operations.submit::<Control>("staged".into()),
                Err(Error::NotReady { .. })
            ));
            assert!(matches!(
                self.operations
                    .submit_encoded(Control::ID, Text::from("staged").encode_to_vec()),
                Err(Error::NotReady { .. })
            ));
            assert_eq!(self.owner.prepared.load(Ordering::SeqCst), 0);
        }
        resources.endpoint = Some(endpoint);
        Ok(())
    }
    async fn stop(
        _: &PluginDefinition,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources)
            | PluginBackend::Sidecar(resources)
            | PluginBackend::Embedded(resources) => resources,
        };
        if let Some(registration) = resources.registration.as_mut() {
            registration.shutdown().await?;
        }
        resources.registration.take();
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn definitions(revision: &str) -> PluginDefinitions {
    let provider = PluginDefinition {
        id: "provider".into(),
        version: "1.0.0".parse().unwrap(),
        artifact: PluginArtifact::Embedded {
            revision: revision.into(),
        },
        requires: Vec::new(),
        configuration: b"null".to_vec(),
        contributions: PluginContributions {
            operations: harmos::operations::Declarations::of::<Controls>().unwrap(),
            ..PluginContributions::default()
        },
    };
    let client = PluginDefinition {
        id: "client".into(),
        requires: vec![Requirement {
            id: "provider".into(),
            version: "*".parse().unwrap(),
        }],
        contributions: PluginContributions::default(),
        ..provider.clone()
    };
    PluginDefinitions::new([provider, client], []).unwrap()
}

fn required(contract: harmos::operations::Contract, grants: Vec<Grant>) -> Required {
    Required { contract, grants }
}

fn requiring(required: Vec<Required>) -> PluginDefinitions {
    let mut declared: Vec<_> = definitions("first").definitions().cloned().collect();
    let client = declared
        .iter_mut()
        .find(|definition| definition.id == "client")
        .unwrap();
    client.artifact = PluginArtifact::Guest { sha256: [1; 32] };
    client.contributions.required_contracts = Some(required);
    PluginDefinitions::new(declared, []).unwrap()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("preflight_preserves_plugin", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::operations::host::{Publication, Publish};
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(4096).unwrap()).await;
                    let owner = Arc::new(CoreOwner::default());
                    runtime
                        .operations
                        .register_wire_operation::<CorePublication>(
                            "host",
                            super::Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    let mut host = Host {
                        operations: runtime.operations.clone(),
                        owner: Arc::new(Owner::default()),
                        scope: None,
                        callers: BTreeMap::new(),
                    };

                    // A Requirement The Host Does Not Provide Exactly Is Refused Before Preparation
                    let selected = WireDeclaration::of::<Publish>().contract().unwrap();
                    let mut drift = selected.operation().clone();
                    drift.policy.timeout = std::time::Duration::from_secs(10);
                    let drift = harmos::operations::Contract::new(
                        drift,
                        selected.version(),
                        selected.codec(),
                        selected.request().to_vec(),
                        selected.response().to_vec(),
                    )
                    .unwrap();
                    let mut refused =
                        PluginManager::new(requiring(vec![required(drift, vec![])])).unwrap();
                    assert!(matches!(
                        refused.initialize(&mut host).await,
                        Err(Error::NotRegistered {
                            domain: "host provider",
                            ..
                        })
                    ));
                    assert!(host.callers.is_empty());

                    // The Exact Requirement Routes To The Host Provider For The Original Caller
                    let chosen = requiring(vec![required(selected.clone(), vec![])]);
                    let mut manager = PluginManager::new(chosen.clone()).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let provider = runtime
                        .operations
                        .caller(&host.callers["provider"])
                        .unwrap();
                    assert!(matches!(
                        provider.submit::<Publish>(Publication::default()),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    let mut context = harmos::operations::CallContext::default();
                    context.insert(Principal("alice")).unwrap();
                    let caller = runtime
                        .operations
                        .with_context(context)
                        .caller(&host.callers["client"])
                        .unwrap();
                    caller
                        .submit::<Publish>(Publication::default())
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(owner.called.load(Ordering::SeqCst), 1);
                    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 0);

                    // Two Host Providers Make The Requirement Ambiguous
                    runtime
                        .operations
                        .register_wire_operation::<CorePublication>(
                            "other-host",
                            super::Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    assert!(matches!(
                        runtime.operations.validate_requirements(&chosen),
                        Err(Error::InvalidInput {
                            field: "host provider",
                            ..
                        })
                    ));
                    runtime
                        .operations
                        .unregister_from("other-host", Publish::ID)
                        .await
                        .unwrap();
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(matches!(
                        caller.submit::<Publish>(Publication::default()),
                        Err(Error::OperationCaller { .. })
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("grants_select_request", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::operations::host::{Survey, SurveyRequest};
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(4096).unwrap()).await;
                    let owner = Arc::new(CoreOwner::default());
                    runtime
                        .operations
                        .register_wire_operation::<Surveying>(
                            "host",
                            super::Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    let mut host = Host {
                        operations: runtime.operations.clone(),
                        owner: Arc::new(Owner::default()),
                        scope: None,
                        callers: BTreeMap::new(),
                    };
                    let contract = WireDeclaration::of::<Survey>().contract().unwrap();
                    let candidate = requiring(vec![required(
                        contract,
                        vec![Grant::Survey("reading".into())],
                    )]);
                    let mut manager = PluginManager::new(candidate).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let caller = runtime.operations.caller(&host.callers["client"]).unwrap();
                    let survey = |stream: &str| SurveyRequest {
                        stream: stream.into(),
                        ..SurveyRequest::default()
                    };
                    caller
                        .submit::<Survey>(survey("reading"))
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(matches!(
                        caller.submit::<Survey>(survey("power")),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert_eq!(owner.called.load(Ordering::SeqCst), 1);
                    // The Host Itself Needs No Grant
                    runtime
                        .operations
                        .submit::<Survey>(survey("power"))
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(owner.called.load(Ordering::SeqCst), 2);
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("controls_retain_ready", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime =
                        super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
                    let mut host = Host {
                        operations: runtime.operations.clone(),
                        owner: Arc::new(Owner::default()),
                        scope: None,
                        callers: BTreeMap::new(),
                    };
                    let mut manager = PluginManager::new(definitions("first")).unwrap();
                    manager.initialize(&mut host).await.unwrap();
                    let scope = host.scope.as_ref().unwrap().clone();
                    let own = runtime
                        .operations
                        .caller(&host.callers["provider"])
                        .unwrap();
                    let dependency = runtime.operations.caller(&host.callers["client"]).unwrap();
                    for caller in [&own, &dependency] {
                        assert!(matches!(
                            caller.submit::<Control>("plugin".into()),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                        assert!(matches!(
                            caller
                                .submit_encoded(Control::ID, Text::from("encoded").encode_to_vec()),
                            Err(Error::OperationUnauthorized { .. })
                        ));
                    }
                    let contract = WireDeclaration::of::<Control>().contract().unwrap();
                    let captured = runtime
                        .operations
                        .capture_callback(&scope, &contract)
                        .unwrap();
                    let encoded = AtomicBool::new(false);
                    assert!(matches!(
                        captured.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                            encoded.store(true, Ordering::SeqCst);
                            Ok(Text::from("callback").encode_to_vec())
                        }),
                        Err(Error::OperationUnauthorized { .. })
                    ));
                    assert!(!encoded.load(Ordering::SeqCst));
                    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(
                        *runtime
                            .operations
                            .call::<Control>("host".into())
                            .await
                            .unwrap(),
                        "host"
                    );

                    let response = runtime
                        .operations
                        .call_encoded(Control::ID, Text::from("encoded host").encode_to_vec())
                        .await
                        .unwrap();
                    assert_eq!(String::decode(&response).unwrap(), "encoded host");

                    // Revoke The Exact Managed Generation Before Its Replacement Becomes Ready
                    host.owner.prepared.store(0, Ordering::SeqCst);
                    manager
                        .reload(definitions("second"), &mut host)
                        .await
                        .unwrap();
                    assert_ne!(
                        scope.generation(),
                        host.scope.as_ref().unwrap().generation()
                    );
                    assert!(matches!(
                        own.submit::<Control>("stale".into()),
                        Err(Error::OperationCaller { .. })
                    ));
                    assert!(
                        captured
                            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                                encoded.store(true, Ordering::SeqCst);
                                Ok(Text::from("stale").encode_to_vec())
                            })
                            .is_err()
                    );
                    assert!(!encoded.load(Ordering::SeqCst));
                    assert_eq!(
                        *runtime
                            .operations
                            .call::<Control>("replacement".into())
                            .await
                            .unwrap(),
                        "replacement"
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(
                        runtime
                            .operations
                            .submit::<Control>("closed".into())
                            .is_err()
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
