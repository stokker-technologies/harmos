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
use std::convert::Infallible;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Lanes, Managed};
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::OperationPhase;
use harmos_runtime::operations::resident::{Key, Kind, Snapshot};
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::sidecar::{
    Attach, DemandKey, ObserverAttachment, ObserverContext, PublicationError, PublishError, Publisher, Supervisor,
};

/* === Constants === */

const RECORD: DemandKey = DemandKey::new("fixture.record", "sample");
const AUXILIARY: DemandKey = DemandKey::new("fixture", "auxiliary");
const IDLE: DemandKey = DemandKey::new("fixture", "idle");
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Counts>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Counts {
    prepared: AtomicUsize,
    initialized: AtomicUsize,
    started: [AtomicUsize; 2],
    observed: [AtomicUsize; 2],
    stopped: [AtomicUsize; 2],
    cleaned: [AtomicUsize; 2],
    block_observe: AtomicBool,
    collect_callback: AtomicBool,
    collecting: AtomicUsize,
    collected: AtomicUsize,
    rejected: AtomicUsize,
    released: AtomicUsize,
    // Announces every counted step.
    progress: tokio::sync::watch::Sender<()>,
}

struct Seed {
    id: usize,
}

#[harmos_runtime::stream(id = "demand.row", window = 16)]
#[harmos_runtime::message]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Row {
    sample: u64,
}

struct Resources {
    counts: Arc<Counts>,
}

struct Producer {
    resources: Arc<Resources>,
    attachment: Arc<ObserverAttachment<Publisher<Producer>>>,
}

struct RecordOnly(Arc<Counts>);

struct Mixed(Arc<Counts>);

struct Idle(Arc<Counts>);

#[harmos_runtime::operation(id = "demand.collect", policy = COLLECTION, resources = [Resources], response = ())]
struct Collect {}

struct Collecting(Arc<Counts>);

/* === Enums === */

#[harmos_domain::catalog]
enum Observers {
    RecordOnly(RecordOnly),
    Mixed(Mixed),
    Idle(Idle),
}

#[harmos_runtime::catalog]
enum Operations {
    Collect(Collect),
}

/* === Traits === */

/* === Implementations === */

impl Seed {
    fn new(counts: &Arc<Counts>) -> Self {
        let id = NEXT_SEED.fetch_add(1, SeqCst);
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(counts)).is_none());
        Self { id }
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl Drop for Resources {
    fn drop(&mut self) {
        self.counts.released.fetch_add(1, SeqCst);
        self.counts.progress.send_replace(());
    }
}

impl Drop for Collecting {
    fn drop(&mut self) {
        self.0.collected.fetch_add(1, SeqCst);
        self.0.progress.send_replace(());
    }
}

impl harmos_runtime::Operation for Collect {
    type Error = Infallible;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let resources = context.resource::<Resources>();
        let counts = Arc::clone(&resources.counts);
        let _collecting = Collecting(Arc::clone(&counts));
        counts.collecting.fetch_add(1, SeqCst);
        counts.progress.send_replace(());
        std::future::pending().await
    }
}

const LIFECYCLE: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    retries: RetryPolicy::Never,
};

const COLLECTION: OperationPolicy = OperationPolicy {
    timeout: LIFECYCLE.timeout.saturating_mul(4),
    attempt_timeout: LIFECYCLE.timeout.saturating_mul(4),
    retries: RetryPolicy::Never,
};

impl harmos_operations::lifecycle::Policy for Producer {
    const INITIALIZATION_POLICY: OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: OperationPolicy = LIFECYCLE;
}

impl harmos_runtime::Lifecycle for Producer {
    type Config = usize;
    type Context = Supervisor;
    type Error = harmos_runtime::sidecar::Refusal;

    async fn prepare(id: usize, setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Self::Error> {
        let counts = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|counts| counts.upgrade())
            .ok_or_else(|| harmos_runtime::sidecar::Refusal::of("original fixture resources unavailable"))?;
        counts.prepared.fetch_add(1, SeqCst);
        counts.progress.send_replace(());
        let resources = Arc::new(Resources { counts });
        let attachment = ObserverAttachment::from_catalog::<Observers>(<Self as harmos_domain::plugin::Declared>::STREAMS)?;
        let plugin = Self { resources, attachment };
        setup.resource::<Resources>(plugin.resources.clone());
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        self.resources.counts.initialized.fetch_add(1, SeqCst);
        self.resources.counts.progress.send_replace(());
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        for index in 0..2 {
            assert_eq!(self.resources.counts.started[index].load(SeqCst), self.resources.counts.stopped[index].load(SeqCst));
        }
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "demand_authoring", requires(harmos_operations::streams::Publish),  observers = Observers, streams(Row))]
impl harmos_runtime::Plugin for Producer {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Operations::CATALOG];

    fn attachment(&self) -> Option<Arc<dyn harmos_sdk::plugin::Attachment<Supervisor>>> {
        Some(self.attachment.clone())
    }
}

#[harmos_runtime::observer(id = "demand.record-only", interval_ms = 5, activation = "demand", demand_keys(RECORD))]
impl RecordOnly {
    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, Infallible> {
        let resolved = context.resource::<Resources>().unwrap();
        let resources = resolved.as_ref();
        resources.counts.started[0].fetch_add(1, SeqCst);
        resources.counts.progress.send_replace(());
        Ok(Self(resources.counts.clone()))
    }
    async fn observe(&mut self, ctx: &ObserverContext<'_, Publisher<Producer>>) -> Result<(), Infallible> {
        assert!(matches!(
            ctx.publish_on::<Row>(&Row { sample: 0 }).await,
            Err(PublishError::Publication(PublicationError::Undeclared { .. }))
        ));
        ctx.resource::<Resources>().unwrap().counts.rejected.fetch_add(1, SeqCst);
        ctx.resource::<Resources>().unwrap().counts.progress.send_replace(());
        ctx.resource::<Resources>().unwrap().counts.observed[0].fetch_add(1, SeqCst);
        ctx.resource::<Resources>().unwrap().counts.progress.send_replace(());
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Infallible> {
        self.0.stopped[0].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());

        Ok(())
    }
}

#[harmos_runtime::observer(id = "demand.mixed", interval_ms = 5, activation = "demand", streams(Row), demand_keys(RECORD, AUXILIARY))]
impl Mixed {
    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, Infallible> {
        let resolved = context.resource::<Resources>().unwrap();
        let resources = resolved.as_ref();
        resources.counts.started[1].fetch_add(1, SeqCst);
        resources.counts.progress.send_replace(());
        Ok(Self(resources.counts.clone()))
    }
    async fn observe(&mut self, ctx: &ObserverContext<'_, Publisher<Producer>>) -> Result<(), Infallible> {
        if self.0.collect_callback.load(SeqCst) {
            ctx.host().supervisor().call::<Collect>(Collect {}).await.unwrap();
        }
        if self.0.block_observe.load(SeqCst) {
            self.0.observed[1].fetch_add(1, SeqCst);
            self.0.progress.send_replace(());
            std::future::pending::<()>().await;
        }
        ctx.publish_on::<Row>(&Row { sample: 1 }).await.unwrap();
        ctx.resource::<Resources>().unwrap().counts.observed[1].fetch_add(1, SeqCst);
        ctx.resource::<Resources>().unwrap().counts.progress.send_replace(());
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Infallible> {
        self.0.stopped[1].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());

        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Infallible> {
        self.0.cleaned[1].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());
        Ok(())
    }
}

#[harmos_runtime::observer(id = "demand.idle", interval_ms = LIFECYCLE.timeout.as_millis() as u64 * 4, activation = "demand", demand_keys(IDLE))]
impl Idle {
    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, Infallible> {
        let resources = context.resource::<Resources>().unwrap();
        let counts = Arc::clone(&resources.counts);
        counts.started[1].fetch_add(1, SeqCst);
        counts.progress.send_replace(());
        Ok(Self(counts))
    }

    async fn observe(&mut self, _: &ObserverContext<'_, Publisher<Producer>>) -> Result<(), Infallible> {
        self.0.observed[1].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Infallible> {
        self.0.stopped[1].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Infallible> {
        self.0.cleaned[1].fetch_add(1, SeqCst);
        self.0.progress.send_replace(());
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn observers(runtime: &harmos_runtime::Runtime<crate::sidecar::support::Bank>) -> std::collections::BTreeMap<Key, Snapshot> {
    runtime.operations.residents().into_iter().filter(|(_, status)| status.kind == Kind::Observer).collect()
}

async fn until(counts: &Counts, predicate: impl Fn() -> bool) {
    counts.progress.subscribe().wait_for(|_| predicate()).await.expect("observer demand converges");
}

#[tokio::test]
async fn demand_survives_reload() {
    let counts = Arc::new(Counts::default());
    let seed = Seed::new(&counts);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Producer>::new(capacity).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::Open,
        capacity,
        vec![RECORD.name(), AUXILIARY.name()],
        std::convert::identity,
    )
    .await;
    let demands = fixture.demands();
    let first = demands.hold(&RECORD.name()).unwrap();
    assert_eq!(counts.prepared.load(SeqCst), 0);
    fixture.publication().await.unwrap();
    fixture.initialize().await.unwrap();
    fixture.ready().wait_for(|ready| *ready).await.unwrap();
    assert!(!SEEDS.lock().unwrap().contains_key(&seed.id));
    assert_eq!(counts.prepared.load(SeqCst), 1);
    until(&counts, || counts.observed.iter().all(|count| count.load(SeqCst) > 0)).await;
    assert!(counts.rejected.load(SeqCst) > 0, "record demand grants no stream publication");
    assert_eq!(counts.initialized.load(SeqCst), 1);
    assert_eq!(counts.started[0].load(SeqCst), 1);
    assert_eq!(counts.started[1].load(SeqCst), 1);
    let first_statuses = observers(&fixture.runtime);
    assert_eq!(first_statuses.len(), 2);

    let second = demands.hold(&RECORD.name()).unwrap();
    let auxiliary = demands.hold(&AUXILIARY.name()).unwrap();
    let stream = fixture.runtime.streams.publication("demand.row", 1).unwrap();
    let rows = stream.watch();
    drop(first);
    let before = counts.observed[0].load(SeqCst);
    until(&counts, || counts.observed[0].load(SeqCst) >= before + 3).await;
    assert_eq!(counts.stopped[0].load(SeqCst), 0);
    drop(second);
    until(&counts, || counts.stopped[0].load(SeqCst) == 1).await;
    assert_eq!(counts.stopped[1].load(SeqCst), 0);

    drop(rows);
    let before = counts.observed[1].load(SeqCst);
    until(&counts, || counts.observed[1].load(SeqCst) >= before + 3).await;
    assert_eq!(counts.started[1].load(SeqCst), 1);
    assert_eq!(counts.stopped[1].load(SeqCst), 0);
    let current = observers(&fixture.runtime);
    for (key, status) in &first_statuses {
        assert_eq!(current[key].instance, status.instance);
    }
    drop(auxiliary);
    until(&counts, || counts.stopped.iter().all(|count| count.load(SeqCst) == 1)).await;
    let mut statuses = fixture.runtime.operations.watch_residents();
    statuses
        .wait_for(|values| {
            let observers: Vec<_> = values.values().filter(|status| status.kind == Kind::Observer).collect();
            observers.len() == 2 && observers.iter().all(|status| status.lifecycle.phase() == OperationPhase::Stopped)
        })
        .await
        .unwrap();

    let held = demands.hold(&RECORD.name()).unwrap();
    until(&counts, || counts.started.iter().all(|count| count.load(SeqCst) == 2)).await;
    fixture.shutdown().await.unwrap();
    assert_eq!(counts.released.load(SeqCst), 1, "shutdown releases the original observer resources before reload");
    assert!(fixture.runtime.operations.resident_definitions().iter().all(|definition| definition.kind() != Kind::Observer));
    assert_eq!(counts.stopped[0].load(SeqCst), 2);
    assert_eq!(counts.stopped[1].load(SeqCst), 2);
    let replacement_seed = Seed::new(&counts);
    fixture.reload(serde_json::to_vec(&replacement_seed.id).unwrap()).await.unwrap();
    assert_eq!(counts.prepared.load(SeqCst), 2);
    until(&counts, || counts.started.iter().all(|count| count.load(SeqCst) == 3)).await;
    assert_eq!(counts.initialized.load(SeqCst), 2);
    let replacement = observers(&fixture.runtime);
    assert!(replacement.keys().all(|key| !first_statuses.contains_key(key)), "reload gets fresh resource scopes");
    drop(held);
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.released.load(SeqCst), 2, "the replacement resources are released before shutdown returns");
    assert_eq!(counts.stopped[0].load(SeqCst), 3);
    assert_eq!(counts.stopped[1].load(SeqCst), 3);
}

#[test]
fn catalog_keys_isolated() {
    let counts = Arc::new(Counts::default());
    let seed = Seed::new(&counts);
    let declaration = harmos_sidecar::PluginNative::<Producer>::new(std::num::NonZeroUsize::new(2).unwrap()).unwrap();
    assert!(SEEDS.lock().unwrap().contains_key(&seed.id));
    assert_eq!(counts.prepared.load(SeqCst), 0);
    let observers = declaration.observers();
    harmos_runtime::sidecar::inventory::observers::validate(&observers, &declaration.publications()).unwrap();
    let record = observers.iter().find(|observer| observer.id == "demand.record-only").unwrap();
    assert!(record.streams.is_empty());
    assert_eq!(record.demand_keys, [RECORD.name()]);
    let mixed = observers.iter().find(|observer| observer.id == "demand.mixed").unwrap();
    assert_eq!(mixed.streams, ["demand.row"]);
    assert_eq!(mixed.demand_keys, [RECORD.name(), AUXILIARY.name()]);
}

async fn release_suspended_observer(key: DemandKey, blocked_callback: bool) {
    let counts = Arc::new(Counts::default());
    counts.block_observe.store(blocked_callback, SeqCst);
    let seed = Seed::new(&counts);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Producer>::new(capacity).unwrap();
    let configuration = serde_json::to_vec(&seed.id).unwrap();
    let mut fixture = Managed::attach(source, configuration, Lanes::Open, capacity, vec![key.name()], std::convert::identity).await;

    fixture.publication().await.unwrap();
    fixture.initialize().await.unwrap();
    let ready = fixture.ready();
    ready.wait_for(|ready| *ready).await.unwrap();
    let demands = fixture.demands();
    let demand = demands.hold(&key.name()).unwrap();
    let initialized = || counts.started[1].load(SeqCst) == 1;
    until(&counts, initialized).await;
    if blocked_callback {
        let entered = || counts.observed[1].load(SeqCst) == 1;
        until(&counts, entered).await;
    }

    drop(demand);
    let settling = async {
        let cleaned = || counts.cleaned[1].load(SeqCst) == 1;
        until(&counts, cleaned).await;
        let mut statuses = fixture.runtime.operations.watch_residents();
        let stopped = statuses.wait_for(|values| {
            let mut observers = values.values().filter(|status| status.kind == Kind::Observer);
            let Some(observer) = observers.next() else { return false };
            observer.lifecycle.phase() == OperationPhase::Stopped && observers.next().is_none()
        });
        stopped.await.unwrap();
    };
    tokio::time::timeout(LIFECYCLE.timeout, settling).await.expect("released observer settles within the lifecycle policy");
    assert_eq!(counts.stopped[1].load(SeqCst), 1);
    assert_eq!(counts.observed[1].load(SeqCst), usize::from(blocked_callback));

    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.released.load(SeqCst), 1);
}

/* === Tests === */

#[tokio::test]
async fn released_demand_interrupts_blocked_callback() {
    release_suspended_observer(AUXILIARY, true).await;
}

#[tokio::test]
async fn released_demand_interrupts_tick_wait() {
    release_suspended_observer(IDLE, false).await;
}

#[tokio::test]
async fn released_publication_interrupts_collect_and_retains_stopped_snapshot() {
    let counts = Arc::new(Counts::default());
    counts.collect_callback.store(true, SeqCst);
    let seed = Seed::new(&counts);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Producer>::new(capacity).unwrap();
    let configuration = serde_json::to_vec(&seed.id).unwrap();
    let mut fixture = Managed::attach(source, configuration, Lanes::Open, capacity, vec![], std::convert::identity).await;
    fixture.publication().await.unwrap();
    fixture.initialize().await.unwrap();
    let ready = fixture.ready();
    ready.wait_for(|ready| *ready).await.unwrap();
    let before = observers(&fixture.runtime);
    assert!(before.is_empty());

    // A Viewer Holds An Unpolled Publication Watch, Like Dokime's Stream Lease.
    let publication = fixture.runtime.streams.publication("demand.row", 1).unwrap();
    let watching = Box::pin(publication.watch());
    let collecting = || counts.collecting.load(SeqCst) == 1;
    tokio::time::timeout(LIFECYCLE.timeout, until(&counts, collecting)).await.expect("observer enters its collect call");
    let running = observers(&fixture.runtime);
    assert_eq!(running.len(), 1);
    let key = running.into_keys().next().unwrap();
    assert_eq!(key.definition, "demand.mixed");

    drop(watching);
    let mut residents = fixture.runtime.operations.watch_residents();
    let settling = residents.wait_for(|values| {
        let Some(status) = values.get(&key) else { return false };
        status.lifecycle.phase() == OperationPhase::Stopped
    });
    let stopped = tokio::time::timeout(LIFECYCLE.timeout, settling).await.expect("publication release settles within policy").unwrap();
    assert_eq!(stopped[&key].leases, 0);
    assert!(stopped[&key].lifecycle.cleanup_failure().is_none());
    drop(stopped);
    assert_eq!(counts.stopped[1].load(SeqCst), 1);
    assert_eq!(counts.cleaned[1].load(SeqCst), 1);
    assert_eq!(counts.collected.load(SeqCst), 1);

    // Resident History Remains Until Scope Retirement, Even After Physical Stop.
    assert!(residents.borrow().contains_key(&key));
    fixture.shutdown().await.unwrap();
    assert!(!residents.borrow().contains_key(&key));
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.released.load(SeqCst), 1);
}
