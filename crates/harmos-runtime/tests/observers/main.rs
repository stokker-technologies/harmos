/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "plugin-sidecar")]
#[path = "../observers/execution.rs"]
mod execution;
#[cfg(feature = "plugin-sidecar")]
#[path = "../observers/host_accessor.rs"]
mod host_accessor;
#[cfg(feature = "plugin-sidecar")]
#[path = "../observers/lifecycle.rs"]
mod lifecycle;
#[cfg(feature = "plugin-sidecar")]
#[path = "../observers/native.rs"]
mod native;
#[path = "../observers/registration.rs"]
mod registration;

#[path = "../support/pending.rs"]
mod pending;
use crate::support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Semaphore;
// Local Imports
use harmos_runtime::observers::{ObserverActivation, ObserverBackend, ObserverDefinition};
use harmos_runtime::operations::resident::{Activation, Definition, Key, Kind};
use harmos_runtime::operations::{OperationManager, OperationOutcome, OperationPhase, Running};
#[cfg(feature = "plugin-sidecar")]
use harmos_runtime::sidecar::{ActivationId, Activity, ObserverError};
use harmos_runtime::{Error, Runtime};
use support::{Bank, Ledger, catalog};

/* === Constants === */

const STARTING: &[OperationPhase] = &[OperationPhase::Queued, OperationPhase::Initializing];

const RUNNING: &[OperationPhase] = &[OperationPhase::Running];

const STOPPING: &[OperationPhase] = &[OperationPhase::Stopping, OperationPhase::Finalizing];

const STOPPED: &[OperationPhase] = &[OperationPhase::Stopped];

/* === Macros === */

/* === Type Aliases === */

type Lease = Running<Activation>;

/* === Structs === */

struct FinalPublication(harmos_runtime::state::StateManager<Bank>);

struct ReentrantBackend(Reentrant);

struct Control {
    starts: Semaphore,
    initialize: Semaphore,
    stops: Semaphore,
    finish: Semaphore,
    completed: AtomicUsize,
}

struct Backend {
    id: &'static str,
    mode: Mode,
}

struct Reentrant {
    manager: OperationManager<support::Lanes>,
    dropped: Arc<AtomicBool>,
}

// Drives one kit activity the way a host does: observation runs on its own
// task until interrupted or unable to continue, and stop and cleanup are then
// driven explicitly over the returned activity.
#[cfg(feature = "plugin-sidecar")]
struct Observing {
    observer: &'static str,
    activation: ActivationId,
    ready: tokio::sync::watch::Receiver<bool>,
    interrupt: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<(Activity, Option<ObserverError>)>>,
    activity: Option<Activity>,
}

/* === Enums === */

#[derive(Clone, Copy)]
enum Mode {
    Cooperative,
    Finishing,
    Failed,
    Panicked,
}

/* === Traits === */

/* === Implementations === */

impl Control {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            starts: Semaphore::new(0),
            initialize: Semaphore::new(1),
            stops: Semaphore::new(0),
            finish: Semaphore::new(0),
            completed: AtomicUsize::new(0),
        })
    }
}

impl ObserverBackend for Backend {
    fn definition(&self) -> harmos_runtime::Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: self.id.into(),
            interval: Duration::from_millis(5),
            streams: vec![],
            demand_keys: vec![],
        })
    }

    async fn run(&self, context: Arc<harmos_sdk::ExecutionContext>, mut activation: ObserverActivation) -> harmos_runtime::Result<()> {
        let resources = context.resource::<Control>().unwrap();
        resources.starts.add_permits(1);
        tokio::select! {
            _ = activation.stopping() => return Ok(()),
            initialized = resources.initialize.acquire() => initialized.unwrap().forget(),
        }
        activation.ready();
        assert!(!matches!(self.mode, Mode::Panicked), "backend panicked");
        if matches!(self.mode, Mode::Finishing) {
            activation.begin_stopping();
        } else {
            activation.stopping().await;
        }
        resources.stops.add_permits(1);
        resources.finish.acquire().await.unwrap().forget();
        resources.completed.fetch_add(1, Ordering::SeqCst);
        if matches!(self.mode, Mode::Failed) {
            Err(Error::backend("observer cleanup", harmos_runtime::BackendFailure::Conflict, std::io::Error::other("cleanup conflict")))
        } else {
            Ok(())
        }
    }
}

impl Drop for Reentrant {
    fn drop(&mut self) {
        let _ = self.manager.resident_definitions();
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl ObserverBackend for FinalPublication {
    fn definition(&self) -> harmos_runtime::Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: "final".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        })
    }
    async fn run(&self, _: Arc<harmos_sdk::ExecutionContext>, mut activation: ObserverActivation) -> harmos_runtime::Result<()> {
        let journal = &self.0;
        activation.ready();
        activation.stopping().await;
        journal.commit(support::deposit(9)).await.map_err(|source| Error::TaskFailed {
            domain: "observer final publication",
            source: Arc::new(source),
        })?;
        Ok(())
    }
}

impl ObserverBackend for ReentrantBackend {
    fn definition(&self) -> harmos_runtime::Result<ObserverDefinition> {
        let _ = self.0.manager.resident_definitions();
        Ok(ObserverDefinition {
            id: "retained".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        })
    }
    async fn run(&self, _: Arc<harmos_sdk::ExecutionContext>, _: ObserverActivation) -> harmos_runtime::Result<()> {
        unreachable!("definition stays inert")
    }
}

#[cfg(feature = "plugin-sidecar")]
impl Observing {
    fn start(mut activity: Activity) -> Self {
        use tracing::Instrument;
        use tracing::instrument::WithSubscriber;
        let observer = activity.observer_id();
        let activation = activity.activation_id();
        let (readiness, ready) = tokio::sync::watch::channel(false);
        let (interrupt, interrupted) = tokio::sync::oneshot::channel();
        let observing = async move {
            let failure = tokio::select! {
                failure = activity.observe(move || {
                    readiness.send_replace(true);
                }) => Some(failure),
                _ = interrupted => None,
            };
            (activity, failure)
        };
        // The caller's span and subscriber follow the observation onto its task.
        let task = tokio::spawn(observing.in_current_span().with_current_subscriber());
        Self {
            observer,
            activation,
            ready,
            interrupt: Some(interrupt),
            task: Some(task),
            activity: None,
        }
    }

    fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    fn is_observing(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }

    async fn ready(&mut self) {
        self.ready.wait_for(|ready| *ready).await.expect("observer reports readiness");
    }

    // Interrupt observation; `None` when the activity was still observing.
    async fn interrupt(&mut self) -> Option<ObserverError> {
        if let Some(interrupt) = self.interrupt.take() {
            let _ = interrupt.send(());
        }
        self.ended().await
    }

    // Wait for observation to end by itself and recover the activity.
    async fn ended(&mut self) -> Option<ObserverError> {
        let task = self.task.take()?;
        let (activity, failure) = task.await.expect("observation task joins");
        self.activity = Some(activity);
        failure
    }

    fn activity(&mut self) -> &mut Activity {
        self.activity.as_mut().expect("observation ended")
    }

    async fn shutdown(&mut self) -> Result<(), ObserverError> {
        self.interrupt().await;
        let activity = self.activity();
        activity.stop().await?;
        activity.cleanup().await
    }
}

#[cfg(feature = "plugin-sidecar")]
impl Drop for Observing {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "plugin-sidecar")]
fn observer_context<T: Send + Sync + 'static>(value: Arc<T>) -> Arc<harmos_sdk::ExecutionContext> {
    let mut table = harmos_sdk::__private::WireTable::new("observer_fixture").unwrap();
    table.provide_resource(value).unwrap();
    Arc::new(table.context())
}

async fn runtime() -> Runtime<Bank> {
    runtime_with(Control::new()).await
}

async fn runtime_with<T: Send + Sync + 'static>(resource: Arc<T>) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.resource::<T>(resource);
        setup.settings().register(catalog());
    })
    .await
    .unwrap()
}

async fn reached(signal: &Semaphore) {
    signal.acquire().await.unwrap().forget();
}

async fn phase(manager: &OperationManager<support::Lanes>, key: &Key, phases: &[OperationPhase]) {
    let mut states = manager.watch_residents();
    states.wait_for(|states| states.get(key).is_some_and(|state| phases.contains(&state.lifecycle.phase()))).await.unwrap();
}

fn key(manager: &OperationManager<support::Lanes>, lease: &Lease) -> Key {
    manager.resident_key(lease.id()).expect("resident instance is registered")
}

fn leases(manager: &OperationManager<support::Lanes>, key: &Key) -> usize {
    manager.resident(key).unwrap().leases
}

fn current(manager: &OperationManager<support::Lanes>, key: &Key) -> OperationPhase {
    manager.resident(key).unwrap().lifecycle.phase()
}

fn observers(manager: &OperationManager<support::Lanes>) -> Vec<Definition> {
    manager.resident_definitions().into_iter().filter(|definition| definition.kind() == Kind::Observer).collect()
}

fn observer_instances(manager: &OperationManager<support::Lanes>) -> usize {
    manager.residents().values().filter(|snapshot| snapshot.kind == Kind::Observer).count()
}

fn register(manager: &OperationManager<support::Lanes>, id: &'static str, mode: Mode) {
    manager.register_observer(Backend { id, mode }).unwrap();
}

#[tokio::test]
async fn cancellation_skips_backend() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "queued", Mode::Cooperative);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let lease = manager.submit_resident("queued").await.unwrap();
    // The Current-Thread Executor Has Not Polled The Queued Backend Yet.
    lease.cancel();
    lease.settle().await.unwrap();
    let lifecycle = lease.status();
    assert_eq!(lifecycle.outcome(), Some(OperationOutcome::Cancelled));
    assert!(lifecycle.started_at().is_none());
    assert!(lifecycle.settled_at().is_some());
    assert_eq!(resources.starts.available_permits(), 0);
    assert_eq!(resources.completed.load(Ordering::SeqCst), 0);
    lease.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shared_demand_lifecycle() {
    let runtime = runtime().await;
    register(&runtime.operations, "watch", Mode::Cooperative);
    {
        let manager = &runtime.operations;
        let resources = runtime.operations.context().resource::<Control>().unwrap();
        let mut consumers = Vec::new();
        for _ in 0..32 {
            let manager = manager.clone();
            consumers.push(tokio::spawn(async move { manager.submit_resident("watch").await }));
        }
        let mut leases_held = Vec::new();
        for consumer in consumers {
            leases_held.push(consumer.await.unwrap().unwrap());
        }
        let last = leases_held.pop().unwrap();
        let key = key(manager, &last);
        phase(manager, &key, RUNNING).await;
        assert_eq!(leases(manager, &key), 32);
        reached(&resources.starts).await;
        assert!(resources.starts.try_acquire().is_err());
        for lease in leases_held {
            assert_eq!(lease.id(), last.id());
            lease.release().await.unwrap();
        }
        assert_eq!(leases(manager, &key), 1);
        resources.finish.add_permits(1);
        last.release().await.unwrap();
        assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
    }
    {
        let manager = runtime.handle().operations;
        assert_eq!(observers(&runtime.operations).len(), 1);
        assert!(matches!(
            manager.register_observer(Backend { id: "watch", mode: Mode::Cooperative }),
            Err(Error::AlreadyRegistered { .. })
        ));
        let resources = runtime.operations.context().resource::<Control>().unwrap();
        resources.initialize.add_permits(1);
        resources.stops.try_acquire().unwrap().forget();
        let first = manager.submit_resident("watch").await.unwrap();
        let second = runtime.operations.submit_resident("watch").await.unwrap();
        let key = key(&manager, &first);
        assert_eq!(first.id(), second.id());
        assert_eq!(manager.resident(&key).unwrap().instance, first.id());
        reached(&resources.starts).await;
        phase(&manager, &key, RUNNING).await;
        assert_eq!(leases(&manager, &key), 2);

        first.release().await.unwrap();
        assert_eq!(leases(&manager, &key), 1);
        assert!(resources.stops.try_acquire().is_err());
        drop(second);
        reached(&resources.stops).await;
        pending::parked(std::pin::pin!(manager.stop_observers())).await;
        resources.finish.add_permits(1);
        runtime.shutdown().await.unwrap();
        assert_eq!(resources.completed.load(Ordering::SeqCst), 2);
        assert_eq!(current(&manager, &key), OperationPhase::Stopped);
        assert!(matches!(manager.submit_resident("watch").await, Err(Error::Closing { .. })));
    }
}

#[tokio::test]
async fn activation_waits_cancelled() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "watch", Mode::Cooperative);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let first = manager.submit_resident("watch").await.unwrap();
    let key = key(manager, &first);
    let generation = first.id();
    phase(manager, &key, RUNNING).await;
    pending::parked(std::pin::pin!(first.release())).await;
    reached(&resources.stops).await;
    pending::parked(std::pin::pin!(manager.submit_resident("watch"))).await;
    resources.initialize.add_permits(1);
    resources.finish.add_permits(1);
    let next = manager.submit_resident("watch").await.unwrap();
    assert_ne!(next.id(), generation);
    phase(manager, &key, RUNNING).await;
    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
    resources.finish.add_permits(1);
    next.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn instance_cancel_joins() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "watch", Mode::Cooperative);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let first = manager.submit_resident("watch").await.unwrap();
    let second = manager.submit_resident("watch").await.unwrap();
    let key = key(manager, &first);
    reached(&resources.starts).await;
    phase(manager, &key, RUNNING).await;
    assert_eq!(leases(manager, &key), 2);
    assert_eq!(first.id(), second.id());
    first.cancel();
    first.cancel();
    reached(&resources.stops).await;
    {
        let settling = first.settle();
        tokio::pin!(settling);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(settling.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
    }
    assert!(!first.status().phase().is_terminal());
    assert!(second.status().cancellation_requested());
    assert!(first.status().settled_at().is_none());
    resources.finish.add_permits(1);
    first.settle().await.unwrap();
    assert!(first.status().phase().is_terminal());
    assert!(second.status().settled_at().is_some());
    assert_eq!(first.status().outcome(), Some(OperationOutcome::Cancelled));
    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);

    resources.initialize.add_permits(1);
    let replacement = manager.submit_resident("watch").await.unwrap();
    phase(manager, &key, RUNNING).await;
    assert_ne!(replacement.id(), first.id());
    first.cancel();
    first.settle().await.unwrap();
    first.release().await.unwrap();
    second.release().await.unwrap();
    assert_eq!(leases(manager, &key), 1);
    assert_eq!(current(manager, &key), OperationPhase::Running);
    resources.finish.add_permits(1);
    replacement.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cannot_release_replacement() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "watch", Mode::Finishing);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let old = manager.submit_resident("watch").await.unwrap();
    let key = key(manager, &old);
    reached(&resources.stops).await;
    phase(manager, &key, STOPPING).await;
    resources.finish.add_permits(1);
    phase(manager, &key, STOPPED).await;
    resources.initialize.add_permits(1);
    let next = manager.submit_resident("watch").await.unwrap();
    assert_ne!(old.id(), next.id());
    old.release().await.unwrap();
    assert_eq!(leases(manager, &key), 1);
    reached(&resources.stops).await;
    resources.finish.add_permits(1);
    next.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn unregister_reserves_name() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "watch", Mode::Cooperative);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let old = manager.submit_resident("watch").await.unwrap();
    phase(manager, &key(manager, &old), RUNNING).await;
    pending::parked(std::pin::pin!(manager.unregister_resident("watch"))).await;
    assert!(manager.resident_definition("watch").is_some());
    assert!(matches!(manager.submit_resident("watch").await, Err(Error::Closing { .. })));
    assert!(matches!(manager.register_observer(Backend { id: "watch", mode: Mode::Cooperative }), Err(Error::AlreadyRegistered { .. })));
    resources.finish.add_permits(1);
    manager.unregister_resident("watch").await.unwrap();
    assert!(manager.resident_definition("watch").is_none());
    register(manager, "watch", Mode::Cooperative);
    resources.initialize.add_permits(1);
    let next = manager.submit_resident("watch").await.unwrap();
    old.release().await.unwrap();
    let next_key = key(manager, &next);
    phase(manager, &next_key, RUNNING).await;
    assert_eq!(leases(manager, &next_key), 1);
    resources.finish.add_permits(1);
    next.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn instances_retain_runtime() {
    let first = runtime().await;
    let second = runtime().await;
    register(&first.operations, "watch", Mode::Cooperative);
    register(&second.operations, "watch", Mode::Cooperative);
    let first_control = first.operations.context().resource::<Control>().unwrap();
    let second_control = second.operations.context().resource::<Control>().unwrap();

    let one = first.operations.submit_resident("watch").await.unwrap();
    let two = second.operations.submit_resident("watch").await.unwrap();

    assert_ne!(one.id(), two.id());
    assert!(first.operations.resident_key(two.id()).is_none());
    assert!(second.operations.resident_key(one.id()).is_none());
    assert_eq!(leases(&first.operations, &key(&first.operations, &one)), 1);
    assert_eq!(leases(&second.operations, &key(&second.operations, &two)), 1);
    assert!(matches!(first.operations.submit_resident("absent").await, Err(Error::NotRegistered { .. })));
    assert!(matches!(first.operations.register_observer(Backend { id: "", mode: Mode::Cooperative }), Err(Error::InvalidInput { .. })));

    first_control.finish.add_permits(1);
    second_control.finish.add_permits(1);
    drop((one, two));
    first.shutdown().await.unwrap();
    second.shutdown().await.unwrap();
}

#[tokio::test]
async fn runtime_retains_failure() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "bad", Mode::Failed);
    register(manager, "good", Mode::Cooperative);
    let control = runtime.operations.context().resource::<Control>().unwrap();
    control.initialize.add_permits(1);
    let a = manager.submit_resident("bad").await.unwrap();
    let b = manager.submit_resident("good").await.unwrap();
    let a_key = key(manager, &a);
    phase(manager, &a_key, RUNNING).await;
    phase(manager, &key(manager, &b), RUNNING).await;
    control.finish.add_permits(2);
    for _ in 0..2 {
        assert!(matches!(runtime.shutdown().await, Err(Error::TaskFailed { domain: "observer", .. })));
    }
    assert_eq!(control.completed.load(Ordering::SeqCst), 2);
    assert!(matches!(
        manager.resident(&a_key).unwrap().failure.as_deref(),
        Some(Error::Backend {
            kind: harmos_runtime::BackendFailure::Conflict,
            ..
        })
    ));
    assert!(a.release().await.is_err());
    b.release().await.unwrap();
}

#[tokio::test]
async fn backend_panic_joined() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "panic", Mode::Panicked);
    let lease = manager.submit_resident("panic").await.unwrap();
    let key = key(manager, &lease);
    phase(manager, &key, STOPPING).await;
    let status = manager.resident(&key).unwrap();
    let Some(Error::TaskFailed { source, .. }) = status.failure.as_deref() else { panic!("typed panic cause") };
    assert!(source.downcast_ref::<tokio::task::JoinError>().unwrap().is_panic());
    assert!(lease.release().await.is_err());
    assert!(runtime.shutdown().await.is_err());
}

#[tokio::test]
async fn shutdown_cancels_initialization() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    register(manager, "watch", Mode::Cooperative);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    resources.initialize.acquire().await.unwrap().forget();
    let lease = manager.submit_resident("watch").await.unwrap();
    let key = key(manager, &lease);
    reached(&resources.starts).await;
    assert!(STARTING.contains(&current(manager, &key)));
    runtime.shutdown().await.unwrap();
    assert_eq!(current(manager, &key), OperationPhase::Stopped);
    assert!(resources.stops.try_acquire().is_err());
    lease.release().await.unwrap();
}

#[tokio::test]
async fn runtime_joins_final() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    manager.register_observer(FinalPublication(runtime.state.clone())).unwrap();
    let lease = manager.submit_resident("final").await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    let before = runtime.state.applied();
    let stopped = runtime.shutdown().await.unwrap();
    assert!(stopped > before);
    assert_eq!(stopped, runtime.state.applied());
    assert!(matches!(runtime.state.commit(support::deposit(1)).await, Err(Error::Unavailable)));
    lease.release().await.unwrap();
}

#[tokio::test]
async fn shutdown_releases_factories() {
    let runtime = runtime().await;
    let dropped = Arc::new(AtomicBool::new(false));
    runtime
        .operations
        .register_observer(ReentrantBackend(Reentrant {
            manager: runtime.handle().operations,
            dropped: dropped.clone(),
        }))
        .unwrap();
    runtime.shutdown().await.unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(observers(&runtime.operations).is_empty());
}

#[tokio::test]
async fn closing_preserves_failure() {
    let runtime = runtime().await;
    register(&runtime.operations, "watch", Mode::Failed);
    let resources = runtime.operations.context().resource::<Control>().unwrap();
    let first = runtime.operations.submit_resident("watch").await.unwrap();
    let key = key(&runtime.operations, &first);
    phase(&runtime.operations, &key, RUNNING).await;
    drop(first);
    reached(&resources.stops).await;
    let manager = runtime.operations.clone();
    let mut activating = tokio::spawn(async move { manager.submit_resident("watch").await.map(|lease| lease.id()) });
    pending::parked(std::pin::Pin::new(&mut activating)).await;
    pending::parked(std::pin::pin!(runtime.shutdown())).await;
    resources.finish.add_permits(1);
    assert!(matches!(activating.await.unwrap(), Err(Error::Closing { .. })));
    assert!(matches!(runtime.shutdown().await, Err(Error::TaskFailed { domain: "observer", .. })));
    assert!(runtime.operations.resident(&key).unwrap().failure.is_some());
}

/* === Tests === */
