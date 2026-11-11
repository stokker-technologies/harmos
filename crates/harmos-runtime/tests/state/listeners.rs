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
use harmos_operations::listeners::{Context as DeliveryContext, Delivery, Source};
use harmos_runtime::guest::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginScope, PluginTransition, Router,
};
use harmos_runtime::operations::{OperationPolicy, PluginRegistration, RetryPolicy};
use harmos_runtime::records::PluginListenerRegistration;
use harmos_runtime::schema::{Described, Field};
use harmos_runtime::{Error, Message, MessageError, Result, Runtime, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources>;
type Seen = Vec<(bool, u32, Position)>;

/* === Structs === */

// Neither The Original Native Record Nor Its Decoded Callback Copy Is Clone.
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
    records: harmos_runtime::records::RecordManager<Model>,
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

fn handler_operations_catalog() -> harmos_runtime::operations::Catalog {
    handler_operations_catalog_batch().named(concat!(module_path!(), "::HandlerOperations"))
}

fn handler_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(hearing_first_operation().expect("representable operation"));
    batch = batch.entry(hearing_second_operation().expect("representable operation"));
    batch
}

enum Rows {
    Fact(Fact),
}
impl From<Fact> for Rows {
    fn from(value: Fact) -> Self {
        Self::Fact(value)
    }
}
impl harmos_runtime::state::CatalogCodec for Rows {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Fact(_) => <Fact as harmos_runtime::state::Record>::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Fact(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
fn rows_registry() -> harmos_runtime::state::RecordRegistry<Rows> {
    harmos_runtime::state::RecordRegistry::new().entry(<Fact as harmos_runtime::state::Record>::ENTRY, Rows::Fact, |value| {
        let Rows::Fact(value) = value;
        Some(value)
    })
}

static CALLBACKS_CATALOG_DECLARATIONS: [harmos_operations::catalog::Node<harmos_operations::operations::WireDeclaration>; 2] = [
    harmos_operations::catalog::Node::Leaf(HEARING_FIRST_OPERATION.wire()),
    harmos_operations::catalog::Node::Leaf(HEARING_SECOND_OPERATION.wire()),
];
static CALLBACKS_CATALOG_CALLEES: [harmos_operations::catalog::Node<&'static [harmos_operations::operations::WireDeclaration]>; 2] = [
    harmos_operations::catalog::Node::Leaf(HEARING_FIRST_OPERATION.callees()),
    harmos_operations::catalog::Node::Leaf(HEARING_SECOND_OPERATION.callees()),
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

impl harmos_runtime::state::Record for Fact {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("listener.fact");
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
    type Record = Rows;
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
        let DeliveryContext::Record { position } = request.context() else { panic!("original record coordinate") };
        resources.seen.lock().unwrap().push((true, request.row().value, position));
        resources.observed.notify_one();
        if true && request.row().value == 0 {
            if resources.reenter {
                resources.records.publish(fact(9)).await?;
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
        let DeliveryContext::Record { position } = request.context() else { panic!("original record coordinate") };
        resources.seen.lock().unwrap().push((false, request.row().value, position));
        resources.observed.notify_one();
        if false && request.row().value == 0 {
            if resources.reenter {
                resources.records.publish(fact(9)).await?;
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
        if !self.skip_sources {
            let original = Arc::new(tokio::sync::Mutex::new(self.owner.records.stage_listeners(
                scope,
                &self.operations,
                Position::ORIGIN,
                |bindings| bindings.record::<Fact>(),
            )?));
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
    Fact {
        value,
        dropped: Arc::new(AtomicUsize::new(0)),
    }
}

fn candidate() -> PluginDefinitions {
    let mut listeners = Vec::new();
    for (id, callback) in [
        ("listener.first", HEARING_FIRST_OPERATION.wire().contract().unwrap()),
        ("listener.second", HEARING_SECOND_OPERATION.wire().contract().unwrap()),
    ] {
        listeners.push(
            harmos_operations::listeners::Definition::new(
                harmos_runtime::Identifier::new(id).unwrap(),
                Source::Record {
                    id: "listener.fact".into(),
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
                ..PluginContributions::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(block: bool, reenter: bool, tail: usize) -> (Runtime<Model>, PluginManager<Resources, Resources>, Host) {
    let runtime = Runtime::<Model>::launch_with((), (), |setup| {
        setup.resource::<OwnerResource>(OwnerResource(std::sync::OnceLock::new()));
        setup.settings().register(rows_registry());
        setup.settings().tail(tail);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let owner = Arc::new(Owner {
        records: runtime.records.clone(),
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
    assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::guest::PluginState::Failed);
    assert!(matches!(host.view.as_ref().unwrap().lock().await.failure().as_deref(), Some(Error::OperationFailed { .. })));
    assert!(matches!(runtime.operations.definition(Heard::<false>::CALLBACK), Err(Error::Closing { .. })));
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
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
    (host.owner.started.notified()).await;
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
    (gates.running.notified()).await;
    let mut stopping = Box::pin(runtime.shutdown());
    (async {
        tokio::select! {
            _ = async {
                gates.stopping.notified().await;
                host.owner.settled.notified().await;
            } => {},
            result = &mut stopping => panic!("runtime settled before blocked cleanup: {result:?}"),
        }
    })
    .await;
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
    runtime.shutdown().await.unwrap();
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
    assert_eq!(manager.status("listener").unwrap().state, harmos_runtime::guest::PluginState::Failed);
    assert_eq!(*host.owner.seen.lock().unwrap(), [(true, 0, Position(1)), (false, 0, Position(1))]);
    manager.shutdown(&mut host).await.unwrap();
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
