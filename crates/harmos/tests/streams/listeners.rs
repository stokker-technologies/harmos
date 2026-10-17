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
use harmos::operations::{Context, Operation, OperationPolicy, PluginRegistration, RestartPolicy, RetryPolicy, WireDeclaration};
use harmos::plugin::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginScope, PluginTransition, Router,
};
use harmos::schema::{Described, Field};
use harmos::streams::PluginListenerRegistration;
use harmos::{Error, Message, MessageError, Result, Runtime};
use harmos_domain::listeners::{Context as DeliveryContext, Delivery, Listener, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Backends = PluginBackend<Resources, Resources, Resources>;
type Seen = Vec<(bool, u32, DeliveryContext)>;

/* === Structs === */
// Clone Panics To Prove The Listener Borrows The Original Native Stream Row.
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

// Each Callback Over The Same Row Is Its Own Listener.
struct Heard<const FIRST: bool>;

struct Owner {
    streams: harmos::streams::StreamManager<Model>,
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
    managed: bool,
    endpoint: Option<PluginEndpoint>,
    operations: Option<PluginRegistration>,
    streams: Option<harmos::streams::StreamRegistration>,
    listeners: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

struct Host {
    operations: harmos::operations::OperationManager,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    fail_start: bool,
    managed: bool,
    lease: Option<Arc<harmos::streams::PublicationLease>>,
    skip_sources: bool,
    drop_before_commit: bool,
    original_drop: Arc<AtomicUsize>,
    view: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

/* === Enums === */
#[harmos::streams]
enum Rows {
    Fact(Fact),
}

#[harmos::catalog]
enum Callbacks {
    First(Delivery<Heard<true>>),
    Second(Delivery<Heard<false>>),
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
impl Fact {
    const DEFINITION: (&'static str, u32, usize) = ("listener.samples", 1, 512);
    fn definition() -> harmos::streams::Definition {
        harmos::__private::stream_definition::<Self>(Self::DEFINITION.0, Self::DEFINITION.1, Self::DEFINITION.2)
    }
}

impl harmos::__private::StreamRow for Fact {
    const ID: &'static str = "listener.samples";
    const VERSION: u32 = 1;
    const WINDOW: usize = 512;
}

impl Clone for Fact {
    fn clone(&self) -> Self {
        panic!("native listener must borrow the original row");
    }
}

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
        Ok(Self { value: WireFact::decode(bytes)?.value, dropped: Arc::new(AtomicUsize::new(0)) })
    }
}

impl harmos::Application for Model {
    type State = ();
    type Transaction = harmos::state::Never;
    type Record = ();
    type Streams = Rows;
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
    type Declaration = Delivery<Heard<FIRST>>;
    type Resources = Owner;
    type Error = Failed;
    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> std::result::Result<Self, Failed> {
        Ok(Self { resources, wait_cleanup: false })
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut Delivery<Heard<FIRST>>, _endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<harmos::operations::Response<()>, Failed> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.wait_cleanup = FIRST && request.row().value == 0 && self.resources.block;
            assert!(matches!(request.context(), DeliveryContext::Stream { .. }));
            self.resources.seen.lock().unwrap().push((FIRST, request.row().value, request.context()));
            self.resources.observed.notify_one();
            if FIRST && request.row().value == 0 {
                if self.resources.reenter {
                    self.resources.streams.publish(fact(9));
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
        Ok(PluginBackend::Embedded(Resources { owner: Some(self.owner.clone()), managed: self.managed, ..Resources::default() }))
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
        if self.managed {
            resources.streams = Some(self.owner.streams.stage_plugin(scope, [publication()]).await.unwrap());
            self.lease = Some(resources.streams.as_ref().unwrap().leases()[0].clone());
            let original = scope.publication("listener.samples", 1).unwrap();
            assert!(Arc::ptr_eq(&original, self.lease.as_ref().unwrap()));
            assert!(matches!(scope.publication("listener.samples", 2), Err(Error::InvalidInput { field: "publication stream", .. })));
        }
        if !self.skip_sources {
            let original = Arc::new(tokio::sync::Mutex::new(self.owner.streams.stage_listeners(scope, &self.operations, |bindings| {
                if self.managed { bindings.publication("listener.samples", 1, 0) } else { bindings.stream::<Fact>(0) }
            })?));
            self.view = Some(original.clone());
            resources.listeners = Some(original);
        }
        // The Same Scope Still Blocks Delivery Until PluginManager Commits.
        if let Some(streams) = &resources.streams {
            streams.leases()[0].stage_derived_row(publication_row(0), 0).unwrap();
        } else {
            self.owner.streams.publish(Fact { value: 0, dropped: self.original_drop.clone() });
        }
        tokio::task::yield_now().await;
        assert!(self.owner.seen.lock().unwrap().is_empty());
        if self.drop_before_commit {
            resources.listeners.take();
            self.view.take();
        }
        if self.fail_start {
            return Err(Error::InvalidInput { field: "listener fixture", reason: "failed original setup" });
        }
        Ok(())
    }
    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        if let Some(registration) = &mut resources.listeners {
            registration.lock().await.shutdown().await?;
        } else if let Some(scope) = resources.endpoint.as_ref().and_then(|endpoint| endpoint.scope()) {
            resources.owner.as_ref().unwrap().streams.shutdown_listeners_for(scope).await?;
        }
        if let Some(registration) = &mut resources.operations {
            registration.shutdown().await?;
        }
        if resources.managed {
            let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
            assert!(matches!(scope.publication("listener.samples", 1), Err(Error::Closing { domain: "publication generation" })));
        }
        resources.streams.take();
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn fact(value: u32) -> Fact {
    Fact { value, dropped: Arc::new(AtomicUsize::new(0)) }
}

fn publication() -> harmos_domain::StreamDeclaration {
    harmos_domain::StreamDeclaration {
        metadata: harmos::Metadata::new(harmos::Identifier::new("listener.samples").unwrap(), "Samples", None),
        row_metadata: vec![],
        id: "listener.samples".into(),
        schema: 1,
        window: 512,
        row: harmos::schema::graph::Schema::from_declaration(
            <Fact as harmos::schema::Described>::DEFINITION,
            &harmos::config::SchemaSettings::default(),
        )
        .unwrap(),
    }
}

fn publication_row(value: u32) -> harmos::streams::PublicationRow {
    harmos::streams::PublicationRow { payload: WireFact { value }.encode_to_vec(), published_unix_micros: None, context: None }
}

fn candidate(managed: bool) -> PluginDefinitions {
    let mut listeners = Vec::new();
    for (id, callback) in [
        ("listener.first", WireDeclaration::of::<Delivery<Heard<true>>>().contract().unwrap()),
        ("listener.second", WireDeclaration::of::<Delivery<Heard<false>>>().contract().unwrap()),
    ] {
        listeners.push(
            harmos_domain::listeners::Definition::new(
                harmos::Identifier::new(id).unwrap(),
                Source::Stream {
                    id: "listener.samples".into(),
                    schema_ordinal: 1,
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
            artifact: PluginArtifact::Embedded { revision: "native".into() },
            requires: vec![],
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                operations: harmos::operations::Declarations::of::<Callbacks>().unwrap(),
                listeners,
                streams: if managed { vec![publication()] } else { vec![] },
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(block: bool, reenter: bool, tail: usize) -> (Runtime<Model>, PluginManager<Resources, Resources, Resources>, Host) {
    setup_source(block, reenter, tail, false).await
}

async fn setup_source(
    block: bool, reenter: bool, tail: usize, managed: bool,
) -> (Runtime<Model>, PluginManager<Resources, Resources, Resources>, Host) {
    let runtime = Runtime::<Model>::builder((), ())
        .register(if managed { vec![] } else { Rows::catalog() })
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
        streams: runtime.streams.clone(),
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
        managed,
        lease: None,
        skip_sources: false,
        drop_before_commit: false,
        original_drop: Arc::new(AtomicUsize::new(0)),
        view: None,
    };
    (runtime, PluginManager::new(candidate(managed)).unwrap(), host)
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
async fn reentrancy_preserve_logical() {
    let (runtime, mut manager, mut host) = setup(false, true, 256).await;
    manager.initialize(&mut host).await.unwrap();
    observed(&host.owner, 4).await;
    assert_eq!(
        *host.owner.seen.lock().unwrap(),
        [
            (true, 0, DeliveryContext::Stream { sequence: 1, missed: 0 }),
            (false, 0, DeliveryContext::Stream { sequence: 1, missed: 0 }),
            (true, 9, DeliveryContext::Stream { sequence: 2, missed: 0 }),
            (false, 9, DeliveryContext::Stream { sequence: 2, missed: 0 })
        ]
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
    fenced(&host).await;
    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
    assert_eq!(manager.status("listener").unwrap().state, harmos::plugin::PluginState::Failed);
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
    impl harmos::observers::ObserverBackend for BlockedObserver {
        type Resources = Gates;
        fn definition(&self) -> harmos::Result<harmos::observers::ObserverDefinition> {
            Ok(harmos::observers::ObserverDefinition {
                id: "blocked-cleanup".into(),
                interval: Duration::from_secs(10),
                streams: vec![],
                demand_keys: vec![],
            })
        }
        async fn run(&self, gates: Arc<Gates>, mut activation: harmos::observers::ObserverActivation) -> Result<()> {
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
    host.owner.started.notified().await;
    let gates = Arc::new(Gates { running: Notify::new(), stopping: Notify::new(), release: Notify::new() });
    runtime.operations.register_observer(BlockedObserver).unwrap();
    let lease = runtime.operations.submit_with_resources_at("blocked-cleanup", gates.clone()).await.unwrap();
    gates.running.notified().await;
    let mut stopping = Box::pin(runtime.stop());
    tokio::select! {
        _ = async {
            gates.stopping.notified().await;
            host.owner.finalizing.notified().await;
        } => {},
        result = &mut stopping => panic!("runtime settled before blocked cleanup: {result:?}"),
    }
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
    host.owner.started.notified().await;
    for value in 1..=512 {
        runtime.streams.publish(fact(value));
    }
    let owner = host.owner.clone();
    let mut closing = Box::pin(manager.shutdown(&mut host));
    tokio::select! { biased; _ = owner.finalizing.notified() => {}, result = &mut closing => panic!("cleanup returned before original finalizer: {result:?}") }
    let pending = std::future::poll_fn(|context| std::task::Poll::Ready(closing.as_mut().poll(context).is_pending())).await;
    assert!(pending, "shutdown waits on the held finalizer");
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
    host.owner.started.notified().await;
    for value in 1..=512 {
        runtime.streams.publish(fact(value));
    }
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
        runtime.streams.publish(fact(value));
    }
    host.owner.resume.notify_one();
    host.owner.release.notify_one();
    observed(&host.owner, 522).await;

    // The frontier advances only after a row's last callback settles, so one
    // more row's first callback proves every earlier page settled.
    runtime.streams.publish(fact(261));
    observed(&host.owner, 523).await;
    let frontier = host.view.as_ref().unwrap().lock().await.frontier("listener.samples", 1);
    assert!(matches!(frontier, Some(261 | 262)));
    {
        let seen = host.owner.seen.lock().unwrap();
        for (value, pair) in seen.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(
                *pair,
                [
                    (true, value as u32, DeliveryContext::Stream { sequence: value as u64 + 1, missed: 0 }),
                    (false, value as u32, DeliveryContext::Stream { sequence: value as u64 + 1, missed: 0 })
                ]
            );
        }
    }
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn overwrite_reports_logical() {
    let (runtime, mut manager, mut host) = setup(true, false, 8).await;
    manager.initialize(&mut host).await.unwrap();
    host.owner.started.notified().await;
    for value in 1..=520 {
        runtime.streams.publish(fact(value));
    }
    host.owner.resume.notify_one();
    host.owner.release.notify_one();
    observed(&host.owner, 1026).await;
    {
        let seen = host.owner.seen.lock().unwrap();
        assert_eq!(seen[2], (true, 9, DeliveryContext::Stream { sequence: 10, missed: 8 }));
        assert_eq!(seen[3], (false, 9, DeliveryContext::Stream { sequence: 10, missed: 8 }));
        assert_eq!(seen.last(), Some(&(false, 520, DeliveryContext::Stream { sequence: 521, missed: 0 })));
    }
    assert!(host.scope.as_ref().unwrap().is_ready());
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn receipt_cleans_replacement() {
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
    assert!(runtime.streams.shutdown_listeners_for(&old).await.is_err());
    assert!(current.is_ready());
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn codec_panic_fences() {
    let (runtime, mut manager, mut host) = setup(false, false, 8).await;
    manager.initialize(&mut host).await.unwrap();
    observed(&host.owner, 2).await;
    runtime.streams.publish(fact(u32::MAX));
    fenced(&host).await;
    assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::OperationPanicked { .. })));
    assert_eq!(manager.status("listener").unwrap().state, harmos::plugin::PluginState::Failed);
    assert_eq!(host.owner.seen.lock().unwrap().len(), 2);
    manager.shutdown(&mut host).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn source_preserves_staged() {
    let (runtime, mut manager, mut host) = setup_source(false, false, 256, true).await;
    manager.initialize(&mut host).await.unwrap();
    observed(&host.owner, 2).await;
    let lease = host.lease.as_ref().unwrap().clone();
    lease.admit_row(publication_row(5), 5, 3).unwrap();
    observed(&host.owner, 4).await;
    assert_eq!(
        host.owner.seen.lock().unwrap()[2..],
        [(true, 5, DeliveryContext::Stream { sequence: 5, missed: 3 }), (false, 5, DeliveryContext::Stream { sequence: 5, missed: 3 }),]
    );
    assert_eq!(host.view.as_ref().unwrap().lock().await.frontier("listener.samples", 1), Some(5));
    manager.shutdown(&mut host).await.unwrap();
    assert!(lease.admit_row(publication_row(6), 6, 0).is_err());
    runtime.stop().await.unwrap();
}

/* === Tests === */
