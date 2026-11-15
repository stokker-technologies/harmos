/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Scoped Sleep Admission Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc as blocking,
    },
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

// Third Party Imports
use prost::Message;
use prost_types::Duration as ProtocolDuration;
use tokio::{
    sync::{Notify, mpsc, oneshot},
    time::Instant,
};

// Local Imports
use super::{
    invoke_support::{HostFixture, payload, pending},
    support,
};
use harmos_operations::clock::{ClockAccess, Sleep as SleepOperation};
use harmos_protocol::host::{TimeSleepRequest, TimeSleepResponse};
use harmos_runtime::{
    Scope,
    clock::{Clock, Reading, Sleep, Source},
    operations::{
        ExecutionContext, Lane, NativeOperation, OperationFactory, OperationPolicy, ProtobufDependency, ProtobufOperation,
        Registration as Batch, Resource, ServingError,
        middleware::{Enter, Invocation, Middleware},
    },
    plugin::grpc::Placement,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
struct Controlled {
    state: Arc<Mutex<TimeState>>,
    registered: Arc<Notify>,
    hold: Arc<Mutex<Option<Registration>>>,
}

struct Registration {
    entered: oneshot::Sender<()>,
    release: blocking::Receiver<()>,
}

struct TimeState {
    reading: Reading,
    next_id: usize,
    timers: BTreeMap<usize, (Instant, oneshot::Sender<()>)>,
}

struct Timer {
    receiver: oneshot::Receiver<()>,
    state: Arc<Mutex<TimeState>>,
    id: usize,
}

struct Admission {
    scope: String,
    release: oneshot::Sender<()>,
}

struct Gate(mpsc::UnboundedSender<Admission>);

/// A Host Operation That Sleeps Through a Typed Child Call.
struct TypedSleeper;

/// A Product-Defined Native Operation That Waits on Its Call's Clock.
struct NativeSleep;

#[derive(Clone)]
struct Wait(Duration);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Source for Controlled {
    fn read(&self) -> Reading {
        self.state.lock().unwrap().reading
    }

    fn sleep_until(&self, deadline: Instant) -> Sleep {
        let (sender, receiver) = oneshot::channel();
        let mut state = self.state.lock().unwrap();
        let id = state.next_id;
        state.next_id += 1;
        if deadline <= state.reading.instant {
            sender.send(()).unwrap();
        } else {
            state.timers.insert(id, (deadline, sender));
        }
        // Claim the Hold Before Announcing This Registration, So a Hold Set After the
        // Announcement Applies Only to the Next Registration.
        let hold = self.hold.lock().unwrap().take();
        drop(state);
        self.registered.notify_one();
        if let Some(registration) = hold {
            registration.entered.send(()).unwrap();
            registration.release.recv().unwrap();
        }
        Box::pin(Timer { receiver, state: self.state.clone(), id })
    }
}

impl Controlled {
    fn new() -> Self {
        let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
        Self {
            state: Arc::new(Mutex::new(TimeState {
                reading: clock.read(),
                next_id: 0,
                timers: BTreeMap::new(),
            })),
            registered: Arc::new(Notify::new()),
            hold: Arc::new(Mutex::new(None)),
        }
    }

    fn pending(&self) -> usize {
        self.state.lock().unwrap().timers.len()
    }

    fn advance_by(&self, duration: Duration) {
        let mut state = self.state.lock().unwrap();
        state.reading.instant += duration;
        state.reading.date += duration;
        let now = state.reading.instant;
        let ready = state.timers.extract_if(.., |_, (deadline, _)| *deadline <= now).map(|(_, (_, sender))| sender).collect::<Vec<_>>();
        drop(state);
        for sender in ready {
            let _ = sender.send(());
        }
    }

    async fn wait_registered(&self) {
        loop {
            let registered = self.registered.notified();
            if self.pending() != 0 {
                return;
            }
            registered.await;
        }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.receiver).poll(context).map(|_| ())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.state.lock().unwrap().timers.remove(&self.id);
    }
}

impl harmos_domain::MemorySize for Wait {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl NativeOperation for NativeSleep {
    type Request = Wait;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "fixture.native_sleep";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ClockAccess>()];
    fn admitted_wait(request: &Wait) -> Option<Duration> {
        Some(request.0)
    }
    async fn run(context: ExecutionContext, request: Wait, _: &()) -> Result<(), ServingError> {
        context.resource::<ClockAccess>().map_err(|_| ServingError::Catalog)?.0.sleep(request.0).await;
        Ok(())
    }
}

impl ProtobufOperation for TypedSleeper {
    type Request = TimeSleepRequest;
    type Response = TimeSleepResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.typed_sleeper";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const OPERATIONS: &'static [ProtobufDependency] = &[ProtobufDependency::of::<SleepOperation>()];
    const RESOURCES: &'static [Resource] = &[Resource::of::<harmos_operations::execution::operations::Calls>()];
    async fn run(context: ExecutionContext, request: TimeSleepRequest, _: &()) -> Result<TimeSleepResponse, ServingError> {
        Ok(context.call_protobuf::<SleepOperation>(request).await?.into_owned())
    }
}

impl Middleware for Gate {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            if matches!(call.operation().id.as_str(), "harmos.host.v1.Time/Sleep" | "fixture.native_sleep")
                && let Some(scope) = call.context().get::<Scope>()
            {
                let (release, waiting) = oneshot::channel();
                self.0.send(Admission { scope: scope.as_str().into(), release }).unwrap();
                let _ = waiting.await;
            }
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn retain_rows(requested: usize) {
    // Connect the Plugin Before Binding the Scoped Unit.
    let (admitted, mut admissions) = mpsc::unbounded_channel();
    let host = HostFixture::with_middleware(Arc::new(Gate(admitted))).await;
    let sdk = host.sdk("scoped-sleep").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/boundary", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();

    // Start the First Tick on the Controlled Clock.
    source.wait_registered().await;
    source.advance_by(Duration::from_secs(1));

    // Advance After Ingestion and While Middleware Still Holds Each Sleep.
    for row in 0..requested {
        let admission = admissions.recv().await.unwrap();
        assert_eq!(admission.scope, "unit/boundary");
        assert_eq!(source.pending(), 1, "Register Scoped Sleep Before Middleware for Row {row}");
        source.advance_by(Duration::from_secs(1));
        admission.release.send(()).unwrap();
        host.signals.storage_ingested.notified().await;
        assert_eq!(host.rows().len(), row + 1, "Ingest Exactly One Scoped Sample");
    }

    // Close Demand Without Capping Accepted Rows.
    unit.close().await.unwrap();
    source.advance_by(Duration::from_secs(10));
    let rows = host.rows();
    assert_eq!(rows.len(), requested, "Retain the Requested Scoped Row Count");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.published_unix_micros, Some((index as i64 + 1) * 1_000_000));
    }
    let values = (1..=requested)
        .map(|sequence| serde_json::json!({ "value": format!("unit/boundary:{sequence}:{}", sequence + 1).into_bytes() }))
        .collect::<Vec<_>>();
    assert_eq!(host.values(), values, "Retain Consecutive Scoped Samples");
    assert_eq!(source.pending(), 0, "Remove Scoped Timers on Close");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

/// One Sleep Spanning `requested` Ticks Observes Every One of Them.
async fn count_after_sleep(requested: u64) {
    // A Plugin Counts Its Scoped Ticks on a Virtual Unit Clock.
    let host = HostFixture::new().await;
    let sdk = host.sdk("counted-sleep").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let (clock, time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/counted", clock).await.unwrap();
    unit.start_samples().await.unwrap();

    // The Plugin Sleeps Once, Then Reads Its Count; the Last Tick Shares the Sleep's Instant.
    let finished = AtomicBool::new(false);
    let call = async {
        let result = unit.operations().submit_encoded("fixture.count_after", payload(&requested.to_le_bytes())).unwrap().await;
        finished.store(true, Ordering::SeqCst);
        result
    };
    let (result, advanced) = tokio::join!(call, time.advance_until(Duration::from_secs(requested + 1), || finished.load(Ordering::SeqCst)));
    advanced.unwrap();
    assert_eq!(result.unwrap(), payload(&requested.to_le_bytes()), "Settle the Sleep After Every Tick It Spans");
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn native_and_encoded_calls_own_registration_before_middleware() {
    // Bind a Scoped Unit Without Publication Timers.
    let (admitted, mut admissions) = mpsc::unbounded_channel();
    let host = HostFixture::with_middleware(Arc::new(Gate(admitted))).await;
    let sdk = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/calls", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();
    let request = TimeSleepRequest {
        duration: Some(ProtocolDuration { seconds: 1, nanos: 0 }),
    };

    // Register Native Protobuf Calls Before Returning Their Invocation Handle.
    let (call, _) = unit.operations().submit_protobuf::<SleepOperation>(request, None).unwrap();
    assert_eq!(source.pending(), 1, "Admit Native Scoped Sleep");
    let admission = admissions.recv().await.unwrap();
    source.advance_by(Duration::from_secs(1));
    admission.release.send(()).unwrap();
    call.await.unwrap();
    assert_eq!(source.pending(), 0, "Settle Native Scoped Sleep");

    // Cancel an Encoded Call While Middleware Holds Its Registered Timer.
    let call = unit.operations().submit_encoded("harmos.host.v1.Time/Sleep", request.encode_to_vec()).unwrap();
    assert_eq!(source.pending(), 1, "Admit Encoded Scoped Sleep");
    let admission = admissions.recv().await.unwrap();
    call.cancel().await.unwrap_err();
    drop(admission);
    assert_eq!(source.pending(), 0, "Cancel Encoded Scoped Sleep");
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_a_unit_during_timer_registration_removes_the_timer() {
    // Block the Clock's Synchronous Registration on the Other Runtime Thread.
    let host = HostFixture::new().await;
    let sdk = host.sdk("scoped-sleep").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/cancel", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();
    source.wait_registered().await;
    let (entered, registering) = oneshot::channel();
    let (release, held) = blocking::channel();
    *source.hold.lock().unwrap() = Some(Registration { entered, release: held });
    source.advance_by(Duration::from_secs(1));
    registering.await.unwrap();
    let registered = source.pending();

    // Initiate Unit Cancellation Before Registration Returns to Admission.
    let closing = unit.close();
    tokio::pin!(closing);
    let settling = pending(closing.as_mut()).await;
    release.send(()).unwrap();
    closing.await.unwrap();
    assert!(settling, "Await Scoped Registration Settlement on Close");
    assert_eq!(registered, 1, "Own the Timer During Scoped Registration");
    assert_eq!(source.pending(), 0, "Remove the Timer After Cancelling Registration");
    assert!(host.rows().is_empty(), "Cancel the Sample Being Prepared");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn admitted_sleeps_retain_exactly_1023_rows() {
    retain_rows(1023).await;
}

#[tokio::test]
async fn admitted_sleeps_retain_exactly_1024_rows() {
    retain_rows(1024).await;
}

#[tokio::test]
async fn admitted_sleeps_retain_exactly_1025_rows() {
    retain_rows(1025).await;
}

#[tokio::test]
async fn a_sleep_spanning_1023_ticks_observes_1023() {
    count_after_sleep(1023).await;
}

#[tokio::test]
async fn a_sleep_spanning_1024_ticks_observes_1024() {
    count_after_sleep(1024).await;
}

#[tokio::test]
async fn a_sleep_spanning_1025_ticks_observes_1025() {
    count_after_sleep(1025).await;
}

#[tokio::test]
async fn concurrent_units_register_and_cancel_only_their_own_sleepers() {
    // Share One Connection Between Two Independent Units.
    let (admitted, mut admissions) = mpsc::unbounded_channel();
    let host = HostFixture::with_middleware(Arc::new(Gate(admitted))).await;
    let sdk = host.sdk("scoped-sleep").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let left_source = Controlled::new();
    let right_source = Controlled::new();
    let left = host.runtime.bind_grpc(&activation, "unit/left", Clock::with_clock(left_source.clone())).await.unwrap();
    left.start_samples().await.unwrap();
    let right = host.runtime.bind_grpc(&activation, "unit/right", Clock::with_clock(right_source.clone())).await.unwrap();
    right.start_samples().await.unwrap();

    // Start Both Timelines Before Receiving Either Unit's Sleep.
    left_source.wait_registered().await;
    right_source.wait_registered().await;
    left_source.advance_by(Duration::from_secs(1));
    right_source.advance_by(Duration::from_secs(1));

    // Admit Both Units' Sleeps Together, Then Ingest Each on Its Own Clock.
    for _ in 0..1024 {
        let mut sleeping = BTreeMap::new();
        for _ in 0..2 {
            let admission = admissions.recv().await.unwrap();
            assert!(sleeping.insert(admission.scope, admission.release).is_none());
        }
        for (scope, source) in [("unit/left", &left_source), ("unit/right", &right_source)] {
            assert_eq!(source.pending(), 1, "Register Each Unit's Sleep Before Middleware");
            source.advance_by(Duration::from_secs(1));
            sleeping.remove(scope).unwrap().send(()).unwrap();
            host.signals.storage_ingested.notified().await;
        }
    }

    // Cancel Left During the Registration Handoff, Before Its Handler Runs.
    let mut sleeping = BTreeMap::new();
    for _ in 0..2 {
        let admission = admissions.recv().await.unwrap();
        assert!(sleeping.insert(admission.scope, admission.release).is_none());
    }
    assert_eq!(left_source.pending(), 1);
    left.close().await.unwrap();
    sleeping.remove("unit/left");
    assert_eq!(left_source.pending(), 0, "Cancel the Admitted Scoped Timer");

    // Right Can Still Complete Another Sample Before Its Own Close.
    right_source.advance_by(Duration::from_secs(1));
    sleeping.remove("unit/right").unwrap().send(()).unwrap();
    host.signals.storage_ingested.notified().await;
    right.close().await.unwrap();
    left_source.advance_by(Duration::from_secs(10));
    right_source.advance_by(Duration::from_secs(10));
    let rows = host.rows();
    for (scope, requested) in [("unit/left", 1024), ("unit/right", 1025)] {
        let retained = rows
            .iter()
            .filter(|row| {
                row.context.as_ref().is_some_and(|context| serde_json::from_str::<serde_json::Value>(context).unwrap()["scope"] == scope)
            })
            .count();
        assert_eq!(retained, requested, "Retain Each Unit's Exact Row Count");
    }
    assert_eq!(right_source.pending(), 0, "Remove Right's Timers on Close");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_operations_register_their_declared_wait_at_admission() {
    // Register a Product Native Operation That Declares Its Clock Wait.
    let (admitted, mut admissions) = mpsc::unbounded_channel();
    let host = HostFixture::with_middleware(Arc::new(Gate(admitted))).await;
    let registration = Batch::new(Lane::default()).operation(OperationFactory::native::<NativeSleep>().unwrap(), NativeSleep::ID).unwrap();
    host.runtime.register_batch(registration).await.unwrap();
    let sdk = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/native", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();

    // Admission Owns the Timer Before Middleware Sees the Native Call.
    let call = unit.operations().submit_native::<NativeSleep>(Wait(Duration::from_secs(2)), ()).unwrap();
    assert_eq!(source.pending(), 1, "Register the Native Wait at Admission");
    let admission = admissions.recv().await.unwrap();
    assert_eq!(admission.scope, "unit/native");
    source.advance_by(Duration::from_secs(2));
    admission.release.send(()).unwrap();
    call.await.unwrap();
    assert_eq!(source.pending(), 0, "Settle the Native Wait");
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_during_an_in_flight_tick_settles_without_failure() {
    // Hold One Tick In Flight While the Plugin Sleeps on the Unit Clock.
    let host = HostFixture::new().await;
    let sdk = host.sdk("scoped-sleep").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/racing", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();
    source.wait_registered().await;
    source.advance_by(Duration::from_secs(1));
    loop {
        let registered = source.registered.notified();
        if source.pending() == 1 {
            break;
        }
        registered.await;
    }

    // Close Wins Over the Tick It Cancels and Leaves Nothing Behind.
    unit.close().await.unwrap();
    assert_eq!(source.pending(), 0, "Remove the In-Flight Sleep Timer");
    assert!(host.rows().is_empty(), "Publish No Sample From the Cancelled Tick");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn typed_child_sleeps_register_at_admission() {
    // A Host Operation Calls Time.Sleep as a Typed Child Inside the Unit.
    let (admitted, mut admissions) = mpsc::unbounded_channel();
    let host = HostFixture::with_middleware(Arc::new(Gate(admitted))).await;
    host.runtime.register_batch(Batch::new(Lane::default()).protobuf::<TypedSleeper>().unwrap()).await.unwrap();
    let sdk = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let source = Controlled::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/typed", Clock::with_clock(source.clone())).await.unwrap();
    unit.start_samples().await.unwrap();
    let request = TimeSleepRequest {
        duration: Some(ProtocolDuration { seconds: 1, nanos: 0 }),
    };

    // The Child's Timer Exists Before Middleware Sees the Child Call.
    let (call, _) = unit.operations().submit_protobuf::<TypedSleeper>(request, None).unwrap();
    let admission = admissions.recv().await.unwrap();
    assert_eq!(admission.scope, "unit/typed");
    assert_eq!(source.pending(), 1, "Register the Typed Child Sleep at Admission");
    source.advance_by(Duration::from_secs(1));
    admission.release.send(()).unwrap();
    call.await.unwrap();
    assert_eq!(source.pending(), 0, "Settle the Typed Child Sleep");
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
