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
use std::io::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Semaphore, watch};
// Local Imports
use super::{RUNNING, STOPPING, key, pending, phase};
use harmos_runtime::operations::OperationPhase;
use harmos_runtime::sidecar::{Instance, Observer, ObserverContext, ObserverError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Counts {
    initialized: AtomicUsize,
    stopped: AtomicUsize,
    ticks: AtomicUsize,
    cleanup: Semaphore,
    refuse_stop: AtomicBool,
    refuse_finalize: AtomicBool,
    finalization: Semaphore,
    finalizations: AtomicUsize,
    stop_attempts: AtomicUsize,
    dropped: AtomicUsize,
    // Announces every observed tick.
    progress: watch::Sender<()>,
}

struct Watch<const PANIC: bool>(Arc<Counts>);

#[harmos_runtime::message]
struct ConfiguredRow {
    value: String,
}

struct ConfiguredWatch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::sidecar::StreamSpec for ConfiguredRow {
    type Row = Self;
    const ID: &'static str = "schema_budget.row";
    const VERSION: u32 = 1;
    const WINDOW: usize = 1;
}

impl Observer<()> for ConfiguredWatch {
    type Error = Error;
    const ID: &'static str = "schema-budget.native";
    const INTERVAL: Duration = Duration::from_millis(5);
    const STREAMS: &'static [harmos_runtime::sidecar::inventory::publications::Entry] =
        &[harmos_runtime::sidecar::inventory::publications::Entry::of::<ConfiguredRow>()];

    async fn initialize(_: &harmos_sdk::ExecutionContext) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn observe(&mut self, _: &ObserverContext<'_, ()>) -> Result<(), Error> {
        Ok(())
    }
}

impl Counts {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            initialized: AtomicUsize::new(0),
            stopped: AtomicUsize::new(0),
            ticks: AtomicUsize::new(0),
            cleanup: Semaphore::new(0),
            refuse_stop: AtomicBool::new(false),
            refuse_finalize: AtomicBool::new(false),
            finalization: Semaphore::new(1),
            finalizations: AtomicUsize::new(0),
            stop_attempts: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
            progress: watch::Sender::new(()),
        })
    }
}

impl<const PANIC: bool> Observer<()> for Watch<PANIC> {
    type Error = Error;
    const ID: &'static str = "native";
    const INTERVAL: Duration = Duration::from_millis(5);
    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, Error> {
        let resolved = context.resource::<Counts>().unwrap();
        let resources = &resolved;
        resources.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(Self(resources.clone()))
    }
    async fn observe(&mut self, _: &ObserverContext<'_, ()>) -> Result<(), Error> {
        self.0.ticks.fetch_add(1, Ordering::SeqCst);
        self.0.progress.send_replace(());
        assert!(!PANIC, "native callback panic");
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Error> {
        self.0.stop_attempts.fetch_add(1, Ordering::SeqCst);
        if self.0.refuse_stop.swap(false, Ordering::SeqCst) {
            return Err(Error::other("native cleanup refusal"));
        }
        self.0.cleanup.acquire().await.unwrap().forget();
        self.0.stopped.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Error> {
        self.0.finalizations.fetch_add(1, Ordering::SeqCst);
        if self.0.refuse_finalize.swap(false, Ordering::SeqCst) {
            return Err(Error::other("native finalization refusal"));
        }
        self.0.finalization.acquire().await.unwrap().forget();
        Ok(())
    }
}

impl<const PANIC: bool> Drop for Watch<PANIC> {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn watch<const PANIC: bool>() -> Instance<()> {
    Instance::<()>::of::<Watch<PANIC>>()
}

#[tokio::test]
async fn native_observer_definition_uses_supplied_schema_snapshot() {
    for accepted in [true, false] {
        let mut settings = harmos_runtime::config::SchemaSettings::default();
        if !accepted {
            settings.preparation_bytes = std::num::NonZeroUsize::new(1).unwrap();
        }
        let runtime = harmos_runtime::Runtime::<super::Bank>::launch_with(super::Ledger::default(), (), |setup| {
            setup.settings().schema_settings(settings).register(super::catalog());
        })
        .await
        .unwrap();
        let result = runtime.operations.register_native_observer(Instance::<()>::of::<ConfiguredWatch>(), |_| ());
        assert_eq!(result.is_ok(), accepted);
        let definitions = runtime.operations.resident_definitions();
        let present = definitions.iter().any(|definition| {
            matches!(
                definition,
                harmos_runtime::operations::resident::Definition::Observer(observer) if observer.id == "schema-budget.native"
            )
        });
        assert_eq!(present, accepted);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn leases_share_existing() {
    let counts = Counts::new();
    let runtime = super::runtime_with(counts.clone()).await;
    let manager = &runtime.operations;
    // Native observers run on the runtime executor, not a dedicated thread.
    let owner_thread = std::thread::current().id();
    manager
        .register_native_observer(watch::<false>(), move |_| {
            assert_eq!(std::thread::current().id(), owner_thread);
        })
        .unwrap();

    let first = manager.submit_resident("native").await.unwrap();
    let second = manager.submit_resident("native").await.unwrap();
    phase(manager, &key(manager, &first), RUNNING).await;
    counts.progress.subscribe().wait_for(|_| counts.ticks.load(Ordering::SeqCst) >= 2).await.unwrap();
    first.release().await.unwrap();
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
    counts.cleanup.add_permits(1);
    runtime.shutdown().await.unwrap();
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    let ticks = counts.ticks.load(Ordering::SeqCst);
    second.release().await.unwrap();
    assert_eq!(counts.ticks.load(Ordering::SeqCst), ticks);
}

#[tokio::test]
async fn native_panic_closes() {
    let counts = Counts::new();
    let runtime = super::runtime_with(counts.clone()).await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<true>(), |_| ()).unwrap();

    let lease = manager.submit_resident("native").await.unwrap();
    phase(manager, &key(manager, &lease), STOPPING).await;
    pending::parked(std::pin::pin!(manager.submit_resident("native"))).await;
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    counts.cleanup.add_permits(1);
    let failure = lease.release().await.unwrap_err();
    let harmos_runtime::Error::TaskFailed { source, .. } = failure else { panic!("managed failure") };
    let Some(harmos_runtime::Error::TaskFailed { source, .. }) = source.downcast_ref::<harmos_runtime::Error>() else {
        panic!("native failure remains a root error");
    };
    assert!(matches!(source.downcast_ref::<ObserverError>(), Some(ObserverError::CallbackPanicked)));
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(runtime.shutdown().await.is_err());
}

#[tokio::test]
async fn binding_owns_cleanup() {
    let counts = Counts::new();
    let runtime = super::runtime_with(counts.clone()).await;
    let manager = &runtime.operations;

    let bindings = Arc::new(AtomicUsize::new(0));
    let bound = bindings.clone();
    let owner_thread = std::thread::current().id();
    let candidate = manager
        .stage_native_observers([watch::<false>()], move |_| {
            assert_eq!(std::thread::current().id(), owner_thread);
            bound.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    candidate.commit().unwrap();
    assert_eq!(bindings.load(Ordering::SeqCst), 0);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 0);
    let first = manager.submit_resident("native").await.unwrap();
    let second = manager.submit_resident("native").await.unwrap();
    phase(manager, &key(manager, &first), RUNNING).await;
    assert_eq!(first.id(), second.id());
    assert_eq!(bindings.load(Ordering::SeqCst), 1);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    counts.cleanup.add_permits(1);
    manager.unregister_resident("native").await.unwrap();
    first.release().await.unwrap();
    second.release().await.unwrap();
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(manager.resident_definition("native").is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failure_keeps_manager() {
    cleanup_recovery(false).await;
}

#[tokio::test]
async fn failure_preserves_cleanup() {
    cleanup_recovery(true).await;
}

#[tokio::test]
async fn handles_keep_cleanup() {
    let counts = Counts::new();
    let runtime = super::runtime_with(counts.clone()).await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<false>(), |_| ()).unwrap();
    counts.refuse_stop.store(true, Ordering::SeqCst);
    let lease = manager.submit_resident("native").await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    lease.request_cancel();
    assert!(lease.settle().await.is_err());
    drop(lease);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 1);

    counts.cleanup.add_permits(1);
    assert!(manager.stop_observers().await.is_err());
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 1);
    assert!(runtime.shutdown().await.is_err());
}

async fn cleanup_recovery(finalizing: bool) {
    let counts = Counts::new();
    let runtime = super::runtime_with(counts.clone()).await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<false>(), |_| ()).unwrap();
    if finalizing {
        counts.cleanup.add_permits(1);
        counts.finalization.try_acquire().unwrap().forget();
        counts.refuse_finalize.store(true, Ordering::SeqCst);
    } else {
        counts.refuse_stop.store(true, Ordering::SeqCst);
    }
    // A refused stop is retained while stopping; a refused cleanup while
    // finalizing.
    let expected = if finalizing { OperationPhase::Finalizing } else { OperationPhase::Stopping };
    let lease = manager.submit_resident("native").await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    lease.request_cancel();
    assert!(lease.settle().await.is_err());
    assert!(!lease.status().phase().is_terminal());
    assert!(lease.status().cleanup_pending());
    assert!(lease.status().cleanup_failure().is_some());
    assert_eq!(lease.status().phase(), expected);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 1);
    pending::parked(std::pin::pin!(lease.settle())).await;
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), if finalizing { 1 } else { 2 });
    assert_eq!(counts.finalizations.load(Ordering::SeqCst), if finalizing { 2 } else { 0 });
    assert_eq!(lease.status().phase(), expected);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    if finalizing {
        counts.finalization.add_permits(1);
    } else {
        counts.cleanup.add_permits(1);
    }
    assert!(lease.settle().await.is_err(), "the original logical failure remains observable");
    assert!(lease.status().phase().is_terminal());
    assert!(!lease.status().cleanup_pending());
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(lease.release().await.is_err());
    assert!(runtime.shutdown().await.is_err());
}

/* === Tests === */
