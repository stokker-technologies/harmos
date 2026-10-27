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
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// Third Party Imports

// Local Imports
use super::cases::Text;
use harmos_domain::inventory::contracts::Required;
use harmos_domain::plugin::Grant;
use harmos_runtime::operations::{Handler, OperationDeclaration, PluginRegistration, WireDeclaration};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginCaller, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginScope, PluginTransition, Requirement, Router,
};
use harmos_runtime::{Error, Message, Result, operations::OperationManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources, Resources>;

/* === Structs === */

#[harmos_runtime::operation(id = "fixture_host_control",
    response = String,
    visibility = Host,
    policy = super::cases::EchoDeclaration::POLICY
)]
struct ControlDeclaration {
    value: String,
}

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
    operations: OperationManager<super::Lanes>,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    callers: BTreeMap<String, PluginCaller>,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Controls {
    ControlDeclaration(ControlDeclaration),
}

/* === Traits === */

/* === Implementations === */

struct CorePublication(Arc<CoreOwner>);

impl harmos_runtime::operations::Handler for CorePublication {
    type Request = <harmos_operations::streams::Publish as harmos_runtime::OperationDeclaration>::Request;
    type Response = <harmos_operations::streams::Publish as harmos_runtime::OperationDeclaration>::Response;

    type Error = super::Failed;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> std::result::Result<Self, Self::Error> {
        let owner = context.resource::<CoreOwner>().expect("fixture dependency");
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self(owner))
    }
    async fn run(
        &mut self, context: harmos_runtime::OperationContext, _request: &mut harmos_operations::streams::Publish,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            assert_eq!(context.context().require::<Principal>().unwrap().0, "alice");
            assert!(!context.is_host_call());
            assert_eq!(context.caller_scope().unwrap().owner(), "client");
            assert!(matches!(
                context.submit::<ControlDeclaration>(ControlDeclaration::from("child")),
                Err(Error::OperationUnauthorized { .. })
            ));
            self.0.called.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl From<&str> for ControlDeclaration {
    fn from(value: &str) -> Self {
        Self { value: value.to_owned() }
    }
}

struct Controlling {
    _resources: Arc<Owner>,
}

impl Handler for Controlling {
    type Request = ControlDeclaration;
    type Response = <ControlDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = super::Failed;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> std::result::Result<Self, Self::Error> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _resources: resources })
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut ControlDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<String>, Self::Error> {
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(request.value.clone()) }.await;
        result.map(::core::convert::Into::into)
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
            harmos_runtime::plugin::PluginKind::Guest => PluginBackend::Guest(Resources::default()),
            harmos_runtime::plugin::PluginKind::Sidecar => PluginBackend::Sidecar(Resources::default()),
            harmos_runtime::plugin::PluginKind::Embedded => PluginBackend::Embedded(Resources::default()),
        })
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources) | PluginBackend::Sidecar(resources) | PluginBackend::Embedded(resources) => resources,
        };
        let endpoint = router.enter(&definition.id).unwrap();
        self.callers.insert(definition.id.clone(), endpoint.caller());
        if definition.id == "provider" {
            let scope = endpoint.scope().unwrap();
            self.scope = Some(scope.clone());
            resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                bindings.register(harmos_runtime::RegistrationExt::lane(
                    harmos_runtime::RegistrationBatch::new({
                        let mut dependencies = harmos_runtime::Dependencies::new();
                        dependencies.insert(self.owner.clone()).unwrap();
                        dependencies
                    })
                    .implementation::<ControlDeclaration, Controlling>(),
                    super::Lanes::Open,
                ))
            })?);
            assert!(matches!(
                self.operations.submit::<ControlDeclaration>(ControlDeclaration::from("staged")),
                Err(Error::NotReady { .. })
            ));
            assert!(matches!(
                self.operations.submit_encoded(&format!("provider.{}", ControlDeclaration::ID), Text::from("staged").encode()),
                Err(Error::NotReady { .. })
            ));
            assert_eq!(self.owner.prepared.load(Ordering::SeqCst), 0);
        }
        resources.endpoint = Some(endpoint);
        Ok(())
    }
    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        let resources = match backend {
            PluginBackend::Guest(resources) | PluginBackend::Sidecar(resources) | PluginBackend::Embedded(resources) => resources,
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
        artifact: PluginArtifact::Embedded { revision: revision.into() },
        requires: Vec::new(),
        configuration: b"null".to_vec(),
        contributions: PluginContributions {
            operations: harmos_runtime::operations::Declarations::of::<Controls>().unwrap(),
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

fn required(contract: harmos_runtime::operations::Contract, grants: Vec<Grant>) -> Required {
    Required { contract, grants }
}

fn requiring(required: Vec<Required>) -> PluginDefinitions {
    let mut declared: Vec<_> = definitions("first").definitions().cloned().collect();
    let client = declared.iter_mut().find(|definition| definition.id == "client").unwrap();
    client.artifact = PluginArtifact::Guest { sha256: [1; 32] };
    client.contributions.required_contracts = Some(required);
    PluginDefinitions::new(declared, []).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn host_requirements_route() {
    use harmos_operations::streams::Publish;
    let runtime = super::runtime_with_preparation(NonZeroUsize::new(4096).unwrap()).await;
    let owner = Arc::new(CoreOwner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<Publish, CorePublication>(),
            super::Lanes::Open,
        ))
        .await
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
    let drift =
        harmos_runtime::operations::Contract::new(drift, selected.fingerprint(), selected.codec(), selected.schema().clone()).unwrap();
    let mut refused = PluginManager::new(requiring(vec![required(drift, vec![])])).unwrap();
    assert!(matches!(refused.initialize(&mut host).await, Err(Error::NotRegistered { domain: "host provider", .. })));
    assert!(host.callers.is_empty());

    // The Exact Requirement Routes To The Host Provider For The Original Caller
    let chosen = requiring(vec![required(selected.clone(), vec![])]);
    let mut manager = PluginManager::new(chosen.clone()).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let provider = runtime.operations.caller(&host.callers["provider"]).unwrap();
    assert!(matches!(provider.submit::<Publish>(harmos_operations::streams::Publish::default()), Err(Error::OperationUnauthorized { .. })));
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal("alice")).unwrap();
    let caller = runtime.operations.with_context(context).caller(&host.callers["client"]).unwrap();
    caller.submit::<Publish>(harmos_operations::streams::Publish::default()).unwrap().await.unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.called.load(Ordering::SeqCst), 1);
    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 0);

    // Root registration cannot manufacture another host owner.
    assert!(matches!(
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(owner.clone()).unwrap();
                    dependencies
                })
                .implementation::<Publish, CorePublication>(),
                super::Lanes::Open
            ))
            .await,
        Err(Error::AlreadyRegistered { .. })
    ));
    runtime.operations.validate_requirements(&chosen).unwrap();
    manager.shutdown(&mut host).await.unwrap();
    assert!(matches!(caller.submit::<Publish>(harmos_operations::streams::Publish::default()), Err(Error::OperationCaller { .. })));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn host_visibility_enforced() {
    let runtime = super::runtime_with_preparation(NonZeroUsize::new(128).unwrap()).await;
    let mut host = Host {
        operations: runtime.operations.clone(),
        owner: Arc::new(Owner::default()),
        scope: None,
        callers: BTreeMap::new(),
    };
    let mut manager = PluginManager::new(definitions("first")).unwrap();
    manager.initialize(&mut host).await.unwrap();
    let scope = host.scope.as_ref().unwrap().clone();
    let own = runtime.operations.caller(&host.callers["provider"]).unwrap();
    let dependency = runtime.operations.caller(&host.callers["client"]).unwrap();
    for caller in [&own, &dependency] {
        assert!(matches!(
            caller.submit::<ControlDeclaration>(ControlDeclaration::from("plugin")),
            Err(Error::OperationUnauthorized { .. })
        ));
        assert!(matches!(
            caller.submit_encoded(&format!("provider.{}", ControlDeclaration::ID), Text::from("encoded").encode()),
            Err(Error::OperationUnauthorized { .. })
        ));
    }
    let contract = WireDeclaration::of::<ControlDeclaration>().contract().unwrap();
    let captured = runtime.operations.capture_callback(&scope, &contract).unwrap();
    let encoded = AtomicBool::new(false);
    assert!(matches!(
        captured.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            encoded.store(true, Ordering::SeqCst);
            Ok(Text::from("callback").encode())
        }),
        Err(Error::OperationUnauthorized { .. })
    ));
    assert!(!encoded.load(Ordering::SeqCst));
    assert_eq!(host.owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(*runtime.operation(ControlDeclaration::from("host")).run().await.unwrap(), "host");

    let response = runtime
        .operations
        .call_encoded(&format!("provider.{}", ControlDeclaration::ID), Text::from("encoded host").encode())
        .await
        .unwrap();
    assert_eq!(String::decode(&response).unwrap(), "encoded host");

    // Revoke The Exact Managed Generation Before Its Replacement Becomes Ready
    host.owner.prepared.store(0, Ordering::SeqCst);
    manager.reload(definitions("second"), &mut host).await.unwrap();
    assert_ne!(scope.generation(), host.scope.as_ref().unwrap().generation());
    assert!(matches!(own.submit::<ControlDeclaration>(ControlDeclaration::from("stale")), Err(Error::OperationCaller { .. })));
    assert!(
        captured
            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                encoded.store(true, Ordering::SeqCst);
                Ok(Text::from("stale").encode())
            })
            .is_err()
    );
    assert!(!encoded.load(Ordering::SeqCst));
    assert_eq!(*runtime.operation(ControlDeclaration::from("replacement")).run().await.unwrap(), "replacement");
    manager.shutdown(&mut host).await.unwrap();
    assert!(runtime.operation(ControlDeclaration::from("closed")).start().await.is_err());
    runtime.stop().await.unwrap();
}
