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
use std::pin::pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Semaphore, watch};
// Local Imports
use super::{Observing, RUNNING, STOPPED, STOPPING, pending, phase};
use harmos_runtime::sidecar::{Activity, Instance, Observer, ObserverContext, ObserverError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Resources {
    initialization: Semaphore,
    cleanup: Semaphore,
    finalization: Semaphore,
    stops: AtomicUsize,
    finalizations: AtomicUsize,
    attempts: AtomicUsize,
    acquired: AtomicUsize,
    ticks: AtomicUsize,
    dropped: Arc<AtomicUsize>,
    // Announces every initialization attempt and observed tick.
    progress: watch::Sender<()>,
}

struct Watch<const MODE: u8, const INTERVAL_MS: u64 = 5>(Arc<Resources>, bool);

/* === Enums === */

enum Fault {
    Panic,
    Refusal,
}

/* === Traits === */

/* === Implementations === */

impl Resources {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            initialization: Semaphore::new(0),
            cleanup: Semaphore::new(0),
            finalization: Semaphore::new(0),
            stops: AtomicUsize::new(0),
            finalizations: AtomicUsize::new(0),
            attempts: AtomicUsize::new(0),
            acquired: AtomicUsize::new(0),
            ticks: AtomicUsize::new(0),
            dropped: Arc::new(AtomicUsize::new(0)),
            progress: watch::Sender::new(()),
        })
    }
}

impl<const MODE: u8, const INTERVAL_MS: u64> Observer<()> for Watch<MODE, INTERVAL_MS> {
    type Error = std::io::Error;
    const ID: &'static str = "lifecycle";
    fn interval(_: &harmos_sdk::ExecutionContext) -> Result<Duration, Self::Error> {
        Ok(Duration::from_millis(INTERVAL_MS))
    }

    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, Self::Error> {
        let resolved = context.resource::<Resources>().unwrap();
        let resources = &resolved;
        let constructed: Result<Self, Self::Error> = async move { Ok(Self(resources.clone(), false)) }.await;
        let mut observer = constructed?;
        let started: Result<(), Self::Error> = async {
            let observer = &mut observer;
            let resources = &observer.0;
            observer.1 = true;
            resources.acquired.fetch_add(1, Ordering::SeqCst);
            let attempt = resources.attempts.fetch_add(1, Ordering::SeqCst);
            resources.progress.send_replace(());
            assert!(MODE != 4 || attempt != 0, "initialization panic");
            if MODE == 3 && attempt == 0 {
                return Err(std::io::Error::other("initialization refusal"));
            }
            resources.initialization.acquire().await.unwrap().forget();
            Ok(())
        }
        .await;
        started?;
        Ok(observer)
    }

    async fn observe(&mut self, _: &ObserverContext<'_, ()>) -> Result<(), Self::Error> {
        self.0.ticks.fetch_add(1, Ordering::SeqCst);
        self.0.progress.send_replace(());
        assert_ne!(MODE, 1, "callback panic");
        Err(std::io::Error::other("ordinary refusal"))
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        let attempt = self.0.stops.fetch_add(1, Ordering::SeqCst);
        assert!(MODE != 2 || attempt != 0, "cleanup panic");
        if MODE == 7 && attempt == 0 {
            return Err(std::io::Error::other("stop refusal"));
        }
        self.0.cleanup.acquire().await.unwrap().forget();
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        let attempt = self.0.finalizations.fetch_add(1, Ordering::SeqCst);
        assert!(MODE != 6 || attempt != 0, "finalization panic");
        if MODE == 5 && attempt == 0 {
            return Err(std::io::Error::other("finalization refusal"));
        }
        if MODE == 5 || MODE == 6 {
            self.0.finalization.acquire().await.unwrap().forget();
        }
        Ok(())
    }
}

impl<const MODE: u8, const INTERVAL_MS: u64> Drop for Watch<MODE, INTERVAL_MS> {
    fn drop(&mut self) {
        if self.1 {
            self.0.acquired.fetch_sub(1, Ordering::SeqCst);
        }
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn activity<S: Observer<()>>(resources: &Arc<Resources>) -> Activity {
    Instance::<()>::of::<S>().activate(super::observer_context(resources.clone()), ()).unwrap()
}

async fn count(resources: &Resources, value: fn(&Resources) -> &AtomicUsize, minimum: usize) {
    resources
        .progress
        .subscribe()
        .wait_for(|_| value(resources).load(Ordering::SeqCst) >= minimum)
        .await
        .expect("resources outlive the observer");
}

/* === Tests === */

#[tokio::test]
async fn readiness_follows_lifecycle() {
    let resources = Resources::new();
    let mut task = Observing::start(activity::<Watch<0>>(&resources));
    count(&resources, |held| &held.attempts, 1).await;
    assert!(!task.is_ready());
    resources.initialization.add_permits(1);
    task.ready().await;
    count(&resources, |held| &held.ticks, 2).await;
    assert!(task.is_observing());

    assert!(task.interrupt().await.is_none());
    let activity = task.activity();
    assert!(activity.is_initialized());
    resources.cleanup.add_permits(1);
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert!(!activity.is_initialized());
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelled_initialization_unready() {
    let resources = Resources::new();
    let mut task = Observing::start(activity::<Watch<0>>(&resources));
    count(&resources, |held| &held.attempts, 1).await;
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
    // Initialization owns its partial acquisition; interrupting it
    // drops that owner without running stop or cleanup.
    assert!(task.interrupt().await.is_none());
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
    let activity = task.activity();
    assert!(!activity.is_initialized());
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert_eq!(resources.stops.load(Ordering::SeqCst), 0);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 0);
    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn binding_panic_cause() {
    let resources = Resources::new();
    let refused = Instance::<()>::of::<Watch<0>>()
        .with_context::<()>(|_| panic!("binding refused"))
        .activate(super::observer_context(resources.clone()), ());
    assert!(matches!(refused, Err(ObserverError::BindingPanicked)));
    assert_eq!(resources.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn callback_panic_cleanup() {
    let resources = Resources::new();
    resources.initialization.add_permits(1);
    let mut task = Observing::start(activity::<Watch<1>>(&resources));
    // A panicked callback ends observation.
    let failure = task.ended().await;
    assert!(matches!(failure, Some(ObserverError::CallbackPanicked)));
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
    let activity = task.activity();
    assert!(activity.is_initialized());
    resources.cleanup.add_permits(1);
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stop_retries_faults() {
    let cases = [(activity::<Watch<2>> as fn(&Arc<Resources>) -> Activity, Fault::Panic), (activity::<Watch<7>>, Fault::Refusal)];
    for (activate, fault) in cases {
        let resources = Resources::new();
        resources.initialization.add_permits(1);
        let mut task = Observing::start(activate(&resources));
        task.ready().await;
        assert!(task.interrupt().await.is_none());
        let activity = task.activity();
        let failure = activity.stop().await.unwrap_err();
        match fault {
            Fault::Panic => assert!(matches!(&failure, ObserverError::StopPanicked), "{failure}"),
            Fault::Refusal => assert!(matches!(&failure, ObserverError::StopFailed(_)), "{failure}"),
        }
        // The activity retains the observer for a later attempt.
        assert!(activity.is_initialized());
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
        assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
        assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
        assert_eq!(resources.finalizations.load(Ordering::SeqCst), 0);
        resources.cleanup.add_permits(1);
        activity.stop().await.unwrap();
        activity.stop().await.unwrap();
        activity.cleanup().await.unwrap();
        activity.cleanup().await.unwrap();
        assert!(!activity.is_initialized());
        assert_eq!(resources.stops.load(Ordering::SeqCst), 2);
        assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
        assert_eq!(resources.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
        assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn finalization_retries_faults() {
    let cases = [(activity::<Watch<5>> as fn(&Arc<Resources>) -> Activity, Fault::Refusal), (activity::<Watch<6>>, Fault::Panic)];
    for (activate, fault) in cases {
        let resources = Resources::new();
        resources.initialization.add_permits(1);
        resources.cleanup.add_permits(1);
        let mut task = Observing::start(activate(&resources));
        task.ready().await;
        assert!(task.interrupt().await.is_none());
        let activity = task.activity();
        activity.stop().await.unwrap();
        let failure = activity.cleanup().await.unwrap_err();
        match fault {
            Fault::Panic => assert!(matches!(&failure, ObserverError::CleanupPanicked), "{failure}"),
            Fault::Refusal => assert!(matches!(&failure, ObserverError::CleanupFailed(_)), "{failure}"),
        }
        assert!(activity.is_initialized());
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
        assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
        {
            let mut retrying = pin!(activity.cleanup());
            pending::parked(retrying.as_mut()).await;
            assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
            pending::parked(retrying.as_mut()).await;
            assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
            resources.finalization.add_permits(1);
            retrying.await.unwrap();
        }
        activity.cleanup().await.unwrap();
        activity.stop().await.unwrap();
        assert!(!activity.is_initialized());
        assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
        assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn initialization_retries_faults() {
    let cases = [activity::<Watch<3>> as fn(&Arc<Resources>) -> Activity, activity::<Watch<4>>];
    for activate in cases {
        let resources = Resources::new();
        let mut task = Observing::start(activate(&resources));
        // A failed initialization drops its partial owner and retries on the next
        // cadence without stop or cleanup.
        count(&resources, |held| &held.attempts, 2).await;
        assert!(!task.is_ready());
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
        assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
        assert_eq!(resources.stops.load(Ordering::SeqCst), 0);
        assert_eq!(resources.finalizations.load(Ordering::SeqCst), 0);
        resources.initialization.add_permits(1);
        task.ready().await;
        resources.cleanup.add_permits(1);
        task.shutdown().await.unwrap();
        assert!(!task.activity().is_initialized());
        assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn slow_stop_retained() {
    let resources = Resources::new();
    resources.initialization.add_permits(1);
    let mut task = Observing::start(activity::<Watch<0>>(&resources));
    task.ready().await;
    assert!(task.interrupt().await.is_none());
    let activity = task.activity();
    {
        // No deadline abandons a slow stop: the observer stays owned.
        let mut stopping = pin!(activity.stop());
        pending::outlasts(stopping.as_mut(), Duration::from_secs(60)).await;
        assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
        resources.cleanup.add_permits(1);
        stopping.await.unwrap();
    }
    assert!(activity.is_initialized());
    activity.cleanup().await.unwrap();
    assert!(!activity.is_initialized());
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn dropped_lease_cleans() {
    // Dropping a bare activity skips stop and cleanup; the host owns
    // them, so a dropped lease still stops and cleans up the observer.
    let resources = Resources::new();
    let runtime = super::runtime_with(resources.clone()).await;
    let manager = &runtime.operations;
    manager.register_native_observer(Instance::<()>::of::<Watch<0>>(), |_| ()).unwrap();
    resources.initialization.add_permits(1);
    let lease = manager.submit_resident("lifecycle").await.unwrap();
    let key = super::key(manager, &lease);
    phase(manager, &key, RUNNING).await;
    drop(lease);
    phase(manager, &key, STOPPING).await;
    // Dropping a lease retains cleanup until it completes.
    let mut states = manager.watch_residents();
    let settling = pin!(states.wait_for(|states| { states.get(&key).is_some_and(|state| state.lifecycle.phase().is_terminal()) }));
    pending::outlasts(settling, Duration::from_secs(60)).await;
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
    resources.cleanup.add_permits(1);
    phase(manager, &key, STOPPED).await;
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn refused_retry_cancels() {
    let resources = Resources::new();
    let mut task = Observing::start(activity::<Watch<3>>(&resources));
    count(&resources, |held| &held.attempts, 1).await;
    // A refused initialization already released its partial owner.
    assert!(task.interrupt().await.is_none());
    let activity = task.activity();
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert!(!activity.is_initialized());
    let attempts = resources.attempts.load(Ordering::SeqCst);
    assert!(attempts >= 1);
    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), attempts);
    assert_eq!(resources.stops.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancellation_wakes_cadence() {
    let resources = Resources::new();
    resources.initialization.add_permits(1);
    resources.cleanup.add_permits(1);
    let mut task = Observing::start(activity::<Watch<0, 10_000>>(&resources));
    task.ready().await;
    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
    // Interruption wakes the cadence wait long before its next tick.
    task.shutdown().await.unwrap();
    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert!(!task.activity().is_initialized());
    assert!(!task.is_observing());
}
