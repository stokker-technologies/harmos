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
use std::convert::Infallible;
use std::pin::Pin;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::operations::{Lanes, Managed};
use super::*;
use harmos::{storage::Batch, storage::Cursor, storage::StreamConsumer};
use harmos_contract::plugin::Declared;
use harmos_sidecar::{Attach, ObserverContext};

/* === Constants === */
// 2025-10-09 Around 08:53 UTC In Microseconds
const TIMED_BASE: i64 = 1_760_000_000_000_000;
const TICKING_DEMAND: harmos_sidecar::DemandKey =
    harmos_sidecar::DemandKey::new("ticking", "pulse");
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

#[harmos::stream(id = "ticking.pulse", window = 8, row = support::Pulse)]
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

#[harmos::stream(id = "twins.left", window = 16, row = support::Pulse)]
struct Left;

#[harmos::stream(id = "twins.right", window = 16, row = support::Pulse)]
struct Right;

struct Twins {
    resources: Arc<()>,
}

#[harmos::stream(id = "batches.rows", window = 16, row = support::Pulse)]
struct BatchedRows;

#[harmos::stream(id = "batches.raw", window = 16, row = support::Pulse)]
struct RawRows;

#[harmos::stream(id = "batches.timed", window = 1024, row = support::Pulse)]
struct TimedRows;

#[harmos::stream(id = "batches.clocked", window = 1024, row = support::Pulse)]
struct ClockedRows;

#[harmos::stream(id = "batches.stamped", window = 16, row = support::Pulse)]
struct StampedRows;

struct Batches {
    resources: Arc<()>,
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

struct ObserveControl;

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
#[harmos::catalog]
enum TickingObservers {
    Producer(Producer),
}

#[harmos::catalog]
enum TwinObservers {
    Left(LeftObserver),
    Right(RightObserver),
}

#[harmos::catalog]
enum MultiObservers {
    Producer(MultiStreamObserver),
}

#[harmos::catalog]
enum ObserverOperations {
    Observe(ObserveControl),
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

    fn operations(&self) -> harmos_contract::Result<Vec<harmos::operations::Definition>> {
        self.inner.operations()
    }
    fn operation_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos::operations::Contract>>> {
        self.inner.operation_contracts()
    }
    fn required_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::inventory::contracts::Required>>> {
        self.inner.required_contracts()
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_contract::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
        self.inner.sdk_lifecycle()
    }
    fn observer_control(&self) -> harmos_contract::Result<Option<harmos::operations::Contract>> {
        self.inner.observer_control()
    }
    fn listener_declarations(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::listeners::Definition>>> {
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

impl harmos::operations::lifecycle::Policy for Ticking {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Observer Demand During SDK Initialization.", id = "ticking", provides = ObserverOperations, requires(harmos::operations::host::Publish),
    observers = TickingObservers, observer_control = ObserveControl, streams(TickingPulse))]
impl harmos::Plugin for Ticking {
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
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        self.attachment.bind::<ObserveControl>(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_contract::plugin::Attachment<Self::Context>>> {
        Some(self.attachment.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[harmos::observer(
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
        ctx.publish_on::<TickingPulse>(&support::pulse(ctx.tick().ordinal()))
            .await
            .unwrap();
        Ok(())
    }
}

impl harmos::operations::lifecycle::Policy for Twins {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy =
        harmos::operations::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_millis(500),
            retries: harmos::operations::RetryPolicy::Never,
            restart: harmos::operations::RestartPolicy::Never,
        };
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Two Streams Share One Row Shape.", id = "twins", provides = (), requires(harmos::operations::host::Publish),  observers = (), streams(Left),
    stream_entries = &[harmos_sidecar::inventory::publications::Entry::of::<Right>()])]
impl harmos::Plugin for Twins {
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
            .map_err(harmos_sidecar::Refusal::of)?;
        context
            .publisher::<Self>()
            .publish_on::<Right>(&support::pulse(20))
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
        context
            .publisher::<Self>()
            .publish_on_context::<Left>(&support::pulse(11), Some("execution-b"))
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos::operations::lifecycle::Policy for Batches {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = Twins::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Publishes Ordered Row Batches.", id = "batches", provides = (), requires(harmos::operations::host::Publish), observers = (), streams(BatchedRows, RawRows, TimedRows, ClockedRows, StampedRows))]
impl harmos::Plugin for Batches {
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
            .map_err(harmos_sidecar::Refusal::of)?;
        publisher
            .publish_on::<BatchedRows>(&support::pulse(4))
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
        publisher
            .publish_batch_on::<BatchedRows>(&publisher.batch())
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
        publisher
            .publish_batch_on_context::<BatchedRows>(&pulses(5..=6), Some("execution-b"))
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
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
            .map_err(harmos_sidecar::Refusal::of)?;
        // Kit Stamps: Each Row Carries The Time It Was Pushed, Not Sent
        let mut clocked = publisher.batch::<ClockedRows>();
        for sample in 0..100 {
            clocked.push(support::pulse(sample));
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        publisher
            .publish_batch_on::<ClockedRows>(&clocked)
            .await
            .map_err(harmos_sidecar::Refusal::of)?;
        // A Refused Row Ends Its Batch; The Rows Before It Stay Published.
        let raw = |stream: &str,
                   sequence: u64,
                   published_unix_micros: Option<i64>,
                   following: Vec<(Vec<u8>, Option<i64>)>| {
            harmos::operations::host::Publication {
                stream: stream.to_owned(),
                schema: 1,
                sequence: Some(sequence),
                published_unix_micros,
                context: None,
                payload: harmos::Message::encode_to_vec(&support::pulse(sequence)),
                following: following
                    .into_iter()
                    .map(|(payload, published_delta_micros)| {
                        harmos::operations::host::FollowingRow {
                            payload,
                            published_delta_micros,
                        }
                    })
                    .collect(),
            }
        };
        let pulse = |sample: u64| harmos::Message::encode_to_vec(&support::pulse(sample));
        let publish = |request| {
            harmos::operations::Host::call_as::<harmos::operations::host::Publish>(context, request)
        };
        let raw_id = <RawRows as harmos::StreamSpec>::ID;
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
            .map_err(harmos_sidecar::Refusal::of)?;
        // A Delta Without A Stamped Predecessor Refuses The Whole Call
        let stamped_id = <StampedRows as harmos::StreamSpec>::ID;
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
            .map_err(harmos_sidecar::Refusal::of)?;
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
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

impl harmos::operations::OperationDeclaration for ObserveControl {
    type Request = harmos_contract::observers::Control;
    type Response = ();
    const ID: &'static str = "publications.observer.observe";
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Host;
    const POLICY: harmos::operations::OperationPolicy = harmos_contract::observers::POLICY;
}

impl harmos::operations::WireOperation for ObserveControl {
    const VERSION: u32 = 1;
}

const LIFECYCLE: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    retries: harmos::operations::RetryPolicy::Never,
    restart: harmos::operations::RestartPolicy::Never,
};

impl harmos::operations::lifecycle::Policy for SampledTwins {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Independent Observers Sharing The Original Initialized Resources.", id = "twin-observers", provides = ObserverOperations, requires(harmos::operations::host::Publish),
    observers = TwinObservers, observer_control = ObserveControl, streams(Left, Right))]
impl harmos::Plugin for SampledTwins {
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
            <Self as harmos_contract::plugin::Declared>::STREAMS,
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
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        self.attachment.bind::<ObserveControl>(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_contract::plugin::Attachment<Self::Context>>> {
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

#[harmos::observer(streams(Left), activation = "demand", interval_ms = 5)]
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
        while self.0.coordinated.load(std::sync::atomic::Ordering::SeqCst)
            && self
                .0
                .stopped
                .iter()
                .any(|count| count.load(std::sync::atomic::Ordering::SeqCst) == 0)
        {
            tokio::task::yield_now().await;
        }

        Ok(())
    }
}

#[harmos::observer(streams(Right), activation = "demand", interval_ms = 5)]
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
        while self.0.coordinated.load(std::sync::atomic::Ordering::SeqCst)
            && self
                .0
                .stopped
                .iter()
                .any(|count| count.load(std::sync::atomic::Ordering::SeqCst) == 0)
        {
            tokio::task::yield_now().await;
        }

        Ok(())
    }
}

#[harmos::observer(interval_ms = 5, streams(Left, Right), activation = "demand")]
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

impl harmos::operations::lifecycle::Policy for MultiStream {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = LIFECYCLE;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "One Observer Publishing Two Rows.", id = "multi-stream", provides = ObserverOperations, requires(harmos::operations::host::Publish),
    observers = MultiObservers, observer_control = ObserveControl, streams(Left, Right))]
impl harmos::Plugin for MultiStream {
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
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        self.attachment.bind::<ObserveControl>(table)
    }
    fn attachment(&self) -> Option<Arc<dyn harmos_contract::plugin::Attachment<Self::Context>>> {
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
    tokio::time::timeout(Duration::from_secs(2), ready.wait_for(|value| *value))
        .await
        .unwrap()
        .unwrap();
}

/* === Private Functions === */
fn observers(
    runtime: &harmos::Runtime<crate::support::Bank>,
) -> std::collections::BTreeMap<
    harmos::operations::resident::Key,
    harmos::operations::resident::Snapshot,
> {
    runtime
        .operations
        .residents()
        .into_iter()
        .filter(|(_, status)| status.kind == harmos::operations::resident::Kind::Observer)
        .collect()
}

fn observer_definitions(
    runtime: &harmos::Runtime<crate::support::Bank>,
) -> Vec<harmos::operations::resident::Definition> {
    runtime
        .operations
        .resident_definitions()
        .into_iter()
        .filter(|definition| definition.kind() == harmos::operations::resident::Kind::Observer)
        .collect()
}

async fn row(
    stream: &mut (
             impl futures_core::Stream<Item = harmos::streams::Item<harmos::streams::PublicationRow>>
             + Unpin
         ),
) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let item = std::future::poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await;
            if matches!(item, Some(harmos::streams::Item::Row(_))) {
                return;
            }
            assert!(item.is_some(), "the observer stream must stay live");
        }
    })
    .await
    .unwrap();
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("capture_survives_reload", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let attachments = Arc::new(AtomicUsize::new(0));
                    let source = meter(attachments.clone());
                    let definitions =
                        operations::inspected(&source, serde_json::to_vec(&()).unwrap());
                    let calls = Supervised::new(
                        source,
                        (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice),
                    );
                    let rows = Arc::new(Mutex::new(Vec::new()));
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .operation_settings(harmos::operations::Settings {
                            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .attach_streams(Recorder {
                            cursor: Cursor::ORIGIN,
                            rows: rows.clone(),
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let mut fixture = Managed::from_runtime(
                        runtime,
                        definitions,
                        calls,
                        Lanes::NativeOpen,
                        std::num::NonZeroUsize::new(16).unwrap(),
                        harmos::services::POLICY,
                    );
                    fixture.publication().unwrap();
                    fixture
                        .runtime
                        .operations
                        .register_operation::<Sampling>(
                            "meter-sample",
                            Lanes::NativeOpen,
                            Arc::new(SampleProjection {
                                streams: fixture.runtime.streams.clone(),
                            }),
                        )
                        .unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    fixture
                        .call_as::<PublishPulse>(PublishPulse { sample: 41 })
                        .await
                        .unwrap();
                    let stream = fixture
                        .runtime
                        .streams
                        .publication("meter.pulse", 1)
                        .unwrap();
                    assert_eq!(stream.definition().owner(), Some("meter"));
                    assert!(
                        !stream
                            .definition()
                            .schema()
                            .unwrap()
                            .root()
                            .unwrap()
                            .fields()
                            .is_empty()
                    );

                    fixture.shutdown().await.unwrap();
                    assert_eq!(stream.query(..).await.unwrap().count(), 1);
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    fixture
                        .call_as::<PublishPulse>(PublishPulse { sample: 42 })
                        .await
                        .unwrap();
                    let samples: Vec<_> = stream
                        .query(..)
                        .await
                        .unwrap()
                        .map(|row| {
                            assert!(
                                row.value.published_unix_micros.is_some(),
                                "sidecar submission time must reach the retained stream"
                            );
                            support::Pulse::decode(row.value.payload.as_slice())
                                .unwrap()
                                .sample
                        })
                        .collect();
                    assert_eq!(samples, [41, 42]);
                    assert_eq!(attachments.load(Ordering::SeqCst), 2);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();

                    let recorded = rows.lock().unwrap();
                    assert_eq!(recorded.len(), 2);
                    assert!(recorded.iter().all(|(id, _)| id == "meter.pulse"));
                    assert_eq!(
                        recorded
                            .iter()
                            .map(|(_, bytes)| support::Pulse::decode(bytes.as_slice())
                                .unwrap()
                                .sample)
                            .collect::<Vec<_>>(),
                        [41, 42]
                    );
                });
        });
    }
    {
        scenario_runtime::check("demand_survives_reload", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let gate = Gate::default();
    let seed = GateSeed::new(&gate);
    let capacity = std::num::NonZeroUsize::new(4).unwrap();
    let source = harmos_sidecar::PluginNative::<Ticking>::new(capacity).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        capacity,
        vec![TICKING_DEMAND.name()],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    let demands = fixture.demands();
    let held = demands.hold(&TICKING_DEMAND.name()).unwrap();
    let streams = fixture.runtime.streams.clone();
    {
        let initializing = fixture.initialize();
        tokio::pin!(initializing);
        tokio::select! {
            result = &mut initializing => panic!("initialization completed before gate release: {result:?}"),
            _ = gate.started.notified() => {}
        }
        assert!(
            streams.publication("ticking.pulse", 1).is_none(),
            "blocked initialization cannot expose staged streams"
        );
        gate.release.notify_one();
        initializing.await.unwrap();
    }
    ready(fixture.ready().clone()).await;
    let stream = streams.publication("ticking.pulse", 1).unwrap();
    let mut samples = stream.watch();
    row(&mut samples).await;
    let original = observers(&fixture.runtime);
    assert_eq!(original.len(), 1);
    let retiring = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    fixture.pause_retirement(retiring.clone(), release.clone());
    {
        let shutdown = fixture.shutdown();
        tokio::pin!(shutdown);
        tokio::select! {
            result = &mut shutdown => panic!("shutdown completed before retirement release: {result:?}"),
            _ = retiring.notified() => {}
        }
        // The Original Observer Still Owns Its Control After Ordinary Admission Closes.
        let mut during_retirement = stream.watch();
        tokio::select! {
            result = &mut shutdown => panic!("shutdown completed before retirement release: {result:?}"),
            _ = row(&mut during_retirement) => {}
        }
        release.notify_one();
        shutdown.await.unwrap();
    }
    drop(samples);
    assert!(observer_definitions(&fixture.runtime).is_empty());

    let replacement_seed = GateSeed::new(&gate);
    {
        let reloading = fixture.reload(serde_json::to_vec(&replacement_seed.id).unwrap());
        tokio::pin!(reloading);
        tokio::select! {
            result = &mut reloading => panic!("replacement initialized before gate release: {result:?}"),
            _ = gate.started.notified() => {}
        }
        gate.release.notify_one();
        reloading.await.unwrap();
    }
    ready(fixture.ready().clone()).await;
    let replacement = observers(&fixture.runtime);
    assert_eq!(replacement.len(), 1);
    assert!(
        replacement.keys().all(|key| !original.contains_key(key)),
        "replacement keeps demand but gets a fresh generation"
    );
    let mut after_restart = streams.publication("ticking.pulse", 1).unwrap().watch();
    row(&mut after_restart).await;
    drop(held);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("batches_keep_order", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let capacity = std::num::NonZeroUsize::new(2).unwrap();
                    let source = harmos_sidecar::PluginNative::<Batches>::new(capacity).unwrap();
                    // A Decoded 100-Row Batch Outgrows The Usual 4 KiB Preparation Budget
                    let runtime = assembly()
                        .register(Lanes::catalog())
                        .operation_settings(harmos::operations::Settings {
                            preparation_bytes: std::num::NonZeroUsize::new(64 * 1024),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let definitions =
                        super::operations::inspected(&source, serde_json::to_vec(&()).unwrap());
                    let resident = harmos::sidecar::Supervised::new(
                        source,
                        (|_: &harmos::services::Service<crate::support::Bank>,
                          _: harmos::sidecar::Notice| {})
                            as fn(&harmos::services::Service<crate::support::Bank>, _),
                    );
                    let mut fixture = Managed::from_runtime(
                        runtime,
                        definitions,
                        resident,
                        Lanes::NativeOpen,
                        capacity,
                        harmos::services::POLICY,
                    );
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    fixture.shutdown().await.unwrap();

                    let rows = |id: &str| {
                        let stream = fixture.runtime.streams.publication(id, 1).unwrap();
                        async move {
                            stream
                                .query(..)
                                .await
                                .unwrap()
                                .map(|row| {
                                    (
                                        support::Pulse::decode(row.value.payload.as_slice())
                                            .unwrap()
                                            .sample,
                                        row.value.context.clone(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        }
                    };
                    let a = || Some("execution-a".to_owned());
                    let b = || Some("execution-b".to_owned());
                    assert_eq!(
                        rows("batches.rows").await,
                        vec![(1, a()), (2, a()), (3, a()), (4, None), (5, b()), (6, b())]
                    );
                    assert_eq!(
                        rows("batches.raw").await,
                        vec![(1, None), (2, None), (3, None)]
                    );
                    let stamps = |id: &str| {
                        let stream = fixture.runtime.streams.publication(id, 1).unwrap();
                        async move {
                            stream
                                .query(..)
                                .await
                                .unwrap()
                                .map(|row| {
                                    (
                                        support::Pulse::decode(row.value.payload.as_slice())
                                            .unwrap()
                                            .sample,
                                        row.value.published_unix_micros.unwrap(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        }
                    };
                    // 100 Distinct Stamps Exactly One Millisecond Apart
                    assert_eq!(
                        stamps("batches.timed").await,
                        (0..100)
                            .map(|sample| (sample, TIMED_BASE + 1_000 * sample as i64))
                            .collect::<Vec<_>>()
                    );
                    // Pushed At Least A Millisecond Apart: Strictly Rising Stamps
                    let clocked = stamps("batches.clocked").await;
                    assert_eq!(
                        clocked
                            .iter()
                            .map(|(sample, _)| *sample)
                            .collect::<Vec<_>>(),
                        (0..100).collect::<Vec<_>>()
                    );
                    let gaps: Vec<_> = clocked
                        .windows(2)
                        .map(|pair| pair[1].1 - pair[0].1)
                        .collect();
                    assert!(gaps.iter().all(|gap| *gap >= 1_000), "{gaps:?}");
                    let span = clocked[99].1 - clocked[0].1;
                    assert!((99_000..10_000_000).contains(&span), "{span}");
                    // The Backwards Row And Everything After It Stay Unpublished
                    assert_eq!(
                        stamps("batches.stamped").await,
                        vec![
                            (1, TIMED_BASE),
                            (2, TIMED_BASE + 1_000),
                            (3, TIMED_BASE + 1_000),
                            (4, TIMED_BASE),
                        ]
                    );
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("named_streams_isolated", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let capacity = std::num::NonZeroUsize::new(2).unwrap();
                    let source = harmos_sidecar::PluginNative::<Twins>::new(capacity).unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&()).unwrap(),
                        Lanes::NativeOpen,
                        capacity,
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    fixture.shutdown().await.unwrap();

                    for (id, expected) in [("left", vec![10, 11]), ("right", vec![20])] {
                        let stream = fixture
                            .runtime
                            .streams
                            .publication(&format!("twins.{id}"), 1)
                            .unwrap();
                        let samples: Vec<_> = stream
                            .query(..)
                            .await
                            .unwrap()
                            .map(|row| {
                                support::Pulse::decode(row.value.payload.as_slice())
                                    .unwrap()
                                    .sample
                            })
                            .collect();
                        assert_eq!(samples, expected);
                        let contexts: Vec<_> = stream
                            .query(..)
                            .await
                            .unwrap()
                            .map(|row| row.value.context.clone())
                            .collect();
                        assert_eq!(
                            contexts,
                            if id == "left" {
                                vec![Some("execution-a".into()), Some("execution-b".into())]
                            } else {
                                vec![None]
                            }
                        );
                        assert_eq!(stream.definition().id(), format!("twins.{id}"));
                        assert_eq!(
                            stream.definition().schema().unwrap(),
                            &harmos::schema::graph::Schema::from_declaration(
                                <support::Pulse as harmos::schema::Described>::DEFINITION,
                                &harmos::config::SchemaSettings::default()
                            )
                            .unwrap()
                        );
                    }
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("projection_preserves_evidence", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let capacity = std::num::NonZeroUsize::new(2).unwrap();
                    let source = harmos_sidecar::PluginNative::<Twins>::new(capacity).unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&()).unwrap(),
                        Lanes::NativeOpen,
                        capacity,
                        vec![],
                        |resident| resident.ingest::<Left>().ingest::<Right>(),
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    fixture.shutdown().await.unwrap();

                    let projected: Vec<_> = fixture
                        .runtime
                        .streams
                        .query::<support::Pulse>(..)
                        .await
                        .unwrap()
                        .map(|row| row.value.sample)
                        .collect();
                    assert_eq!(projected, [10, 20, 11]);
                    for (id, expected) in [("twins.left", vec![10, 11]), ("twins.right", vec![20])]
                    {
                        let source = fixture.runtime.streams.publication(id, 1).unwrap();
                        assert_eq!(source.definition().owner(), Some("twins"));
                        let retained: Vec<_> = source
                            .query(..)
                            .await
                            .unwrap()
                            .map(|row| {
                                support::Pulse::decode(row.value.payload.as_slice())
                                    .unwrap()
                                    .sample
                            })
                            .collect();
                        assert_eq!(retained, expected);
                    }
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shared_row_independence", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering::SeqCst;
                    let counts = ObserverCounts::default();
                    let seed = ObserverSeed::new(&counts);
                    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(
                        std::num::NonZeroUsize::new(4).unwrap(),
                    )
                    .unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        std::num::NonZeroUsize::new(4).unwrap(),
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    let runtime = &fixture.runtime;
                    let left = runtime.streams.publication("twins.left", 1).unwrap();
                    let right = runtime.streams.publication("twins.right", 1).unwrap();
                    assert_eq!(left.definition().schema(), right.definition().schema());
                    assert_eq!(counts.started[0].load(SeqCst), 0);
                    assert_eq!(counts.started[1].load(SeqCst), 0);

                    let mut left_rows = left.watch();
                    row(&mut left_rows).await;
                    assert_eq!(counts.started[0].load(SeqCst), 1);
                    assert_eq!(counts.started[1].load(SeqCst), 0);
                    assert_eq!(right.query(..).await.unwrap().count(), 0);

                    let mut right_rows = right.watch();
                    row(&mut right_rows).await;
                    assert_eq!(counts.started[1].load(SeqCst), 1);
                    drop(left_rows);
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while counts.stopped[0].load(SeqCst) == 0 {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(counts.stopped[1].load(SeqCst), 0);
                    assert_eq!(counts.released.load(SeqCst), 0);
                    assert_eq!(
                        support::Pulse::decode(
                            left.query(..)
                                .await
                                .unwrap()
                                .next()
                                .unwrap()
                                .value
                                .payload
                                .as_slice()
                        )
                        .unwrap()
                        .sample,
                        101
                    );
                    assert_eq!(
                        support::Pulse::decode(
                            right
                                .query(..)
                                .await
                                .unwrap()
                                .next()
                                .unwrap()
                                .value
                                .payload
                                .as_slice()
                        )
                        .unwrap()
                        .sample,
                        201
                    );

                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(counts.stopped[0].load(SeqCst), 1);
                    assert_eq!(counts.stopped[1].load(SeqCst), 1);
                    assert_eq!(counts.released.load(SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("catalog_precedes_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use harmos::plugin::{PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitions};
    use std::sync::atomic::Ordering::SeqCst;

    for case in 0..5 {
        let counts = ObserverCounts::default();
        let seed = ObserverSeed::new(&counts);
        let capacity = std::num::NonZeroUsize::new(4).unwrap();
        let inner = harmos_sidecar::PluginNative::<SampledTwins>::new(capacity).unwrap();
        let mut observers = inner.observers();
        let reason = match case {
            0 => {
                observers[0].interval = Duration::ZERO;
                "interval must be positive"
            }
            1 => {
                observers.pop();
                "observers differ from the admitted catalog"
            }
            2 => {
                observers[0].interval += Duration::from_nanos(1);
                "observers differ from the admitted catalog"
            }
            3 => {
                observers[0].streams.push("ungranted".into());
                "artifact publication grant"
            }
            _ => {
                observers.push(observers[0].clone());
                "identities must be nonempty, unique"
            }
        };
        let inert =
            harmos_sidecar::inventory::observers::validate(&observers, &inner.publications());
        let definition = PluginDefinition {
            id: inner.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar {
                revision: "fixture".into(),
            },
            configuration: serde_json::to_vec(&seed.id).unwrap(),
            requires: vec![],
            contributions: PluginContributions {
                operations: inner.operations().unwrap(),
                wire_contracts: inner.operation_contracts().unwrap().unwrap(),
                required_contracts: inner.required_contracts().unwrap(),
                lifecycle: inner.sdk_lifecycle().unwrap(),
                streams: inner.publications(),
                observers: observers.clone(),
                observer_control: inner.observer_control().unwrap(),
                listeners: inner.listener_declarations().unwrap().unwrap(),
                ..PluginContributions::default()
            },
        };
        let definitions = PluginDefinitions::new([definition], []);
        if matches!(case, 0 | 3 | 4) {
            assert!(
                inert.unwrap_err().to_string().contains(reason),
                "case {case}"
            );
            assert!(
                definitions.is_err(),
                "invalid inventory fails manager preflight before attachment"
            );
            assert_eq!(counts.initialized.load(SeqCst), 0);
            assert!(
                OBSERVER_SEEDS.lock().unwrap().contains_key(&seed.id),
                "preflight did not prepare resources"
            );
            assert_eq!(counts.released.load(SeqCst), 0);
            continue;
        }
        inert.unwrap();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peer = Advertised {
            inner,
            observers,
            attached: attempts.clone(),
        };
        let mut fixture = Managed::from_definition(
            peer,
            definitions.unwrap(),
            Lanes::NativeOpen,
            capacity,
            vec![],
            std::convert::identity,
        )
        .await;
        fixture.publication().unwrap();
        assert!(fixture.initialize().await.is_err());
        let status = service_status(
            fixture.runtime.operations.watch_residents(),
            "twin-observers",
            |status| matches!(status, ServiceStatus::Inactive { .. }),
        )
        .await;
        assert!(
            matches!(status, ServiceStatus::Inactive { cause: ServiceExit::Application(ref message), .. } if message.contains(reason)),
            "case {case}: {status:?}"
        );
        assert_eq!(
            attempts.load(SeqCst),
            1,
            "application admission refusal is terminal under the original service policy"
        );
        assert_eq!(counts.initialized.load(SeqCst), 0, "case {case}");
        assert!(
            OBSERVER_SEEDS.lock().unwrap().contains_key(&seed.id),
            "wire mismatch precedes preparation"
        );
        assert!(!*fixture.ready().borrow());
        assert_eq!(counts.released.load(SeqCst), 0);
        assert!(matches!(
            fixture.runtime.stop().await,
            Err(harmos::Error::TaskFailed {
                domain: "service",
                ..
            })
        ));
    }

});
        });
    }
    {
        scenario_runtime::check("catalog_order_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos_sidecar::Attach;
                    use std::sync::atomic::Ordering::SeqCst;

                    let counts = ObserverCounts::default();
                    let seed = ObserverSeed::new(&counts);
                    let inner = harmos_sidecar::PluginNative::<SampledTwins>::new(
                        std::num::NonZeroUsize::new(4).unwrap(),
                    )
                    .unwrap();
                    let mut observers = inner.observers();
                    observers.reverse();
                    for observer in &mut observers {
                        observer.streams.reverse();
                        observer.demand_keys.reverse();
                    }
                    let peer = Advertised {
                        inner,
                        observers,
                        attached: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                    };
                    let mut fixture = Managed::attach(
                        peer,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        std::num::NonZeroUsize::new(4).unwrap(),
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    assert_eq!(counts.initialized.load(SeqCst), 1);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_signals_siblings", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering::SeqCst;
                    let counts = ObserverCounts::default();
                    counts.coordinated.store(true, SeqCst);
                    let seed = ObserverSeed::new(&counts);
                    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(
                        std::num::NonZeroUsize::new(4).unwrap(),
                    )
                    .unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        std::num::NonZeroUsize::new(4).unwrap(),
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    let runtime = &fixture.runtime;
                    assert_eq!(observer_definitions(runtime).len(), 2);
                    let mut left = runtime
                        .streams
                        .publication("twins.left", 1)
                        .unwrap()
                        .watch();
                    let mut right = runtime
                        .streams
                        .publication("twins.right", 1)
                        .unwrap()
                        .watch();
                    row(&mut left).await;
                    row(&mut right).await;
                    let mut statuses = runtime.operations.watch_residents();
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        statuses.wait_for(|statuses| {
                            let observers: Vec<_> = statuses
                                .values()
                                .filter(|status| {
                                    status.kind == harmos::operations::resident::Kind::Observer
                                })
                                .collect();
                            observers.len() == 2
                                && observers.iter().all(|status| {
                                    status.lifecycle.phase()
                                        == harmos::operations::OperationPhase::Running
                                        && status.leases == 1
                                })
                        }),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(counts.stopped[0].load(SeqCst), 1);
                    assert_eq!(counts.stopped[1].load(SeqCst), 1);
                    assert_eq!(counts.released.load(SeqCst), 1);
                    assert!(observer_definitions(&fixture.runtime).is_empty());
                });
        });
    }
    {
        scenario_runtime::check("stop_preserves_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering::SeqCst;
                    let counts = ObserverCounts::default();
                    counts.stop_panics.store(true, SeqCst);
                    let seed = ObserverSeed::new(&counts);
                    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(
                        std::num::NonZeroUsize::new(4).unwrap(),
                    )
                    .unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        std::num::NonZeroUsize::new(4).unwrap(),
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.publication().unwrap();
                    fixture.initialize().await.unwrap();
                    ready(fixture.ready().clone()).await;
                    let runtime = &fixture.runtime;
                    let mut left = runtime
                        .streams
                        .publication("twins.left", 1)
                        .unwrap()
                        .watch();
                    row(&mut left).await;
                    let error = fixture
                        .runtime
                        .operations
                        .stop_observers()
                        .await
                        .expect_err("native cleanup failure reaches the manager");
                    assert_eq!(counts.stop_attempts.load(SeqCst), 1);
                    assert_eq!(counts.stopped[0].load(SeqCst), 0);
                    assert_eq!(
                        counts.released.load(SeqCst),
                        0,
                        "provider survives failed observer cleanup"
                    );
                    let mut cause: &(dyn std::error::Error + 'static) = &error;
                    loop {
                        if cause
                            .to_string()
                            .contains("the observer panicked while stopping")
                        {
                            break;
                        }
                        if let Some(harmos::Error::TaskFailed { source, .. }) =
                            cause.downcast_ref::<harmos::Error>()
                        {
                            cause = source.as_ref();
                        } else if let Some(harmos::Error::PluginLifecycle { source }) =
                            cause.downcast_ref::<harmos::Error>()
                        {
                            cause = source.as_ref();
                        } else if let Some(harmos::plugin::PluginFailure::Cleanup(failures)) =
                            cause.downcast_ref::<harmos::plugin::PluginFailure>()
                        {
                            assert_eq!(failures.len(), 1);
                            cause = &failures[0].error;
                        } else {
                            cause = cause.source().expect("structured native exit is retained");
                        }
                    }
                    counts.stop_panics.store(false, SeqCst);
                    let _ = fixture.runtime.operations.stop_observers().await;
                    assert_eq!(counts.stop_attempts.load(SeqCst), 2);
                    assert_eq!(counts.started[0].load(SeqCst), 1);
                    assert_eq!(counts.stopped[0].load(SeqCst), 1);
                    let _ = fixture.shutdown().await;
                    let _ = fixture.runtime.stop().await;
                    assert_eq!(counts.released.load(SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("dropped_manager_recovery", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering::SeqCst;
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let counts = ObserverCounts::default();
                        counts.stop_panics.store(true, SeqCst);
                        let seed = ObserverSeed::new(&counts);
                        let source = harmos_sidecar::PluginNative::<SampledTwins>::new(
                            std::num::NonZeroUsize::new(4).unwrap(),
                        )
                        .unwrap();
                        let mut fixture = Managed::attach(
                            source,
                            serde_json::to_vec(&seed.id).unwrap(),
                            Lanes::NativeOpen,
                            std::num::NonZeroUsize::new(4).unwrap(),
                            vec![],
                            std::convert::identity,
                        )
                        .await;
                        fixture.publication().unwrap();
                        fixture.initialize().await.unwrap();
                        ready(fixture.ready().clone()).await;
                        let mut left = fixture
                            .runtime
                            .streams
                            .publication("twins.left", 1)
                            .unwrap()
                            .watch();
                        row(&mut left).await;
                        let (runtime, cleanup) = fixture.drop_manager();
                        cleanup.join().await.expect_err(
            "failed original observer cleanup is diagnosable after manager destruction",
        );
                        assert_eq!(counts.started[0].load(SeqCst), 1);
                        assert_eq!(counts.stop_attempts.load(SeqCst), 1);
                        assert_eq!(counts.stopped[0].load(SeqCst), 0);
                        assert_eq!(counts.released.load(SeqCst), 0);
                        counts.stop_panics.store(false, SeqCst);
                        let recovered = cleanup.retry().await;
                        assert_eq!(counts.stop_attempts.load(SeqCst), 2);
                        assert_eq!(counts.stopped[0].load(SeqCst), 1);
                        recovered.unwrap();
                        assert_eq!(counts.started[0].load(SeqCst), 1);
                        assert_eq!(counts.stop_attempts.load(SeqCst), 2);
                        assert_eq!(counts.stopped[0].load(SeqCst), 1);
                        assert_eq!(counts.released.load(SeqCst), 1);
                        drop(left);
                        let _ = runtime.stop().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("explicit_artifact_grants", || {
            assert!(
                harmos_sidecar::Attach::observers(
                    &harmos_sidecar::PluginNative::<Twins>::new(
                        std::num::NonZeroUsize::new(2).unwrap()
                    )
                    .unwrap()
                )
                .is_empty(),
                "linked observer types are not selected implicitly"
            );
            assert_eq!(MultiStream::STREAMS.len(), 2);
            let source = harmos_sidecar::PluginNative::<MultiStream>::new(
                std::num::NonZeroUsize::new(4).unwrap(),
            )
            .unwrap();
            let observers = source.observers();
            assert_eq!(observers.len(), 1);
            assert_eq!(observers[0].streams.len(), 2);

            let empty = harmos::sidecar::inventory::Declaration::inspect(
                &<Twins as harmos_contract::plugin::Declared>::markers()
                    .unwrap()
                    .concat(),
                harmos::sidecar::inventory::Facility::Sidecar,
            )
            .unwrap();
            assert!(empty.observers.is_empty());
            let selected = harmos::sidecar::inventory::Declaration::inspect(
                &MultiStream::markers().unwrap().concat(),
                harmos::sidecar::inventory::Facility::Sidecar,
            )
            .unwrap();
            assert_eq!(selected.observers.len(), 1);
            assert_eq!(source.observers(), selected.observers);
            assert_eq!(selected.observers[0].id, observers[0].id);
            assert_eq!(selected.observers[0].interval, observers[0].interval);
            assert_eq!(
                selected.observers[0].activation,
                harmos::sidecar::Activation::Demand
            );
            assert_eq!(
                selected.observers[0].streams,
                MultiStream::STREAMS
                    .iter()
                    .map(|entry| entry.id)
                    .collect::<Vec<_>>()
            );

            let missing: Vec<_> = MultiStream::markers()
                .unwrap()
                .into_iter()
                .filter(|marker| {
                    !String::from_utf8_lossy(marker).contains("\"kind\":\"observers\"")
                })
                .collect();
            assert!(matches!(
                harmos::sidecar::inventory::Declaration::inspect(
                    &missing.concat(),
                    harmos::sidecar::inventory::Facility::Sidecar
                ),
                Err(harmos::sidecar::inventory::DeclarationError::ObserverInventory(_))
            ));
        });
    }
}

/* === Tests === */
