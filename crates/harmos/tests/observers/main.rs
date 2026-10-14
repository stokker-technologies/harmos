/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[cfg(feature = "plugin-sidecar")]
mod execution;
#[cfg(feature = "plugin-sidecar")]
mod lifecycle;
#[cfg(feature = "plugin-sidecar")]
mod native;
mod registration;

#[path = "../support/mod.rs"]
mod support;

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
use harmos::observers::{ObserverActivation, ObserverBackend, ObserverDefinition};
use harmos::operations::resident::{Activation, Definition, Key, Kind};
use harmos::operations::{OperationInstance, OperationManager, OperationOutcome, OperationPhase};
#[cfg(feature = "plugin-sidecar")]
use harmos::sidecar::{ActivationId, Activity, ObserverError};
use harmos::{Error, Runtime};
use support::{Bank, Ledger, catalog};

/* === Constants === */
const STARTING: &[OperationPhase] = &[OperationPhase::Queued, OperationPhase::Initializing];

const RUNNING: &[OperationPhase] = &[OperationPhase::Running];

const STOPPING: &[OperationPhase] = &[OperationPhase::Stopping, OperationPhase::Finalizing];

const STOPPED: &[OperationPhase] = &[OperationPhase::Stopped];

/* === Macros === */

/* === Type Aliases === */
type Lease = OperationInstance<Activation>;

/* === Structs === */
struct FinalPublication;

struct ReentrantBackend(Reentrant);

struct Unused;

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
    manager: OperationManager,
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
    type Resources = Control;
    fn definition(&self) -> harmos::Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: self.id.into(),
            interval: Duration::from_millis(5),
            streams: vec![],
            demand_keys: vec![],
        })
    }

    async fn run(
        &self,
        resources: Arc<Control>,
        mut activation: ObserverActivation,
    ) -> harmos::Result<()> {
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
            Err(Error::backend(
                "observer cleanup",
                harmos::BackendFailure::Conflict,
                std::io::Error::other("cleanup conflict"),
            ))
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
    type Resources = harmos::state::StateManager<Bank>;
    fn definition(&self) -> harmos::Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: "final".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        })
    }
    async fn run(
        &self,
        journal: Arc<Self::Resources>,
        mut activation: ObserverActivation,
    ) -> harmos::Result<()> {
        activation.ready();
        activation.stopping().await;
        journal
            .commit(support::deposit(9))
            .await
            .map_err(|source| Error::TaskFailed {
                domain: "observer final publication",
                source: Arc::new(source),
            })?;
        Ok(())
    }
}

impl ObserverBackend for ReentrantBackend {
    type Resources = ();
    fn definition(&self) -> harmos::Result<ObserverDefinition> {
        let _ = self.0.manager.resident_definitions();
        Ok(ObserverDefinition {
            id: "retained".into(),
            interval: Duration::from_secs(1),
            streams: vec![],
            demand_keys: vec![],
        })
    }
    async fn run(&self, _: Arc<()>, _: ObserverActivation) -> harmos::Result<()> {
        unreachable!("definition stays inert")
    }
}

impl ObserverBackend for Unused {
    type Resources = Reentrant;
    fn definition(&self) -> harmos::Result<ObserverDefinition> {
        unreachable!("no unused backend is staged")
    }
    async fn run(&self, _: Arc<Reentrant>, _: ObserverActivation) -> harmos::Result<()> {
        unreachable!("no unused backend is staged")
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
        tokio::time::timeout(Duration::from_secs(3), self.ready.wait_for(|ready| *ready))
            .await
            .expect("observer becomes ready")
            .expect("observer reports readiness");
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
async fn runtime() -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime()
        .await
        .unwrap()
}

async fn reached(signal: &Semaphore) {
    tokio::time::timeout(Duration::from_secs(2), signal.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
}

async fn phase(manager: &OperationManager, key: &Key, phases: &[OperationPhase]) {
    let mut states = manager.watch_residents();
    tokio::time::timeout(
        Duration::from_secs(2),
        states.wait_for(|states| {
            states
                .get(key)
                .is_some_and(|state| phases.contains(&state.lifecycle.phase()))
        }),
    )
    .await
    .unwrap()
    .unwrap();
}

fn key(manager: &OperationManager, lease: &Lease) -> Key {
    manager
        .resident_key(lease.id())
        .expect("resident instance is registered")
}

fn leases(manager: &OperationManager, key: &Key) -> usize {
    manager.resident(key).unwrap().leases
}

fn current(manager: &OperationManager, key: &Key) -> OperationPhase {
    manager.resident(key).unwrap().lifecycle.phase()
}

fn observers(manager: &OperationManager) -> Vec<Definition> {
    manager
        .resident_definitions()
        .into_iter()
        .filter(|definition| definition.kind() == Kind::Observer)
        .collect()
}

fn observer_instances(manager: &OperationManager) -> usize {
    manager
        .residents()
        .values()
        .filter(|snapshot| snapshot.kind == Kind::Observer)
        .count()
}

fn register(manager: &OperationManager, id: &'static str, mode: Mode) {
    manager.register_observer(Backend { id, mode }).unwrap();
}

fn own_contract() {
    {
        scenario_runtime::check("cancellation_skips_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "queued", Mode::Cooperative);
                    let resources = Control::new();
                    let lease = manager
                        .submit_with_resources_at("queued", resources.clone())
                        .await
                        .unwrap();
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
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("clones_share_activation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = runtime.handle().operations;
                    register(&manager, "watch", Mode::Cooperative);
                    assert_eq!(observers(&runtime.operations).len(), 1);
                    assert!(matches!(
                        manager.register_observer(Backend {
                            id: "watch",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    let resources = Control::new();
                    let first = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let second = runtime
                        .operations
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
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
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), manager.stop_observers())
                            .await
                            .is_err()
                    );
                    resources.finish.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
                    assert_eq!(current(&manager, &key), OperationPhase::Stopped);
                    assert!(matches!(
                        manager
                            .submit_with_resources_at("watch", resources.clone())
                            .await,
                        Err(Error::Closing { .. })
                    ));
                });
        });
    }
    {
        scenario_runtime::check("activation_waits_cancelled", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let first = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let key = key(manager, &first);
                    let generation = first.id();
                    phase(manager, &key, RUNNING).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), first.release())
                            .await
                            .is_err()
                    );
                    reached(&resources.stops).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            manager.submit_with_resources_at("watch", resources.clone())
                        )
                        .await
                        .is_err()
                    );
                    resources.initialize.add_permits(1);
                    resources.finish.add_permits(1);
                    let next = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    assert_ne!(next.id(), generation);
                    phase(manager, &key, RUNNING).await;
                    assert_eq!(resources.completed.load(Ordering::SeqCst), 1);
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("instance_cancel_joins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let first = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let second = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
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
                    let replacement = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
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
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cannot_release_replacement", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Finishing);
                    let resources = Control::new();
                    let old = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let key = key(manager, &old);
                    reached(&resources.stops).await;
                    phase(manager, &key, STOPPING).await;
                    resources.finish.add_permits(1);
                    phase(manager, &key, STOPPED).await;
                    resources.initialize.add_permits(1);
                    let next = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    assert_ne!(old.id(), next.id());
                    old.release().await.unwrap();
                    assert_eq!(leases(manager, &key), 1);
                    reached(&resources.stops).await;
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scope_stop_revokes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let first = Control::new();
                    let second = Control::new();
                    let a = manager
                        .submit_with_resources_at("watch", first.clone())
                        .await
                        .unwrap();
                    let b = manager
                        .submit_with_resources_at("watch", second.clone())
                        .await
                        .unwrap();
                    let (a_key, b_key) = (key(manager, &a), key(manager, &b));
                    phase(manager, &a_key, RUNNING).await;
                    phase(manager, &b_key, RUNNING).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            manager.retire_resources(&first)
                        )
                        .await
                        .is_err()
                    );
                    assert!(matches!(
                        manager
                            .submit_with_resources_at("watch", first.clone())
                            .await,
                        Err(Error::Closing { .. })
                    ));
                    assert_eq!(current(manager, &b_key), OperationPhase::Running);
                    first.finish.add_permits(1);
                    manager.retire_resources(&first).await.unwrap();
                    assert!(manager.resident(&a_key).is_none());
                    a.release().await.unwrap();
                    second.finish.add_permits(1);
                    b.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("unregister_reserves_name", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let old = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    phase(manager, &key(manager, &old), RUNNING).await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            manager.unregister_resident("watch")
                        )
                        .await
                        .is_err()
                    );
                    assert!(manager.resident_definition("watch").is_some());
                    assert!(matches!(
                        manager
                            .submit_with_resources_at("watch", resources.clone())
                            .await,
                        Err(Error::Closing { .. })
                    ));
                    assert!(matches!(
                        manager.register_observer(Backend {
                            id: "watch",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    resources.finish.add_permits(1);
                    manager.unregister_resident("watch").await.unwrap();
                    assert!(manager.resident_definition("watch").is_none());
                    register(manager, "watch", Mode::Cooperative);
                    resources.initialize.add_permits(1);
                    let next = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    old.release().await.unwrap();
                    let next_key = key(manager, &next);
                    phase(manager, &next_key, RUNNING).await;
                    assert_eq!(leases(manager, &next_key), 1);
                    resources.finish.add_permits(1);
                    next.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scopes_reject_foreigners", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let first = runtime().await;
                    let second = runtime().await;
                    register(&first.operations, "watch", Mode::Cooperative);
                    register(&second.operations, "watch", Mode::Cooperative);
                    // Resources Shared Across Runtimes Open Independent Scopes.
                    let shared = Control::new();
                    let one = first
                        .operations
                        .submit_with_resources_at("watch", shared.clone())
                        .await
                        .unwrap();
                    let two = second
                        .operations
                        .submit_with_resources_at("watch", shared.clone())
                        .await
                        .unwrap();
                    assert_ne!(one.id(), two.id());
                    assert!(first.operations.resident_key(two.id()).is_none());
                    assert!(second.operations.resident_key(one.id()).is_none());
                    assert_eq!(leases(&first.operations, &key(&first.operations, &one)), 1);
                    assert_eq!(
                        leases(&second.operations, &key(&second.operations, &two)),
                        1
                    );
                    assert!(matches!(
                        first
                            .operations
                            .submit_with_resources_at("watch", Arc::new(()))
                            .await,
                        Err(Error::InvalidInput { .. })
                    ));
                    assert!(matches!(
                        first
                            .operations
                            .submit_with_resources_at("absent", shared.clone())
                            .await,
                        Err(Error::NotRegistered { .. })
                    ));
                    assert!(matches!(
                        first.operations.register_observer(Backend {
                            id: "",
                            mode: Mode::Cooperative
                        }),
                        Err(Error::InvalidInput { .. })
                    ));
                    shared.finish.add_permits(1);
                    drop((one, two));
                    first.stop().await.unwrap();
                    second.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("runtime_retains_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "bad", Mode::Failed);
                    register(manager, "good", Mode::Cooperative);
                    let bad = Control::new();
                    let good = Control::new();
                    let a = manager
                        .submit_with_resources_at("bad", bad.clone())
                        .await
                        .unwrap();
                    let b = manager
                        .submit_with_resources_at("good", good.clone())
                        .await
                        .unwrap();
                    let a_key = key(manager, &a);
                    phase(manager, &a_key, RUNNING).await;
                    phase(manager, &key(manager, &b), RUNNING).await;
                    bad.finish.add_permits(1);
                    good.finish.add_permits(1);
                    for _ in 0..2 {
                        assert!(matches!(
                            runtime.stop().await,
                            Err(Error::TaskFailed {
                                domain: "observer",
                                ..
                            })
                        ));
                    }
                    assert_eq!(good.completed.load(Ordering::SeqCst), 1);
                    assert_eq!(bad.completed.load(Ordering::SeqCst), 1);
                    assert!(matches!(
                        manager.resident(&a_key).unwrap().failure.as_deref(),
                        Some(Error::Backend {
                            kind: harmos::BackendFailure::Conflict,
                            ..
                        })
                    ));
                    assert!(a.release().await.is_err());
                    b.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("backend_panic_joined", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "panic", Mode::Panicked);
                    let lease = manager
                        .submit_with_resources_at("panic", Control::new())
                        .await
                        .unwrap();
                    let key = key(manager, &lease);
                    phase(manager, &key, STOPPING).await;
                    let status = manager.resident(&key).unwrap();
                    let Some(Error::TaskFailed { source, .. }) = status.failure.as_deref() else {
                        panic!("typed panic cause")
                    };
                    assert!(
                        source
                            .downcast_ref::<tokio::task::JoinError>()
                            .unwrap()
                            .is_panic()
                    );
                    assert!(lease.release().await.is_err());
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
    {
        scenario_runtime::check("shutdown_drops_unused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let dropped = Arc::new(AtomicBool::new(false));
                    let resources = Arc::new(Reentrant {
                        manager: runtime.operations.clone(),
                        dropped: dropped.clone(),
                    });
                    let weak = Arc::downgrade(&resources);
                    // Staging an empty catalog opens the scope without activating anything.
                    drop(
                        runtime
                            .operations
                            .stage_observers(&resources, std::iter::empty::<Unused>())
                            .unwrap(),
                    );
                    drop(resources);
                    assert!(weak.upgrade().is_some());
                    runtime.stop().await.unwrap();
                    assert!(weak.upgrade().is_none());
                    assert!(dropped.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("shutdown_cancels_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    resources.initialize.acquire().await.unwrap().forget();
                    let lease = manager
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let key = key(manager, &lease);
                    reached(&resources.starts).await;
                    assert!(STARTING.contains(&current(manager, &key)));
                    runtime.stop().await.unwrap();
                    assert_eq!(current(manager, &key), OperationPhase::Stopped);
                    assert!(resources.stops.try_acquire().is_err());
                    lease.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("runtime_joins_final", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    manager.register_observer(FinalPublication).unwrap();
                    let lease = manager
                        .submit_with_resources_at("final", Arc::new(runtime.state.clone()))
                        .await
                        .unwrap();
                    phase(manager, &key(manager, &lease), RUNNING).await;
                    let before = runtime.state.applied();
                    let stopped = runtime.stop().await.unwrap();
                    assert!(stopped > before);
                    assert_eq!(stopped, runtime.state.applied());
                    assert!(matches!(
                        runtime.state.commit(support::deposit(1)).await,
                        Err(Error::Unavailable)
                    ));
                    lease.release().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("consumers_share_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "watch", Mode::Cooperative);
                    let resources = Control::new();
                    let mut consumers = Vec::new();
                    for _ in 0..32 {
                        let manager = manager.clone();
                        let resources = resources.clone();
                        consumers.push(tokio::spawn(async move {
                            manager.submit_with_resources_at("watch", resources).await
                        }));
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
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_releases_factories", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let dropped = Arc::new(AtomicBool::new(false));
                    runtime
                        .operations
                        .register_observer(ReentrantBackend(Reentrant {
                            manager: runtime.handle().operations,
                            dropped: dropped.clone(),
                        }))
                        .unwrap();
                    runtime.stop().await.unwrap();
                    assert!(dropped.load(Ordering::SeqCst));
                    assert!(observers(&runtime.operations).is_empty());
                });
        });
    }
    {
        scenario_runtime::check("closing_preserves_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    register(&runtime.operations, "watch", Mode::Failed);
                    let resources = Control::new();
                    let first = runtime
                        .operations
                        .submit_with_resources_at("watch", resources.clone())
                        .await
                        .unwrap();
                    let key = key(&runtime.operations, &first);
                    phase(&runtime.operations, &key, RUNNING).await;
                    drop(first);
                    reached(&resources.stops).await;
                    let manager = runtime.operations.clone();
                    let reactivated = resources.clone();
                    let mut activating = tokio::spawn(async move {
                        manager
                            .submit_with_resources_at("watch", reactivated)
                            .await
                            .map(|lease| lease.id())
                    });
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), &mut activating)
                            .await
                            .is_err()
                    );
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), runtime.stop())
                            .await
                            .is_err()
                    );
                    resources.finish.add_permits(1);
                    assert!(matches!(
                        activating.await.unwrap(),
                        Err(Error::Closing { .. })
                    ));
                    assert!(matches!(
                        runtime.stop().await,
                        Err(Error::TaskFailed {
                            domain: "observer",
                            ..
                        })
                    ));
                    assert!(runtime.operations.resident(&key).unwrap().failure.is_some());
                });
        });
    }
    {
        scenario_runtime::check("retirement_replays_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "returned-failure", Mode::Failed);
                    let control = Control::new();
                    let lease = manager
                        .submit_with_resources_at("returned-failure", control.clone())
                        .await
                        .unwrap();
                    let key = key(manager, &lease);
                    phase(manager, &key, RUNNING).await;
                    control.finish.add_permits(1);
                    assert!(manager.retire_resources(&control).await.is_err());
                    // Retired resources are forgotten; retiring them again has nothing to stop.
                    manager.retire_resources(&control).await.unwrap();
                    assert_eq!(control.completed.load(Ordering::Acquire), 1);
                    assert!(manager.resident(&key).is_none());
                    assert!(lease.release().await.is_err());
                    for _ in 0..2 {
                        let error = runtime
                            .stop()
                            .await
                            .expect_err("retired failure reaches root shutdown");
                        let Error::TaskFailed {
                            domain: "observer",
                            source,
                        } = &error
                        else {
                            panic!("root retains observer failure: {error:?}");
                        };
                        let Some(Error::TaskFailed {
                            domain: "retired observer",
                            source,
                        }) = source.downcast_ref::<Error>()
                        else {
                            panic!("retired scope retains its cause: {error:?}");
                        };
                        let Some(Error::TaskFailed { source, .. }) = source.downcast_ref::<Error>()
                        else {
                            panic!("original observer outcome is preserved: {error:?}");
                        };
                        assert!(matches!(
                            source.downcast_ref::<Error>(),
                            Some(Error::Backend {
                                operation: "observer cleanup",
                                kind: harmos::BackendFailure::Conflict,
                                ..
                            })
                        ));
                    }
                    assert_eq!(control.completed.load(Ordering::Acquire), 1);
                });
        });
    }
    {
        scenario_runtime::check("retirement_retains_panicked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.operations;
                    register(manager, "panicked-scope", Mode::Panicked);
                    let resources = Control::new();
                    let lease = manager
                        .submit_with_resources_at("panicked-scope", resources.clone())
                        .await
                        .unwrap();
                    let key = key(manager, &lease);
                    phase(manager, &key, STOPPING).await;
                    let generation = lease.id();
                    for _ in 0..2 {
                        assert!(manager.retire_resources(&resources).await.is_err());
                        let lifecycle = lease.status();
                        assert_eq!(lifecycle.outcome(), Some(OperationOutcome::Failed));
                        assert!(lifecycle.cleanup_pending());
                        assert!(lifecycle.cleanup_failure().is_some());
                        assert!(lifecycle.settled_at().is_none());
                        assert_eq!(manager.resident(&key).unwrap().instance, generation);
                        assert!(
                            manager
                                .submit_with_resources_at("panicked-scope", resources.clone())
                                .await
                                .is_err()
                        );
                    }
                    assert!(lease.release().await.is_err());
                    assert!(runtime.stop().await.is_err());
                });
        });
    }
}

/* === Tests === */

#[test]
fn observer_lifecycle() {
    own_contract();
    registration::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    lifecycle::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    execution::contract_scenario();
    #[cfg(feature = "plugin-sidecar")]
    native::contract_scenario();
}
