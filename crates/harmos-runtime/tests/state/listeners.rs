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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
// Local Imports
use harmos_domain::listeners::{Context as DeliveryContext, Delivery, Listener, Source};
use harmos_runtime::operations::{Context, Operation, OperationPolicy, PluginRegistration, RestartPolicy, RetryPolicy, WireDeclaration};
use harmos_runtime::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginScope, PluginTransition, Router,
};
use harmos_runtime::records::PluginListenerRegistration;
use harmos_runtime::schema::{Described, Field};
use harmos_runtime::{Error, Message, MessageError, Result, Runtime, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<Resources, Resources, Resources>;
type Seen = Vec<(bool, u32, Position)>;

/* === Structs === */
// Neither The Original Native Record Nor Its Decoded Callback Copy Is Clone.
#[harmos_runtime::record(id = "listener.fact", version = 1)]
#[derive(Serialize, Deserialize)]
struct Fact {
    value: u32,
    #[serde(skip)]
    dropped: Arc<AtomicUsize>,
}

#[harmos_runtime::message]
struct WireFact {
    value: u32,
}

struct Model;

// Each Callback Over The Same Row Is Its Own Listener.
struct Heard<const FIRST: bool>;

struct Owner {
    records: harmos_runtime::records::RecordManager<Model>,
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
    operations: harmos_runtime::operations::OperationManager,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    fail_start: bool,
    skip_sources: bool,
    drop_before_commit: bool,
    original_drop: Arc<AtomicUsize>,
    view: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

/* === Enums === */
#[harmos_runtime::records]
enum Rows {
    Fact(Fact),
}

#[harmos_runtime::catalog]
enum Callbacks {
    First(Delivery<Heard<true>>),
    Second(Delivery<Heard<false>>),
}

#[harmos_runtime::lanes]
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
    const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
        { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
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
        Ok(Self { value: WireFact::decode(bytes)?.value, dropped: Arc::new(AtomicUsize::new(0)) })
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = Rows;
    type Streams = ();
    type Resources = ();
}

impl<const FIRST: bool> Listener for Heard<FIRST> {
    type Row = WireFact;
    const CALLBACK: &'static str = if FIRST { "listener.heard-z" } else { "listener.heard-a" };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

#[harmos_runtime::catalog]
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
    type Declaration = Delivery<Heard<FIRST>>;
    type Resources = Owner;
    type Error = Failed;
    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> std::result::Result<Self, Failed> {
        Ok(Self { resources, wait_cleanup: false })
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut Delivery<Heard<FIRST>>, _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos_runtime::operations::Response<()>, Failed> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.wait_cleanup = FIRST && request.row().value == 0 && self.resources.block;
            let DeliveryContext::Record { position } = request.context() else { panic!("original record coordinate") };
            self.resources.seen.lock().unwrap().push((FIRST, request.row().value, position));
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
        Ok(PluginBackend::Embedded(Resources { owner: Some(self.owner.clone()), ..Resources::default() }))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }
    async fn start(
        &mut self, _: &PluginDefinition, router: &Router, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        resources.endpoint = Some(router.enter("listener").unwrap());
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        self.scope = Some(scope.clone());
        resources.operations = Some(self.operations.stage_plugin(scope, |bindings| {
            bindings.register_wire_operations::<ImplementedCallbacks, _>(Lanes::Open, self.owner.clone())
        })?);
        let original = Arc::new(tokio::sync::Mutex::new(self.owner.records.stage_listeners(
            scope,
            &self.operations,
            Position::ORIGIN,
            |bindings| bindings.record::<Fact>(),
        )?));
        self.view = Some(original.clone());
        resources.listeners = Some(original);

        // The Same Scope Still Blocks Delivery Until PluginManager Commits.
        self.owner.records.publish(fact(0)).await?;
        tokio::task::yield_now().await;
        assert!(self.owner.seen.lock().unwrap().is_empty());
        Ok(())
    }
    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        if let Some(registration) = &mut resources.listeners {
            registration.lock().await.shutdown().await?;
        } else if let Some(scope) = resources.endpoint.as_ref().and_then(|endpoint| endpoint.scope()) {
            resources.owner.as_ref().unwrap().records.shutdown_listeners_for(scope).await?;
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
    Fact { value, dropped: Arc::new(AtomicUsize::new(0)) }
}

fn candidate() -> PluginDefinitions {
    let mut listeners = Vec::new();
    for (id, callback) in [
        ("listener.first", WireDeclaration::of::<Delivery<Heard<true>>>().contract().unwrap()),
        ("listener.second", WireDeclaration::of::<Delivery<Heard<false>>>().contract().unwrap()),
    ] {
        listeners.push(
            harmos_domain::listeners::Definition::new(
                harmos_runtime::Identifier::new(id).unwrap(),
                Source::Record {
                    id: "listener.fact".into(),
                    version: 1,
                    row: harmos_runtime::schema::graph::Schema::from_declaration(
                        <Fact as harmos_runtime::schema::Described>::DEFINITION,
                        &harmos_runtime::config::SchemaSettings::default(),
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
            artifact: PluginArtifact::Embedded { revision: "native".into() },
            requires: vec![],
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                operations: harmos_runtime::operations::Declarations::of::<Callbacks>().unwrap(),
                listeners,
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(block: bool, reenter: bool, tail: usize) -> (Runtime<Model>, PluginManager<Resources, Resources, Resources>, Host) {
    let runtime = Runtime::<Model>::builder((), ())
        .register(Rows::catalog())
        .register(Lanes::catalog())
        .tail(tail)
        .operation_settings(harmos_runtime::operations::Settings {
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
    loop {
        let changed = owner.observed.notified();
        if owner.seen.lock().unwrap().len() >= count {
            return;
        }
        changed.await;
    }
}

// Listener failure records its error, then closes the plugin scope. Only the
// crate-private `PluginScope::closed` awaits that close; until it is public
// this yields to the delivery task, and becomes `scope.closed().await` then.
async fn fenced(host: &Host) {
    let scope = host.scope.as_ref().unwrap();
    while scope.is_ready() {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn reentrancy_preserve_positions() {
    let (runtime, mut manager, mut host) = setup(false, true, 256).await;
    manager.initialize(&mut host).await.unwrap();
    observed(&host.owner, 4).await;
    assert_eq!(
        *host.owner.seen.lock().unwrap(),
        [(true, 0, Position(1)), (false, 0, Position(1)), (true, 9, Position(2)), (false, 9, Position(2))]
    );
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn sources_never_commit() {
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
}

#[tokio::test]
async fn failure_fences_side() {
    let (runtime, mut manager, mut host) = setup(false, false, 8).await;
    host.owner.fail_attempt.store(true, Ordering::SeqCst);
    manager.initialize(&mut host).await.unwrap();
    observed(&host.owner, 1).await;
    (async {
        while host.scope.as_ref().unwrap().is_ready() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
    assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::plugin::PluginState::Failed);
    assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::OperationFailed { .. })));
    assert!(matches!(runtime.operations.definition(Heard::<false>::CALLBACK), Err(Error::Closing { .. })));
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn runtime_signals_listener() {
    struct BlockedObserver;
    struct Gates {
        running: Notify,
        stopping: Notify,
        release: Notify,
    }
    impl harmos_runtime::observers::ObserverBackend for BlockedObserver {
        type Resources = Gates;
        fn definition(&self) -> harmos_runtime::Result<harmos_runtime::observers::ObserverDefinition> {
            Ok(harmos_runtime::observers::ObserverDefinition {
                id: "blocked-cleanup".into(),
                interval: Duration::from_secs(10),
                streams: vec![],
                demand_keys: vec![],
            })
        }
        async fn run(&self, gates: Arc<Gates>, mut activation: harmos_runtime::observers::ObserverActivation) -> Result<()> {
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
    (host.owner.started.notified()).await;
    let gates = Arc::new(Gates { running: Notify::new(), stopping: Notify::new(), release: Notify::new() });
    runtime.operations.register_observer(BlockedObserver).unwrap();
    let lease = runtime.operations.submit_with_resources_at("blocked-cleanup", gates.clone()).await.unwrap();
    (gates.running.notified()).await;
    let mut stopping = Box::pin(runtime.stop());
    (async {
        tokio::select! {
            _ = async {
                gates.stopping.notified().await;
                host.owner.finalizing.notified().await;
            } => {},
            result = &mut stopping => panic!("runtime settled before blocked cleanup: {result:?}"),
        }
    })
    .await;
    assert_eq!(host.owner.finalizations.load(Ordering::SeqCst), 1);
    drop(stopping);
    host.owner.release.notify_one();
    gates.release.notify_one();
    runtime.stop().await.unwrap();
    drop(lease);
}

#[tokio::test]
async fn shutdown_retains_callback() {
    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    (host.owner.started.notified()).await;
    runtime.records.publish(fact(1)).await.unwrap();
    let owner = host.owner.clone();
    let mut closing = Box::pin(manager.shutdown(&mut host));
    tokio::select! { biased; _ = owner.finalizing.notified() => {}, result = &mut closing => panic!("cleanup returned before original finalizer: {result:?}") }
    assert!(futures_util::poll!(&mut closing).is_pending());
    drop(closing);
    assert_eq!(host.original_drop.load(Ordering::SeqCst), 0);
    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
    host.owner.release.notify_one();
    manager.shutdown(&mut host).await.unwrap();
    assert_eq!(host.owner.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(host.original_drop.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cleanup_retries_finalizer() {
    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    (host.owner.started.notified()).await;
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
}

#[tokio::test]
async fn pages_deliver_native() {
    let (runtime, mut manager, mut host) = setup(true, false, 512).await;
    manager.initialize(&mut host).await.unwrap();
    host.owner.started.notified().await;
    for value in 1..=260 {
        runtime.records.publish(fact(value)).await.unwrap();
    }
    host.owner.resume.notify_one();
    observed(&host.owner, 522).await;

    // The frontier advances only after a record's last callback settles, so
    // one more record's first callback proves every earlier page settled.
    runtime.records.publish(fact(261)).await.unwrap();
    observed(&host.owner, 523).await;
    let frontier = host.view.as_ref().unwrap().lock().await.frontier();
    assert!(matches!(frontier, Some(Position(261 | 262))));
    {
        let seen = host.owner.seen.lock().unwrap();
        for (value, pair) in seen.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(*pair, [(true, value as u32, Position(value as u64 + 1)), (false, value as u32, Position(value as u64 + 1))]);
        }
    }
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn gap_fences_generation() {
    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    host.owner.started.notified().await;
    runtime.records.publish(fact(1)).await.unwrap();
    runtime.records.publish(fact(2)).await.unwrap();
    host.owner.resume.notify_one();
    fenced(&host).await;
    assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::BeyondTail)));
    assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::plugin::PluginState::Failed);
    assert_eq!(*host.owner.seen.lock().unwrap(), [(true, 0, Position(1)), (false, 0, Position(1))]);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

/* === Tests === */
