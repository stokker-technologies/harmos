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
use harmos_domain::listeners::{Context as DeliveryContext, Delivery, Source};
use harmos_runtime::guest::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginScope, PluginTransition, Router,
};
use harmos_runtime::operations::{OperationPolicy, PluginRegistration, RetryPolicy};
use harmos_runtime::schema::{Described, Field};
use harmos_runtime::streams::PluginListenerRegistration;
use harmos_runtime::{Error, Message, MessageError, Result, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources>;
type Seen = Vec<(bool, u32, DeliveryContext)>;

/* === Structs === */

// Clone Panics To Prove The Listener Borrows The Original Native Stream Row.
#[derive(Serialize, Deserialize)]
struct Fact {
    value: u32,

    #[serde(skip)]
    dropped: Arc<AtomicUsize>,
}

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct WireFact {
    value: u32,
}

struct Model;

// Each Callback Over The Same Row Is Its Own Listener.
struct Heard<const FIRST: bool>;

struct OwnerResource(std::sync::OnceLock<Arc<Owner>>);

struct Owner {
    streams: harmos_runtime::streams::StreamManager<Model>,
    seen: Mutex<Seen>,
    observed: Notify,
    started: Notify,
    resume: Notify,
    settled: Notify,
    finished: AtomicUsize,
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
    streams: Option<harmos_runtime::streams::StreamRegistration>,
    listeners: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

struct Host {
    operations: harmos_runtime::operations::OperationManager,
    owner: Arc<Owner>,
    scope: Option<PluginScope>,
    fail_start: bool,
    managed: bool,
    lease: Option<Arc<harmos_runtime::streams::StreamWriter>>,
    skip_sources: bool,
    drop_before_commit: bool,
    original_drop: Arc<AtomicUsize>,
    view: Option<Arc<tokio::sync::Mutex<PluginListenerRegistration>>>,
}

/* === Enums === */

fn handler_operations_catalog() -> harmos_runtime::operations::Catalog {
    handler_operations_catalog_batch().named(concat!(module_path!(), "::HandlerOperations"))
}

fn handler_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(hearing_first_operation().expect("representable operation"));
    batch = batch.entry(hearing_second_operation().expect("representable operation"));
    batch
}

static CALLBACKS_CATALOG_DECLARATIONS: [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>; 2] = [
    harmos_domain::catalog::Node::Leaf(HEARING_FIRST_OPERATION.wire()),
    harmos_domain::catalog::Node::Leaf(HEARING_SECOND_OPERATION.wire()),
];
static CALLBACKS_CATALOG_CALLEES: [harmos_domain::catalog::Node<&'static [harmos_domain::operations::WireDeclaration]>; 2] = [
    harmos_domain::catalog::Node::Leaf(HEARING_FIRST_OPERATION.callees()),
    harmos_domain::catalog::Node::Leaf(HEARING_SECOND_OPERATION.callees()),
];

const CALLBACKS_CATALOG: harmos_runtime::operations::CatalogDescription = harmos_runtime::operations::CatalogDescription::new(
    concat!(module_path!(), "::Callbacks"),
    &CALLBACKS_CATALOG_DECLARATIONS,
    &CALLBACKS_CATALOG_CALLEES,
    None,
);

#[derive(Debug, thiserror::Error)]
enum Failed {
    #[error("scripted original callback failure")]
    Scripted,

    #[error(transparent)]
    Runtime(#[from] Error),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::RowMetadata for Fact {}

impl harmos_runtime::streams::StreamRow for Fact {
    const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("listener.samples", 512);
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
    const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
        { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Message for Fact {
    const CODEC: &'static str = WireFact::CODEC;
    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
        WireFact::decoding_memory(bytes)
    }

    fn encode(&self) -> Vec<u8> {
        WireFact { value: self.value }.encode()
    }
    fn encoded_size(&self) -> usize {
        assert_ne!(self.value, u32::MAX, "scripted native source codec panic");
        WireFact { value: self.value }.encoded_size()
    }
    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        Ok(Self {
            value: WireFact::decode(bytes)?.value,
            dropped: Arc::new(AtomicUsize::new(0)),
        })
    }
}
impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

impl<const FIRST: bool> Heard<FIRST> {
    const CALLBACK: &'static str = if FIRST { "heard.z" } else { "heard.a" };
    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
}

struct AttemptGuard(Arc<Owner>);
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
        self.0.settled.notify_one();
    }
}
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = ())]
#[derive(Clone)]
struct HearingFirst(Delivery<WireFact>);

static HEARING_FIRST_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<HearingFirst>(Heard::<true>::CALLBACK, Heard::<true>::POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const HEARING_FIRST_OPERATION: harmos_runtime::operations::OperationSpec<HearingFirst> =
    harmos_runtime::operations::OperationSpec::<HearingFirst>::new(&HEARING_FIRST_DESCRIPTION);

impl harmos_runtime::Message for HearingFirst {
    const CODEC: &'static str = <Delivery<WireFact> as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, harmos_runtime::MessageError> {
        <Delivery<WireFact> as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> std::result::Result<Self, harmos_runtime::MessageError> {
        <Delivery<WireFact> as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for HearingFirst {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for HearingFirst {
    const NAME: &'static str = <Delivery<WireFact> as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <Delivery<WireFact> as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <Delivery<WireFact> as harmos_runtime::schema::Described>::MARKER;
}
async fn hearing_first_handler(
    request: HearingFirst, ctx: harmos_runtime::operations::FunctionContext<HearingFirst>,
) -> std::result::Result<harmos_runtime::Response<()>, Failed> {
    let owner = ctx.resource::<OwnerResource>().expect("declared resource is registered");
    let resources = owner.0.get().expect("listener owner configured before registration").clone();
    let _guard = AttemptGuard(resources.clone());
    let request = request.0;
    let result: ::core::result::Result<(), Failed> = async move {
        assert!(matches!(request.context(), DeliveryContext::Stream { .. }));
        resources.seen.lock().unwrap().push((true, request.row().value, request.context()));
        resources.observed.notify_one();
        if true && request.row().value == 0 {
            if resources.reenter {
                resources.streams.publish(fact(9));
            }
            if resources.block {
                resources.started.notify_one();
                resources.resume.notified().await;
            }
            if resources.fail_attempt.load(Ordering::SeqCst) {
                return Err(Failed::Scripted);
            }
        }
        Ok(())
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn hearing_first_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        HEARING_FIRST_OPERATION,
        const { &[harmos_runtime::Resource::of::<OwnerResource>()] },
        hearing_first_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = ())]
#[derive(Clone)]
struct HearingSecond(Delivery<WireFact>);

static HEARING_SECOND_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<HearingSecond>(Heard::<false>::CALLBACK, Heard::<false>::POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const HEARING_SECOND_OPERATION: harmos_runtime::operations::OperationSpec<HearingSecond> =
    harmos_runtime::operations::OperationSpec::<HearingSecond>::new(&HEARING_SECOND_DESCRIPTION);

impl harmos_runtime::Message for HearingSecond {
    const CODEC: &'static str = <Delivery<WireFact> as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, harmos_runtime::MessageError> {
        <Delivery<WireFact> as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> std::result::Result<Self, harmos_runtime::MessageError> {
        <Delivery<WireFact> as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for HearingSecond {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for HearingSecond {
    const NAME: &'static str = <Delivery<WireFact> as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <Delivery<WireFact> as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <Delivery<WireFact> as harmos_runtime::schema::Described>::MARKER;
}
async fn hearing_second_handler(
    request: HearingSecond, ctx: harmos_runtime::operations::FunctionContext<HearingSecond>,
) -> std::result::Result<harmos_runtime::Response<()>, Failed> {
    let owner = ctx.resource::<OwnerResource>().expect("declared resource is registered");
    let resources = owner.0.get().expect("listener owner configured before registration").clone();
    let _guard = AttemptGuard(resources.clone());
    let request = request.0;
    let result: ::core::result::Result<(), Failed> = async move {
        assert!(matches!(request.context(), DeliveryContext::Stream { .. }));
        resources.seen.lock().unwrap().push((false, request.row().value, request.context()));
        resources.observed.notify_one();
        if false && request.row().value == 0 {
            if resources.reenter {
                resources.streams.publish(fact(9));
            }
            if resources.block {
                resources.started.notify_one();
                resources.resume.notified().await;
            }
            if resources.fail_attempt.load(Ordering::SeqCst) {
                return Err(Failed::Scripted);
            }
        }
        Ok(())
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn hearing_second_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        HEARING_SECOND_OPERATION,
        const { &[harmos_runtime::Resource::of::<OwnerResource>()] },
        hearing_second_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl PluginLifecycle for Host {
    type Guest = Resources;

    type Embedded = Resources;
    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Resources {
            owner: Some(self.owner.clone()),
            managed: self.managed,
            ..Resources::default()
        }))
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
        resources.operations =
            Some(self.operations.stage_plugin(scope, |bindings| {
                bindings.catalog(handler_operations_catalog(), harmos_runtime::operations::Lane::default())
            })?);
        if self.managed {
            resources.streams = Some(self.owner.streams.stage_plugin(scope, [publication()]).await.unwrap());
            self.lease = Some(resources.streams.as_ref().unwrap().leases()[0].clone());
            let original = scope.publication("listener.samples").unwrap();
            assert!(Arc::ptr_eq(&original, self.lease.as_ref().unwrap()));
            assert!(matches!(scope.publication("undeclared.samples"), Err(Error::InvalidInput { field: "publication stream", .. })));
        }
        if !self.skip_sources {
            let original = Arc::new(tokio::sync::Mutex::new(self.owner.streams.stage_listeners(scope, &self.operations, |bindings| {
                if self.managed { bindings.publication("listener.samples", 0) } else { bindings.stream::<Fact>(0) }
            })?));
            self.view = Some(original.clone());
            resources.listeners = Some(original);
        }
        // The Same Scope Still Blocks Delivery Until PluginManager Commits.
        if let Some(streams) = &resources.streams {
            streams.leases()[0].stage_derived_row(publication_row(0), 0).unwrap();
        } else {
            self.owner.streams.publish(Fact {
                value: 0,
                dropped: self.original_drop.clone(),
            });
        }
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
            assert!(matches!(scope.publication("listener.samples"), Err(Error::Closing { domain: "publication generation" })));
        }
        resources.streams.take();
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
    vec![<Fact as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap()]
}

/* === Private Functions === */

fn fact(value: u32) -> Fact {
    Fact {
        value,
        dropped: Arc::new(AtomicUsize::new(0)),
    }
}

fn publication() -> harmos_domain::StreamDeclaration {
    harmos_domain::StreamDeclaration {
        metadata: harmos_runtime::Metadata::new(harmos_runtime::Identifier::new("listener.samples").unwrap(), "Samples", None),
        row_metadata: vec![],
        id: "listener.samples".into(),
        window: 512,
        row: harmos_runtime::schema::graph::Schema::from_declaration(
            <Fact as harmos_runtime::schema::Described>::DEFINITION,
            &harmos_runtime::config::SchemaSettings::default(),
        )
        .unwrap(),
    }
}

fn publication_row(value: u32) -> harmos_runtime::streams::PublicationRow {
    harmos_runtime::streams::PublicationRow {
        payload: WireFact { value }.encode(),
        published_unix_micros: None,
        context: None,
    }
}

fn candidate(managed: bool) -> PluginDefinitions {
    let mut listeners = Vec::new();
    for (id, callback) in [
        ("listener.first", HEARING_FIRST_OPERATION.wire().contract().unwrap()),
        ("listener.second", HEARING_SECOND_OPERATION.wire().contract().unwrap()),
    ] {
        listeners.push(
            harmos_domain::listeners::Definition::new(
                harmos_runtime::Identifier::new(id).unwrap(),
                Source::Stream {
                    id: "listener.samples".into(),
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
                operations: CALLBACKS_CATALOG.definitions().unwrap(),
                listeners,
                streams: if managed { vec![publication()] } else { vec![] },
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(block: bool, reenter: bool, tail: usize) -> (Runtime<Model>, PluginManager<Resources, Resources>, Host) {
    setup_source(block, reenter, tail, false).await
}

async fn setup_source(
    block: bool, reenter: bool, tail: usize, managed: bool,
) -> (Runtime<Model>, PluginManager<Resources, Resources>, Host) {
    let runtime = Runtime::<Model>::launch_with((), (), |setup| {
        setup.resource::<OwnerResource>(OwnerResource(std::sync::OnceLock::new()));
        setup.settings().register(if managed { vec![] } else { stream_definitions() });
        setup.settings().tail(tail);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let owner = Arc::new(Owner {
        streams: runtime.streams.clone(),
        seen: Mutex::new(Vec::new()),
        observed: Notify::new(),
        started: Notify::new(),
        resume: Notify::new(),
        settled: Notify::new(),
        finished: AtomicUsize::new(0),
        fail_attempt: AtomicBool::new(false),
        block,
        reenter,
    });
    runtime
        .operations
        .context()
        .resource::<OwnerResource>()
        .unwrap()
        .0
        .set(owner.clone())
        .unwrap_or_else(|_| panic!("listener owner configured once"));
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
    runtime.shutdown().await.unwrap();
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
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn listener_failure_fences() {
    {
        let (runtime, mut manager, mut host) = setup(false, false, 8).await;
        host.owner.fail_attempt.store(true, Ordering::SeqCst);
        manager.initialize(&mut host).await.unwrap();
        observed(&host.owner, 1).await;
        fenced(&host).await;
        assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
        assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::guest::PluginState::Failed);
        assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::OperationFailed { .. })));
        assert!(matches!(runtime.operations.definition(Heard::<false>::CALLBACK), Err(Error::Closing { .. })));
        manager.shutdown(&mut host).await.unwrap();
        runtime.shutdown().await.unwrap();
    }
    {
        let (runtime, mut manager, mut host) = setup(false, false, 8).await;
        manager.initialize(&mut host).await.unwrap();
        observed(&host.owner, 2).await;
        runtime.streams.publish(fact(u32::MAX));
        fenced(&host).await;
        assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::OperationPanicked { .. })));
        assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::guest::PluginState::Failed);
        assert_eq!(host.owner.seen.lock().unwrap().len(), 2);
        manager.shutdown(&mut host).await.unwrap();
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn runtime_signals_listener() {
    struct BlockedService(Arc<Gates>);
    struct Gates {
        running: Notify,
        stopping: Notify,
        release: Notify,
    }
    impl harmos_runtime::services::Resident<Model> for BlockedService {
        type Error = std::io::Error;
        async fn run(&mut self, mut context: harmos_runtime::services::ServiceContext<Model>) -> std::result::Result<(), Self::Error> {
            let gates = &self.0;
            context.ready();
            gates.running.notify_one();
            context.stopping().await;
            gates.stopping.notify_one();
            gates.release.notified().await;
            Ok(())
        }
        async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
            Ok(())
        }
    }
    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    host.owner.started.notified().await;
    let gates = Arc::new(Gates {
        running: Notify::new(),
        stopping: Notify::new(),
        release: Notify::new(),
    });
    runtime
        .operations
        .register_service::<Model, _, _>(
            harmos_runtime::services::ServiceDefinition {
                id: "blocked-cleanup".into(),
                policy: harmos_runtime::services::POLICY,
            },
            {
                let gates = gates.clone();
                move |_| BlockedService(gates.clone())
            },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("blocked-cleanup").await.unwrap();
    gates.running.notified().await;
    let mut stopping = Box::pin(runtime.shutdown());
    tokio::select! {
        _ = async {
            gates.stopping.notified().await;
            host.owner.settled.notified().await;
        } => {},
        result = &mut stopping => panic!("runtime settled before blocked cleanup: {result:?}"),
    }
    assert_eq!(host.owner.finished.load(Ordering::SeqCst), 1);
    drop(stopping);
    gates.release.notify_one();
    runtime.shutdown().await.unwrap();
    drop(lease);
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
    observed(&host.owner, 522).await;

    // The frontier advances only after a row's last callback settles, so one
    // more row's first callback proves every earlier page settled.
    runtime.streams.publish(fact(261));
    observed(&host.owner, 523).await;
    let frontier = host.view.as_ref().unwrap().lock().await.frontier("listener.samples");
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
    runtime.shutdown().await.unwrap();
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
    observed(&host.owner, 1026).await;
    {
        let seen = host.owner.seen.lock().unwrap();
        assert_eq!(seen[2], (true, 9, DeliveryContext::Stream { sequence: 10, missed: 8 }));
        assert_eq!(seen[3], (false, 9, DeliveryContext::Stream { sequence: 10, missed: 8 }));
        assert_eq!(seen.last(), Some(&(false, 520, DeliveryContext::Stream { sequence: 521, missed: 0 })));
    }
    assert!(host.scope.as_ref().unwrap().is_ready());
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
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
    runtime.shutdown().await.unwrap();
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
    assert_eq!(host.view.as_ref().unwrap().lock().await.frontier("listener.samples"), Some(5));
    manager.shutdown(&mut host).await.unwrap();
    assert!(lease.admit_row(publication_row(6), 6, 0).is_err());
    runtime.shutdown().await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn shutdown_releases_callback_before_source() {
    let (runtime, mut manager, mut host) = setup(true, false, 1).await;
    manager.initialize(&mut host).await.unwrap();
    host.owner.started.notified().await;
    manager.shutdown(&mut host).await.unwrap();
    assert_eq!(host.owner.seen.lock().unwrap().len(), 1);
    assert_eq!(host.owner.finished.load(Ordering::SeqCst), 1);
    let source_dropped = host.original_drop.clone();
    assert_eq!(source_dropped.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
    drop((manager, host, runtime));
    assert_eq!(source_dropped.load(Ordering::SeqCst), 1);
}
