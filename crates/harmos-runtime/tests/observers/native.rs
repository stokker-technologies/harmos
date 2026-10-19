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
use super::{RUNNING, STOPPING, key, pending, phase, runtime};
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

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
    type Resources = Arc<Counts>;
    type Error = Error;
    const ID: &'static str = "native";
    const INTERVAL: Duration = Duration::from_millis(5);
    async fn initialize(resources: &Arc<Counts>) -> Result<Self, Error> {
        resources.initialized.fetch_add(1, Ordering::SeqCst);
        Ok(Self(resources.clone()))
    }
    async fn observe(&mut self, _: &ObserverContext<'_, Arc<Counts>, ()>) -> Result<(), Error> {
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

fn watch<const PANIC: bool>() -> Instance<Arc<Counts>, ()> {
    Instance::<Arc<Counts>, ()>::of::<Watch<PANIC>>()
}

#[tokio::test]
async fn leases_share_existing() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    // Native observers run on the runtime executor, not a dedicated thread.
    let owner_thread = std::thread::current().id();
    manager
        .register_native_observer(watch::<false>(), move |_| {
            assert_eq!(std::thread::current().id(), owner_thread);
        })
        .unwrap();
    let counts = Counts::new();
    let resources = Arc::new(counts.clone());
    let first = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    let second = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    phase(manager, &key(manager, &first), RUNNING).await;
    counts.progress.subscribe().wait_for(|_| counts.ticks.load(Ordering::SeqCst) >= 2).await.unwrap();
    first.release().await.unwrap();
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
    counts.cleanup.add_permits(1);
    runtime.stop().await.unwrap();
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    let ticks = counts.ticks.load(Ordering::SeqCst);
    second.release().await.unwrap();
    assert_eq!(counts.ticks.load(Ordering::SeqCst), ticks);
}

#[tokio::test]
async fn native_panic_closes() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<true>(), |_| ()).unwrap();
    let counts = Counts::new();
    let resources = Arc::new(counts.clone());
    let lease = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    phase(manager, &key(manager, &lease), STOPPING).await;
    pending::parked(std::pin::pin!(manager.submit_with_resources_at("native", resources.clone()))).await;
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    counts.cleanup.add_permits(1);
    let failure = lease.release().await.unwrap_err();
    let harmos_runtime::Error::TaskFailed { source, .. } = failure else { panic!("managed failure") };
    let Some(harmos_runtime::Error::TaskFailed { source, .. }) = source.downcast_ref::<harmos_runtime::Error>() else {
        panic!("native failure remains a root error");
    };
    assert!(matches!(source.downcast_ref::<ObserverError>(), Some(ObserverError::CallbackPanicked)));
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(runtime.stop().await.is_err());
}

#[tokio::test]
async fn binding_owns_cleanup() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let counts = Counts::new();
    let resources = Arc::new(counts.clone());
    let bindings = Arc::new(AtomicUsize::new(0));
    let bound = bindings.clone();
    let owner_thread = std::thread::current().id();
    let candidate = manager
        .stage_native_observers(&resources, [watch::<false>()], move |_| {
            assert_eq!(std::thread::current().id(), owner_thread);
            bound.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    candidate.commit().unwrap();
    assert_eq!(bindings.load(Ordering::SeqCst), 0);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 0);
    let first = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    let second = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    phase(manager, &key(manager, &first), RUNNING).await;
    assert_eq!(first.id(), second.id());
    assert_eq!(bindings.load(Ordering::SeqCst), 1);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    counts.cleanup.add_permits(1);
    manager.retire_resources(&resources).await.unwrap();
    first.release().await.unwrap();
    second.release().await.unwrap();
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert!(manager.resident_definition("native").is_none());
    runtime.stop().await.unwrap();
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
    let runtime = runtime().await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<false>(), |_| ()).unwrap();
    let counts = Counts::new();
    counts.refuse_stop.store(true, Ordering::SeqCst);
    let resources = Arc::new(counts.clone());
    let lease = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    lease.cancel();
    assert!(lease.settle().await.is_err());
    drop(lease);
    drop(resources);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 1);

    counts.cleanup.add_permits(1);
    assert!(manager.stop_observers().await.is_err());
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stop_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.dropped.load(Ordering::SeqCst), 1);
    assert!(runtime.stop().await.is_err());
}

async fn cleanup_recovery(finalizing: bool) {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    manager.register_native_observer(watch::<false>(), |_| ()).unwrap();
    let counts = Counts::new();
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
    let resources = Arc::new(counts.clone());
    let lease = manager.submit_with_resources_at("native", resources.clone()).await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    lease.cancel();
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
    assert!(runtime.stop().await.is_err());
}

/* === Tests === */
