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
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
// Local Imports
use harmos::operations::{
    Context, Operation, OperationDeclaration, OperationPolicy, PluginRegistration, RestartPolicy,
    RetryPolicy, Visibility, WireDeclaration, WireOperation,
};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions,
    PluginEndpoint, PluginLifecycle, PluginLifecycleAuthority, PluginManager, PluginScope,
    PluginTransition, Router,
};
use harmos::records::PluginListenerRegistration;
use harmos::schema::{Described, Field};
use harmos::{Error, Message, MessageError, Result, Runtime, state::Position};
use harmos_domain::listeners::{Context as DeliveryContext, Delivery, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<Resources, Resources, Resources>;
type Seen = Vec<(bool, u32, Position)>;

/* === Structs === */
// Neither The Original Native Record Nor Its Decoded Callback Copy Is Clone.
#[harmos::record(id = "listener.fact", version = 1)]
#[derive(Serialize, Deserialize)]
struct Fact {
    value: u32,
    #[serde(skip)]
    dropped: Arc<AtomicUsize>,
}

#[harmos::message]
struct WireFact {
    value: u32,
}

struct Model;

struct Heard<const FIRST: bool>;

struct Owner {
    records: harmos::records::RecordManager<Model>,
    seen: Mutex<Seen>,
    observed: Notify,
    started: Notify,
    resume: Notify,
    finalizing: Notify,
    release: Notify,
    finalizations: AtomicUsize,
    fail_cleanup: AtomicBool,
    fail_attempt: AtomicBool,
    block: bool,
    reenter: bool,
}

#[derive(Default)]
struct Resources {
    owner: Option<Arc<Owner>>,
    endpoint: Option<PluginEndpoint>,
    operations: Option<PluginRegistration>,
    listeners: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

struct Host {
    operations: harmos::operations::OperationManager,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    fail_start: bool,
    skip_sources: bool,
    drop_before_commit: bool,
    original_drop: Arc<AtomicUsize>,
    view: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

/* === Enums === */
#[harmos::records]
enum Rows {
    Fact(Fact),
}

#[harmos::catalog]
enum Callbacks {
    First(Heard<true>),
    Second(Heard<false>),
}

#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
}

#[derive(Debug, thiserror::Error)]
enum Failed {
    #[error("scripted original callback failure")]
    Scripted,
    #[error(transparent)]
    Runtime(#[from] Error),
}

/* === Traits === */

/* === Implementations === */
impl Drop for Fact {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

impl Described for Fact {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Fact));
    const FIELDS: &'static [Field] = WireFact::FIELDS;
    const MARKER: &'static [u8] = &harmos::schema::declaration::encode::<
        { harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Message for Fact {
    fn encode_to_vec(&self) -> Vec<u8> {
        WireFact { value: self.value }.encode_to_vec()
    }
    fn encoded_len(&self) -> usize {
        assert_ne!(self.value, u32::MAX, "scripted native source codec panic");
        WireFact { value: self.value }.encoded_len()
    }
    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        Ok(Self {
            value: WireFact::decode(bytes)?.value,
            dropped: Arc::new(AtomicUsize::new(0)),
        })
    }
}

impl harmos::Application for Model {
    type State = ();
    type Transaction = harmos::state::Never;
    type Record = Rows;
    type Streams = ();
    type Resources = ();
}

impl<const FIRST: bool> OperationDeclaration for Heard<FIRST> {
    type Request = Delivery<WireFact>;
    type Response = ();
    const ID: &'static str = if FIRST {
        "listener.heard-z"
    } else {
        "listener.heard-a"
    };
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl<const FIRST: bool> WireOperation for Heard<FIRST> {
    const VERSION: u32 = 1;
}

#[harmos::catalog]
enum ImplementedCallbacks {
    First(Hearing<true>),
    Second(Hearing<false>),
}

struct Hearing<const FIRST: bool> {
    resources: Arc<Owner>,
    wait_cleanup: bool,
}

impl<const FIRST: bool> Operation for Hearing<FIRST> {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Heard<FIRST>;
    type Resources = Owner;
    type Error = Failed;
    async fn initialize<'ctx>(
        resources: Arc<Owner>,
        _: Self::Context<'ctx>,
    ) -> std::result::Result<Self, Failed> {
        Ok(Self {
            resources,
            wait_cleanup: false,
        })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Delivery<WireFact>,
        _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::Response<()>, Failed> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.wait_cleanup = FIRST && request.row().value == 0 && self.resources.block;
            let DeliveryContext::Record { position } = request.context() else {
                panic!("original record coordinate")
            };
            self.resources
                .seen
                .lock()
                .unwrap()
                .push((FIRST, request.row().value, position));
            self.resources.observed.notify_one();
            if FIRST && request.row().value == 0 {
                if self.resources.reenter {
                    self.resources.records.publish(fact(9)).await?;
                }
                if self.resources.block {
                    self.resources.started.notify_one();
                    self.resources.resume.notified().await;
                }
                if self.resources.fail_attempt.load(Ordering::SeqCst) {
                    return Err(Failed::Scripted);
                }
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> std::result::Result<(), Failed> {
        self.resources.finalizations.fetch_add(1, Ordering::SeqCst);
        if self.wait_cleanup {
            self.resources.finalizing.notify_one();
            if self.resources.fail_cleanup.load(Ordering::SeqCst) {
                return Err(Failed::Scripted);
            }
            self.resources.release.notified().await;
        }
        Ok(())
    }
}

impl PluginLifecycle for Host {
    type Guest = Resources;
    type Sidecar = Resources;
    type Embedded = Resources;
    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Resources {
            owner: Some(self.owner.clone()),
            ..Resources::default()
        }))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        _: &PluginDefinition,
        router: &Router,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        resources.endpoint = Some(router.enter("listener").unwrap());
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        self.scope = Some(scope.clone());
        resources.operations = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register_wire_operations::<ImplementedCallbacks, _>(
                Lanes::Open,
                self.owner.clone(),
            )
        })?);
        if !self.skip_sources {
            let original = Arc::new(tokio::sync::Mutex::new(
                self.owner.records.stage_listeners(
                    scope,
                    &self.operations,
                    Position::ORIGIN,
                    |bindings| bindings.record::<Fact>(),
                )?,
            ));
            self.view = Some(original.clone());
            resources.listeners = Some(original);
        }
        // The Same Scope Still Blocks Delivery Until PluginManager Commits.
        self.owner
            .records
            .publish(Fact {
                value: 0,
                dropped: self.original_drop.clone(),
            })
            .await?;
        tokio::task::yield_now().await;
        assert!(self.owner.seen.lock().unwrap().is_empty());
        if self.drop_before_commit {
            resources.listeners.take();
            self.view.take();
        }
        if self.fail_start {
            return Err(Error::InvalidInput {
                field: "listener fixture",
                reason: "failed original setup",
            });
        }
        Ok(())
    }
    async fn stop(
        _: &PluginDefinition,
        _: &PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else {
            unreachable!()
        };
        if let Some(registration) = &mut resources.listeners {
            registration.lock().await.shutdown().await?;
        } else if let Some(scope) = resources
            .endpoint
            .as_ref()
            .and_then(|endpoint| endpoint.scope())
        {
            resources
                .owner
                .as_ref()
                .unwrap()
                .records
                .shutdown_listeners_for(scope)
                .await?;
        }
        if let Some(registration) = &mut resources.operations {
            registration.shutdown().await?;
        }
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn fact(value: u32) -> Fact {
    Fact {
        value,
        dropped: Arc::new(AtomicUsize::new(0)),
    }
}

fn candidate() -> PluginDefinitions {
    let mut listeners = Vec::new();
    for (id, callback) in [
        (
            "listener.first",
            WireDeclaration::of::<Heard<true>>().contract().unwrap(),
        ),
        (
            "listener.second",
            WireDeclaration::of::<Heard<false>>().contract().unwrap(),
        ),
    ] {
        listeners.push(
            harmos_domain::listeners::Definition::new(
                harmos::Identifier::new(id).unwrap(),
                Source::Record {
                    id: "listener.fact".into(),
                    version: 1,
                    row: harmos::schema::graph::Schema::from_declaration(
                        <Fact as harmos::schema::Described>::DEFINITION,
                        &harmos::config::SchemaSettings::default(),
                    )
                    .unwrap(),
                },
                callback,
            )
            .unwrap(),
        );
    }
    PluginDefinitions::new(
        [PluginDefinition {
            id: "listener".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::Embedded {
                revision: "native".into(),
            },
            requires: vec![],
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                operations: harmos::operations::Declarations::of::<Callbacks>().unwrap(),
                listeners,
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(
    block: bool,
    reenter: bool,
    tail: usize,
) -> (
    Runtime<Model>,
    PluginManager<Resources, Resources, Resources>,
    Host,
) {
    let runtime = Runtime::<Model>::builder((), ())
        .register(Rows::catalog())
        .register(Lanes::catalog())
        .tail(tail)
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let owner = Arc::new(Owner {
        records: runtime.records.clone(),
        seen: Mutex::new(Vec::new()),
        observed: Notify::new(),
        started: Notify::new(),
        resume: Notify::new(),
        finalizing: Notify::new(),
        release: Notify::new(),
        finalizations: AtomicUsize::new(0),
        fail_cleanup: AtomicBool::new(false),
        fail_attempt: AtomicBool::new(false),
        block,
        reenter,
    });
    let host = Host {
        operations: runtime.operations.clone(),
        owner,
        scope: None,
        fail_start: false,
        skip_sources: false,
        drop_before_commit: false,
        original_drop: Arc::new(AtomicUsize::new(0)),
        view: None,
    };
    (runtime, PluginManager::new(candidate()).unwrap(), host)
}

async fn observed(owner: &Owner, count: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let changed = owner.observed.notified();
            if owner.seen.lock().unwrap().len() >= count {
                return;
            }
            changed.await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("reentrancy_preserve_positions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(false, true, 256).await;
                    manager.initialize(&mut host).await.unwrap();
                    observed(&host.owner, 4).await;
                    assert_eq!(
                        *host.owner.seen.lock().unwrap(),
                        [
                            (true, 0, Position(1)),
                            (false, 0, Position(1)),
                            (true, 9, Position(2)),
                            (false, 9, Position(2))
                        ]
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("sources_never_commit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for mode in 0..3 {
                        let (runtime, mut manager, mut host) = setup(false, false, 8).await;
                        host.skip_sources = mode == 0;
                        host.drop_before_commit = mode == 1;
                        host.fail_start = mode == 2;
                        assert!(manager.initialize(&mut host).await.is_err());
                        assert!(!host.scope.as_ref().unwrap().is_ready());
                        assert!(host.owner.seen.lock().unwrap().is_empty());
                        manager.shutdown(&mut host).await.unwrap();
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("failure_fences_side", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(false, false, 8).await;
                    host.owner.fail_attempt.store(true, Ordering::SeqCst);
                    manager.initialize(&mut host).await.unwrap();
                    observed(&host.owner, 1).await;
                    tokio::time::timeout(Duration::from_secs(3), async {
                        while host.scope.as_ref().unwrap().is_ready() {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
                    assert_eq!(
                        manager.status("listener").unwrap().state,
                        harmos::plugin::PluginState::Failed
                    );
                    assert!(matches!(
                        host.view
                            .as_ref()
                            .unwrap()
                            .lock()
                            .await
                            .failure()
                            .as_deref(),
                        Some(Error::OperationFailed { .. })
                    ));
                    assert!(matches!(
                        runtime.operations.definition(Heard::<false>::ID),
                        Err(Error::Closing { .. })
                    ));
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("runtime_signals_listener", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    struct BlockedObserver;
                    struct Gates {
                        running: Notify,
                        stopping: Notify,
                        release: Notify,
                    }
                    impl harmos::observers::ObserverBackend for BlockedObserver {
                        type Resources = Gates;
                        fn definition(
                            &self,
                        ) -> harmos::Result<harmos::observers::ObserverDefinition>
                        {
                            Ok(harmos::observers::ObserverDefinition {
                                id: "blocked-cleanup".into(),
                                interval: Duration::from_secs(10),
                                streams: vec![],
                                demand_keys: vec![],
                            })
                        }
                        async fn run(
                            &self,
                            gates: Arc<Gates>,
                            mut activation: harmos::observers::ObserverActivation,
                        ) -> Result<()> {
                            activation.ready();
                            gates.running.notify_one();
                            activation.stopping().await;
                            gates.stopping.notify_one();
                            gates.release.notified().await;
                            Ok(())
                        }
                    }
                    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
                    manager.initialize(&mut host).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(3), host.owner.started.notified())
                        .await
                        .unwrap();
                    let gates = Arc::new(Gates {
                        running: Notify::new(),
                        stopping: Notify::new(),
                        release: Notify::new(),
                    });
                    runtime
                        .operations
                        .register_observer(BlockedObserver)
                        .unwrap();
                    let lease = runtime
                        .operations
                        .submit_with_resources_at("blocked-cleanup", gates.clone())
                        .await
                        .unwrap();
                    tokio::time::timeout(Duration::from_secs(2), gates.running.notified())
                        .await
                        .expect("the observer starts before testing blocked cleanup");
                    let mut stopping = Box::pin(runtime.stop());
                    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = async {
                gates.stopping.notified().await;
                host.owner.finalizing.notified().await;
            } => {},
            result = &mut stopping => panic!("runtime settled before blocked cleanup: {result:?}"),
        }
    })
    .await
    .unwrap();
                    assert_eq!(host.owner.finalizations.load(Ordering::SeqCst), 1);
                    drop(stopping);
                    host.owner.release.notify_one();
                    gates.release.notify_one();
                    runtime.stop().await.unwrap();
                    drop(lease);
                });
        });
    }
    {
        scenario_runtime::check("shutdown_retains_callback", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), host.owner.started.notified())
        .await
        .unwrap();
    runtime.records.publish(fact(1)).await.unwrap();
    let owner = host.owner.clone();
    let mut closing = Box::pin(manager.shutdown(&mut host));
    tokio::select! { biased; _ = owner.finalizing.notified() => {}, result = &mut closing => panic!("cleanup returned before original finalizer: {result:?}") }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut closing)
            .await
            .is_err()
    );
    drop(closing);
    assert_eq!(host.original_drop.load(Ordering::SeqCst), 0);
    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
    host.owner.release.notify_one();
    manager.shutdown(&mut host).await.unwrap();
    assert_eq!(host.owner.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(host.original_drop.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("cleanup_retries_finalizer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
                    manager.initialize(&mut host).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(3), host.owner.started.notified())
                        .await
                        .unwrap();
                    runtime.records.publish(fact(1)).await.unwrap();
                    host.owner.fail_cleanup.store(true, Ordering::SeqCst);
                    assert!(manager.shutdown(&mut host).await.is_err());
                    assert_eq!(host.original_drop.load(Ordering::SeqCst), 0);
                    let before = host.owner.finalizations.load(Ordering::SeqCst);
                    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
                    host.owner.fail_cleanup.store(false, Ordering::SeqCst);
                    host.owner.release.notify_one();
                    manager.shutdown(&mut host).await.unwrap();
                    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
                    assert_eq!(host.owner.finalizations.load(Ordering::SeqCst), before + 1);
                    assert_eq!(host.original_drop.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("pages_deliver_native", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(true, false, 512).await;
                    manager.initialize(&mut host).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(3), host.owner.started.notified())
                        .await
                        .unwrap();
                    for value in 1..=260 {
                        runtime.records.publish(fact(value)).await.unwrap();
                    }
                    host.owner.resume.notify_one();
                    host.owner.release.notify_one();
                    observed(&host.owner, 522).await;
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            if host.view.as_ref().unwrap().lock().await.frontier()
                                == Some(Position(261))
                            {
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    {
                        let seen = host.owner.seen.lock().unwrap();
                        for (value, pair) in seen.as_chunks::<2>().0.iter().enumerate() {
                            assert_eq!(
                                *pair,
                                [
                                    (true, value as u32, Position(value as u64 + 1)),
                                    (false, value as u32, Position(value as u64 + 1))
                                ]
                            );
                        }
                    }
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("gap_fences_generation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
                    manager.initialize(&mut host).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(3), host.owner.started.notified())
                        .await
                        .unwrap();
                    runtime.records.publish(fact(1)).await.unwrap();
                    runtime.records.publish(fact(2)).await.unwrap();
                    host.owner.resume.notify_one();
                    host.owner.release.notify_one();
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            if host.view.as_ref().unwrap().lock().await.failure().is_some() {
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    assert!(matches!(
                        host.view
                            .as_ref()
                            .unwrap()
                            .lock()
                            .await
                            .failure()
                            .as_deref(),
                        Some(Error::BeyondTail)
                    ));
                    assert_eq!(
                        manager.status("listener").unwrap().state,
                        harmos::plugin::PluginState::Failed
                    );
                    assert_eq!(
                        *host.owner.seen.lock().unwrap(),
                        [(true, 0, Position(1)), (false, 0, Position(1))]
                    );
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("receipt_cleans_replacement", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(false, false, 8).await;
                    host.drop_before_commit = true;
                    assert!(manager.initialize(&mut host).await.is_err());
                    let old = host.scope.as_ref().unwrap().clone();
                    assert!(!old.is_ready());
                    assert!(host.owner.seen.lock().unwrap().is_empty());
                    host.drop_before_commit = false;
                    manager.initialize(&mut host).await.unwrap();
                    observed(&host.owner, 4).await;
                    let current = host.scope.as_ref().unwrap();
                    assert!(current.is_ready());
                    assert!(runtime.records.shutdown_listeners_for(&old).await.is_err());
                    assert!(current.is_ready());
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("codec_panic_fences", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, mut manager, mut host) = setup(false, false, 8).await;
                    manager.initialize(&mut host).await.unwrap();
                    observed(&host.owner, 2).await;
                    runtime.records.publish(fact(u32::MAX)).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            if host.view.as_ref().unwrap().lock().await.failure().is_some() {
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    assert!(matches!(
                        host.view
                            .as_ref()
                            .unwrap()
                            .lock()
                            .await
                            .failure()
                            .as_deref(),
                        Some(Error::OperationPanicked { .. })
                    ));
                    assert_eq!(
                        manager.status("listener").unwrap().state,
                        harmos::plugin::PluginState::Failed
                    );
                    assert_eq!(host.owner.seen.lock().unwrap().len(), 2);
                    manager.shutdown(&mut host).await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
