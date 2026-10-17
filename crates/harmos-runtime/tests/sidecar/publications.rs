/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "publication_delivery.rs"]
mod publication_delivery;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
use std::pin::Pin;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::operations::{Lanes, Managed};
use super::*;
use harmos_runtime::{storage::Batch, storage::Cursor, storage::StreamConsumer};
use harmos_domain::plugin::Declared;
use harmos_sidecar::{Attach, ObserverContext};

/* === Constants === */
// 2025-10-09 Around 08:53 UTC In Microseconds
const TIMED_BASE: i64 = 1_760_000_000_000_000;
const TICKING_DEMAND: harmos_sidecar::DemandKey =
    harmos_sidecar::DemandKey::new("ticking", "pulse");
// Each Tick Publishes A Batch Wider Than The Flood Window
const FLOOD_BATCH: u64 = 12;
const FLOOD_ROWS: u64 = 240;
static NEXT_GATE_SEED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
static GATE_SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<Gate>>> =
    Mutex::new(std::collections::BTreeMap::new());
static NEXT_OBSERVER_SEED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
static OBSERVER_SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<ObserverCounts>>> =
    Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */
type CapturedRows = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/* === Structs === */
struct Recorder {
    cursor: Cursor,
    rows: CapturedRows,
}

#[derive(Clone, Default, serde::Deserialize)]
struct Gate {
    #[serde(skip)]
    started: Arc<Notify>,
    #[serde(skip)]
    release: Arc<Notify>,
}

#[harmos_runtime::stream(id = "ticking.pulse", window = 8, row = support::Pulse)]
struct TickingPulse;

struct Ticking {
    resources: Arc<Gate>,
    attachment: Arc<harmos_sidecar::ObserverAttachment<Gate, harmos_sidecar::Publisher<Ticking>>>,
}

struct GateSeed {
    id: usize,
    _gate: Arc<Gate>,
}

#[derive(Default)]
struct Producer;

#[harmos_runtime::stream(id = "twins.left", window = 16, row = support::Pulse)]
struct Left;

#[harmos_runtime::stream(id = "twins.right", window = 16, row = support::Pulse)]
struct Right;

struct Twins {
    resources: Arc<()>,
}

#[harmos_runtime::stream(id = "batches.rows", window = 16, row = support::Pulse)]
struct BatchedRows;

#[harmos_runtime::stream(id = "batches.raw", window = 16, row = support::Pulse)]
struct RawRows;

#[harmos_runtime::stream(id = "batches.timed", window = 1024, row = support::Pulse)]
struct TimedRows;

#[harmos_runtime::stream(id = "batches.clocked", window = 1024, row = support::Pulse)]
struct ClockedRows;

#[harmos_runtime::stream(id = "batches.stamped", window = 16, row = support::Pulse)]
struct StampedRows;

struct Batches {
    resources: Arc<()>,
}

#[harmos_runtime::stream(id = "flood.rows", window = 8, row = support::Pulse)]
struct FloodRows;

struct Flood {
    resources: Arc<super::witness::Witness>,
    attachment: Arc<harmos_sidecar::ObserverAttachment<super::witness::Witness, harmos_sidecar::Publisher<Flood>>>,
}

struct Flooding {
    next: u64,
    witness: Arc<super::witness::Witness>,
}

#[derive(Clone, Default, serde::Deserialize)]
struct ObserverCounts {
    #[serde(skip)]
    coordinated: Arc<std::sync::atomic::AtomicBool>,
    #[serde(skip)]
    stop_panics: Arc<std::sync::atomic::AtomicBool>,
    #[serde(skip)]
    stop_attempts: Arc<std::sync::atomic::AtomicUsize>,
    #[serde(skip)]
    initialized: Arc<std::sync::atomic::AtomicUsize>,
    #[serde(skip)]
    started: Arc<[std::sync::atomic::AtomicUsize; 2]>,
    #[serde(skip)]
    stopped: Arc<[std::sync::atomic::AtomicUsize; 2]>,
    #[serde(skip)]
    stopped_changed: Arc<tokio::sync::Notify>,
    #[serde(skip)]
    released: Arc<std::sync::atomic::AtomicUsize>,
}

struct SampledTwins {
    resources: Arc<ObserverResources>,
    attachment: Arc<
        harmos_sidecar::ObserverAttachment<
            ObserverResources,
            harmos_sidecar::Publisher<SampledTwins>,
        >,
    >,
}

struct ObserverSeed {
    id: usize,
    _counts: Arc<ObserverCounts>,
}


struct ObserverResources {
    counts: ObserverCounts,
}

struct LeftObserver(ObserverCounts);

struct RightObserver(ObserverCounts);

#[derive(Default)]
struct MultiStreamObserver;

struct MultiStream {
    resources: Arc<()>,
    attachment: Arc<harmos_sidecar::ObserverAttachment<(), harmos_sidecar::Publisher<MultiStream>>>,
}

#[derive(Clone)]
struct Advertised<S> {
    inner: S,
    observers: Vec<harmos_sidecar::inventory::observers::Declaration>,
    attached: Arc<std::sync::atomic::AtomicUsize>,
}

/* === Enums === */
#[harmos_runtime::catalog]
enum TickingObservers {
    Producer(Producer),
}

#[harmos_runtime::catalog]
enum TwinObservers {
    Left(LeftObserver),
    Right(RightObserver),
}

#[harmos_runtime::catalog]
enum MultiObservers {
    Producer(MultiStreamObserver),
}

#[harmos_runtime::catalog]
enum FloodObservers {
    Producer(Flooding),
}

/* === Traits === */

/* === Implementations === */
impl<S: harmos_sidecar::Attach> harmos_sidecar::Attach for Advertised<S> {
    type Error = S::Error;

    fn owner(&self) -> &str {
        self.inner.owner()
    }
    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        self.inner.publications()
    }
    fn observers(&self) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
        self.observers.clone()
    }

    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::Definition>> {
        self.inner.operations()
    }
    fn operation_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        self.inner.operation_contracts()
    }
    fn required_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.inner.required_contracts()
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
        self.inner.sdk_lifecycle()
    }
    fn observe_contract(&self) -> harmos_domain::Result<Option<harmos_runtime::operations::Contract>> {
        self.inner.observe_contract()
    }
    fn listener_declarations(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
        self.inner.listener_declarations()
    }

    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        self.attached
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.attach().await
    }
}

impl StreamConsumer<Bank> for Recorder {
    fn resume(&self) -> Cursor {
        self.cursor
    }

    async fn drain(&mut self, batch: Batch<Bank>) -> Cursor {
        self.rows.lock().unwrap().extend(
            batch.publications().map(|(definition, _, row)| {
                (definition.id().to_owned(), row.value.payload.clone())
            }),
        );
        self.cursor = batch.reaches();
        self.cursor
    }
}

impl GateSeed {
    fn new(gate: &Gate) -> Self {
        use std::sync::atomic::Ordering::SeqCst;
        let id = NEXT_GATE_SEED
            .try_update(SeqCst, SeqCst, |id| id.checked_add(1))
            .unwrap();
        let gate = Arc::new(gate.clone());
        let mut seeds = GATE_SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(&gate)).is_none());
        Self { id, _gate: gate }
    }
}

impl Drop for GateSeed {
    fn drop(&mut self) {
        GATE_SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Ticking {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Observer Demand During SDK Initialization.", id = "ticking", provides = (), requires(harmos_runtime::operations::streams::Publish),
    observers = TickingObservers, streams(TickingPulse))]
impl harmos_runtime::Plugin for Ticking {
    type Config = usize;
    type Context = harmos_sidecar::Supervisor;
    type Resources = Gate;
    type Error = harmos_sidecar::Refusal;
    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let resources = GATE_SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|gate| gate.upgrade())
            .ok_or_else(|| harmos_sidecar::Refusal::of("original fixture gate unavailable"))?;
        let attachment = harmos_sidecar::ObserverAttachment::from_catalog::<TickingObservers>(
            resources.clone(),
            Self::STREAMS,
        )?;
        Ok(Self {
            resources,
            attachment,
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.resources.started.notify_one();
        self.resources.release.notified().await;
        Ok(())
    }
    fn resources(&self) -> Arc<Gate> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        self.attachment.bind(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_domain::plugin::Attachment<Self::Context>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[harmos_runtime::observer(
    streams(TickingPulse),
    demand_keys(TICKING_DEMAND),
    activation = "demand",
    interval_ms = 1
)]
impl Producer {
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Gate, harmos_sidecar::Publisher<Ticking>>,
    ) -> Result<(), Infallible> {
        match ctx
            .publish_on::<TickingPulse>(&support::pulse(ctx.tick().ordinal()))
            .await
        {
            // A Retiring Generation May Tick Once After The Host Closes Its Admission.
            Ok(())
            | Err(harmos_sidecar::PublishError::Call(harmos_sidecar::CallError::Refused {
                code: -32_606,
                ..
            })) => Ok(()),
            Err(error) => panic!("ticking publication failed: {error}"),
        }
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Twins {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy =
        harmos_runtime::operations::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_millis(500),
            retries: harmos_runtime::operations::RetryPolicy::Never,
            restart: harmos_runtime::operations::RestartPolicy::Never,
        };
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Two Streams Share One Row Shape.", id = "twins", provides = (), requires(harmos_runtime::operations::streams::Publish),  observers = (), streams(Left),
    stream_entries = &[harmos_sidecar::inventory::publications::Entry::of::<Right>()])]
impl harmos_runtime::Plugin for Twins {
    type Config = ();
    type Context = harmos_sidecar::Supervisor;
    type Resources = ();
    type Error = harmos_sidecar::Refusal;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self {
            resources: Arc::new(()),
        })
    }
    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Self::Error> {
        context
            .publisher::<Self>()
            .publish_on_context::<Left>(&support::pulse(10), Some("execution-a"))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        context
            .publisher::<Self>()
            .publish_on::<Right>(&support::pulse(20))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        context
            .publisher::<Self>()
            .publish_on_context::<Left>(&support::pulse(11), Some("execution-b"))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Batches {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy = Twins::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Publishes Ordered Row Batches.", id = "batches", provides = (), requires(harmos_runtime::operations::streams::Publish), observers = (), streams(BatchedRows, RawRows, TimedRows, ClockedRows, StampedRows))]
impl harmos_runtime::Plugin for Batches {
    type Config = ();
    type Context = harmos_sidecar::Supervisor;
    type Resources = ();
    type Error = harmos_sidecar::Refusal;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self {
            resources: Arc::new(()),
        })
    }
    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Self::Error> {
        let publisher = context.publisher::<Self>();
        let pulses = |samples: std::ops::RangeInclusive<u64>| {
            let mut batch = publisher.batch::<BatchedRows>();
            for sample in samples {
                batch.push(support::pulse(sample));
            }
            batch
        };
        publisher
            .publish_batch_on_context::<BatchedRows>(&pulses(1..=3), Some("execution-a"))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        publisher
            .publish_on::<BatchedRows>(&support::pulse(4))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        publisher
            .publish_batch_on::<BatchedRows>(&publisher.batch())
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        publisher
            .publish_batch_on_context::<BatchedRows>(&pulses(5..=6), Some("execution-b"))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        let mut undeclared = publisher.batch::<Left>();
        undeclared.push(support::pulse(1));
        let undeclared = publisher.publish_batch_on::<Left>(&undeclared).await;
        if !matches!(
            undeclared,
            Err(harmos_sidecar::PublishError::Publication(
                harmos_sidecar::PublicationError::Undeclared {
                    stream: "twins.left"
                }
            ))
        ) {
            return Err(harmos_sidecar::Refusal::of(
                "undeclared batch was not refused",
            ));
        }
        // Explicit Stamps: 100 Rows A Millisecond Apart Over 100 ms
        let mut timed = publisher.batch::<TimedRows>();
        for sample in 0..100 {
            timed.push_at(support::pulse(sample), TIMED_BASE + 1_000 * sample as i64);
        }
        publisher
            .publish_batch_on::<TimedRows>(&timed)
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        // Source Stamps Survive The Native Wire And Retained Window
        let mut clocked = publisher.batch::<ClockedRows>();
        for sample in 0..100 {
            clocked.push_at(support::pulse(sample), TIMED_BASE + 1_000 * sample as i64);
        }
        publisher
            .publish_batch_on::<ClockedRows>(&clocked)
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        // A Refused Row Ends Its Batch; The Rows Before It Stay Published.
        let raw = |stream: &str,
                   sequence: u64,
                   published_unix_micros: Option<i64>,
                   following: Vec<(Vec<u8>, Option<i64>)>| {
            harmos_runtime::operations::streams::Publish {
                stream: stream.to_owned(),
                schema: 1,
                sequence: Some(sequence),
                published_unix_micros,
                context: None,
                payload: harmos_runtime::Message::encode_to_vec(&support::pulse(sequence)),
                following: following
                    .into_iter()
                    .map(|(payload, published_delta_micros)| {
                        harmos_runtime::operations::streams::FollowingRow {
                            payload,
                            published_delta_micros,
                        }
                    })
                    .collect(),
            }
        };
        let pulse = |sample: u64| harmos_runtime::Message::encode_to_vec(&support::pulse(sample));
        let publish = |request| harmos_runtime::operations::Host::call(context, request);
        let raw_id = <RawRows as harmos_runtime::StreamSpec>::ID;
        let refused = publish(raw(
            raw_id,
            1,
            None,
            vec![(pulse(2), None), (vec![0xff; 3], None), (pulse(4), None)],
        ))
        .await;
        if refused.is_ok() {
            return Err(harmos_sidecar::Refusal::of(
                "a malformed batch row was admitted",
            ));
        }
        // Only The Staged Prefix Advanced The Producer Sequence.
        publish(raw(raw_id, 3, None, Vec::new()))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        // A Delta Without A Stamped Predecessor Refuses The Whole Call
        let stamped_id = <StampedRows as harmos_runtime::StreamSpec>::ID;
        if publish(raw(stamped_id, 1, None, vec![(pulse(2), Some(1_000))]))
            .await
            .is_ok()
        {
            return Err(harmos_sidecar::Refusal::of(
                "a delta without a base was admitted",
            ));
        }
        // A Row Stamped Before Its Predecessor Is Refused At That Row
        let backwards = raw(
            stamped_id,
            1,
            Some(TIMED_BASE),
            vec![
                (pulse(2), Some(1_000)),
                (pulse(3), Some(0)),
                (pulse(4), Some(-1)),
                (pulse(5), Some(1_000)),
            ],
        );
        if publish(backwards).await.is_ok() {
            return Err(harmos_sidecar::Refusal::of(
                "a backwards batch row was admitted",
            ));
        }
        publish(raw(stamped_id, 4, Some(TIMED_BASE), Vec::new()))
            .await
            .map_err(harmos_sidecar::Refusal::from)?;
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ObserverCounts {
    async fn until_stopped(&self, index: usize) {
        loop {
            let changed = self.stopped_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.stopped[index].load(std::sync::atomic::Ordering::SeqCst) != 0 {
                return;
            }
            changed.await;
        }
    }
}

impl Drop for ObserverResources {
    fn drop(&mut self) {
        self.counts
            .released
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl ObserverSeed {
    fn new(counts: &ObserverCounts) -> Self {
        use std::sync::atomic::Ordering::SeqCst;
        let id = NEXT_OBSERVER_SEED
            .try_update(SeqCst, SeqCst, |id| id.checked_add(1))
            .unwrap();
        let counts = Arc::new(counts.clone());
        let mut seeds = OBSERVER_SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(&counts)).is_none());
        Self {
            id,
            _counts: counts,
        }
    }
}

impl Drop for ObserverSeed {
    fn drop(&mut self) {
        OBSERVER_SEEDS.lock().unwrap().remove(&self.id);
    }
}

const LIFECYCLE: harmos_runtime::operations::OperationPolicy = harmos_runtime::operations::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    retries: harmos_runtime::operations::RetryPolicy::Never,
    restart: harmos_runtime::operations::RestartPolicy::Never,
};

impl harmos_runtime::operations::lifecycle::Policy for SampledTwins {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Independent Observers Sharing The Original Initialized Resources.", id = "twin-observers", provides = (), requires(harmos_runtime::operations::streams::Publish),
    observers = TwinObservers, streams(Left, Right))]
impl harmos_runtime::Plugin for SampledTwins {
    type Config = usize;
    type Context = harmos_sidecar::Supervisor;
    type Resources = ObserverResources;
    type Error = harmos_sidecar::Refusal;

    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let counts = OBSERVER_SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|counts| counts.upgrade())
            .ok_or_else(|| harmos_sidecar::Refusal::of("original fixture resources unavailable"))?;
        let resources = Arc::new(ObserverResources {
            counts: (*counts).clone(),
        });
        let attachment = harmos_sidecar::ObserverAttachment::from_catalog::<TwinObservers>(
            resources.clone(),
            <Self as harmos_domain::plugin::Declared>::STREAMS,
        )?;
        Ok(Self {
            resources,
            attachment,
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.resources
            .counts
            .initialized
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn resources(&self) -> Arc<ObserverResources> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        self.attachment.bind(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_domain::plugin::Attachment<Self::Context>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        for index in 0..2 {
            assert_eq!(
                self.resources.counts.started[index].load(std::sync::atomic::Ordering::SeqCst),
                self.resources.counts.stopped[index].load(std::sync::atomic::Ordering::SeqCst)
            );
        }
        Ok(())
    }
}

#[harmos_runtime::observer(streams(Left), activation = "demand", interval_ms = 5)]
impl LeftObserver {
    async fn initialize(resources: &ObserverResources) -> Result<Self, Infallible> {
        resources.counts.started[0].fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self(resources.counts.clone()))
    }
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, ObserverResources, harmos_sidecar::Publisher<SampledTwins>>,
    ) -> Result<(), Infallible> {
        ctx.publish_on::<Left>(&support::pulse(101)).await.unwrap();
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Infallible> {
        self.0
            .stop_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert!(
            !self.0.stop_panics.load(std::sync::atomic::Ordering::SeqCst),
            "observer cleanup panic"
        );
        self.0.stopped[0].fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.0.stopped_changed.notify_waiters();
        if self.0.coordinated.load(std::sync::atomic::Ordering::SeqCst) {
            self.0.until_stopped(0).await;
            self.0.until_stopped(1).await;
        }

        Ok(())
    }
}

#[harmos_runtime::observer(streams(Right), activation = "demand", interval_ms = 5)]
impl RightObserver {
    async fn initialize(resources: &ObserverResources) -> Result<Self, Infallible> {
        resources.counts.started[1].fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self(resources.counts.clone()))
    }
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, ObserverResources, harmos_sidecar::Publisher<SampledTwins>>,
    ) -> Result<(), Infallible> {
        ctx.publish_on::<Right>(&support::pulse(201)).await.unwrap();
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Infallible> {
        self.0.stopped[1].fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.0.stopped_changed.notify_waiters();
        if self.0.coordinated.load(std::sync::atomic::Ordering::SeqCst) {
            self.0.until_stopped(0).await;
            self.0.until_stopped(1).await;
        }

        Ok(())
    }
}

#[harmos_runtime::observer(interval_ms = 5, streams(Left, Right), activation = "demand")]
impl MultiStreamObserver {
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, (), harmos_sidecar::Publisher<MultiStream>>,
    ) -> Result<(), harmos_sidecar::PublishError> {
        if ctx.tick().ordinal() % 2 == 0 {
            return Ok(());
        }

        ctx.publish_on::<Left>(&support::pulse(1)).await?;
        ctx.publish_on::<Right>(&support::pulse(2)).await?;
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for MultiStream {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "One Observer Publishing Two Rows.", id = "multi-stream", provides = (), requires(harmos_runtime::operations::streams::Publish),
    observers = MultiObservers, streams(Left, Right))]
impl harmos_runtime::Plugin for MultiStream {
    type Config = ();
    type Context = harmos_sidecar::Supervisor;
    type Resources = ();
    type Error = harmos_sidecar::Refusal;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        let resources = Arc::new(());
        let attachment = harmos_sidecar::ObserverAttachment::from_catalog::<MultiObservers>(
            resources.clone(),
            Self::STREAMS,
        )?;
        Ok(Self {
            resources,
            attachment,
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        self.attachment.bind(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_domain::plugin::Attachment<Self::Context>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[harmos_runtime::observer(streams(FloodRows), interval_ms = 1)]
impl Flooding {
    async fn initialize(resources: &super::witness::Witness) -> Self {
        Self { next: 0, witness: super::witness::Witness::find(resources.key()).unwrap() }
    }
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, super::witness::Witness, harmos_sidecar::Publisher<Flood>>,
    ) -> Result<(), harmos_sidecar::PublishError> {
        if self.next == FLOOD_ROWS {
            return Ok(());
        }
        let publisher = ctx.host();
        let mut batch = publisher.batch::<FloodRows>();
        for sample in self.next..self.next + FLOOD_BATCH {
            batch.push(support::pulse(sample));
        }
        self.witness.mark("publication.started");
        publisher.publish_batch_on::<FloodRows>(&batch).await?;
        self.witness.mark("publication.completed");
        self.next += FLOOD_BATCH;
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Flood {
    const INITIALIZATION_POLICY: harmos_runtime::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos_runtime::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Publishes Faster Than Its Host Ingests.", id = "flood", provides = (), requires(harmos_runtime::operations::streams::Publish),
    observers = FloodObservers, streams(FloodRows))]
impl harmos_runtime::Plugin for Flood {
    type Config = String;
    type Context = harmos_sidecar::Supervisor;
    type Resources = super::witness::Witness;
    type Error = harmos_sidecar::Refusal;
    fn prepare(key: String) -> Result<Self, Self::Error> {
        let resources = super::witness::Witness::find(&key)
            .ok_or_else(|| harmos_sidecar::Refusal::of("publication witness unavailable"))?;
        let attachment = harmos_sidecar::ObserverAttachment::from_catalog::<FloodObservers>(
            resources.clone(),
            Self::STREAMS,
        )?;
        Ok(Self {
            resources,
            attachment,
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<super::witness::Witness> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut harmos_runtime::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        self.attachment.bind(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_domain::plugin::Attachment<Self::Context>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */
pub(super) async fn ready(mut ready: tokio::sync::watch::Receiver<bool>) {
    (ready.wait_for(|value| *value)).await
        .unwrap();
}

/* === Private Functions === */
fn observers(
    runtime: &harmos_runtime::Runtime<crate::support::Bank>,
) -> std::collections::BTreeMap<
    harmos_runtime::operations::resident::Key,
    harmos_runtime::operations::resident::Snapshot,
> {
    runtime
        .operations
        .residents()
        .into_iter()
        .filter(|(_, status)| status.kind == harmos_runtime::operations::resident::Kind::Observer)
        .collect()
}

fn observer_definitions(
    runtime: &harmos_runtime::Runtime<crate::support::Bank>,
) -> Vec<harmos_runtime::operations::resident::Definition> {
    runtime
        .operations
        .resident_definitions()
        .into_iter()
        .filter(|definition| definition.kind() == harmos_runtime::operations::resident::Kind::Observer)
        .collect()
}

async fn row(
    stream: &mut (
             impl futures_core::Stream<Item = harmos_runtime::streams::Item<harmos_runtime::streams::PublicationRow>>
             + Unpin
         ),
) {
    (async {
        loop {
            let item = std::future::poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await;
            if matches!(item, Some(harmos_runtime::streams::Item::Row(_))) {
                return;
            }
            assert!(item.is_some(), "the observer stream must stay live");
        }
    }).await;
}

