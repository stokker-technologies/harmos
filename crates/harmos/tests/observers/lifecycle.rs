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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Semaphore, watch};
// Local Imports
use harmos::sidecar::{Instance, Observer, ObserverContext, ObserverExit, ObserverStatus};

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
}

struct Watch<const MODE: u8, const INTERVAL_MS: u64 = 5>(Arc<Resources>, bool);

/* === Enums === */

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
        })
    }
}

impl<const MODE: u8, const INTERVAL_MS: u64> Observer<()> for Watch<MODE, INTERVAL_MS> {
    type Resources = Arc<Resources>;
    type Error = std::io::Error;
    const ID: &'static str = "lifecycle";
    const INTERVAL: Duration = Duration::from_millis(INTERVAL_MS);

    fn new(resources: &Arc<Resources>) -> Result<Self, Self::Error> {
        Ok(Self(resources.clone(), false))
    }

    async fn initialize(&mut self) -> Result<(), Self::Error> {
        let resources = &self.0;
        self.1 = true;
        resources.acquired.fetch_add(1, Ordering::SeqCst);
        let attempt = resources.attempts.fetch_add(1, Ordering::SeqCst);
        assert!(MODE != 4 || attempt != 0, "initialization panic");
        if MODE == 3 && attempt == 0 {
            return Err(std::io::Error::other("initialization refusal"));
        }
        resources.initialization.acquire().await.unwrap().forget();
        Ok(())
    }

    async fn observe(
        &mut self,
        _: &ObserverContext<'_, Arc<Resources>, ()>,
    ) -> Result<(), Self::Error> {
        self.0.ticks.fetch_add(1, Ordering::SeqCst);
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

    async fn finalize(&mut self) -> Result<(), Self::Error> {
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
async fn state(states: &mut watch::Receiver<ObserverStatus>, expected: ObserverStatus) {
    tokio::time::timeout(
        Duration::from_secs(3),
        states.wait_for(|value| *value == expected),
    )
    .await
    .expect("lifecycle advances")
    .expect("runner retains terminal status");
}

async fn count(value: &AtomicUsize, minimum: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while value.load(Ordering::SeqCst) < minimum {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("observer advances");
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("readiness_follows_lifecycle", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<0>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    let mut states = task.subscribe();
                    count(&resources.attempts, 1).await;
                    assert_eq!(task.status(), ObserverStatus::Starting);
                    resources.initialization.add_permits(1);
                    state(&mut states, ObserverStatus::Running).await;
                    count(&resources.ticks, 2).await;
                    assert_eq!(task.status(), ObserverStatus::Running);

                    task.cancel();
                    assert_eq!(task.status(), ObserverStatus::Stopping);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(20), task.join())
                            .await
                            .is_err()
                    );
                    resources.cleanup.add_permits(1);
                    state(
                        &mut states,
                        ObserverStatus::Stopped(ObserverExit::Cancelled),
                    )
                    .await;
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                    task.join().await.unwrap();
                    task.stop().await.unwrap();
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::Cancelled)
                    );
                });
        });
    }
    {
        scenario_runtime::check("cancelled_initialization_unready", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<0>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    count(&resources.attempts, 1).await;
                    task.cancel();
                    assert_eq!(task.status(), ObserverStatus::Stopping);
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
                    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(20), task.join())
                            .await
                            .is_err(),
                        "cancelled initialization retains its partial owner for cleanup"
                    );
                    resources.cleanup.add_permits(1);
                    task.join().await.unwrap();
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::Cancelled)
                    );
                    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
                });
        });
    }
    {
        scenario_runtime::check("binding_panic_cause", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<0>>()
                        .with_context::<()>(|_| panic!("binding refused"))
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    task.join().await.unwrap();
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::BindingPanicked)
                    );
                    assert_eq!(resources.attempts.load(Ordering::SeqCst), 0);
                });
        });
    }
    {
        scenario_runtime::check("callback_panic_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    resources.initialization.add_permits(1);
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<1>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    state(&mut task.subscribe(), ObserverStatus::Stopping).await;
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
                    resources.cleanup.add_permits(1);
                    task.join().await.unwrap();
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::CallbackPanicked)
                    );
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cleanup_panic_recovery", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    stop_recovery::<2>(ObserverExit::StopPanicked).await;
                });
        });
    }
    {
        scenario_runtime::check("cleanup_refusal_recovery", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    stop_recovery::<7>(ObserverExit::StopFailed).await;
                });
        });
    }
    {
        scenario_runtime::check("finalization_refusal_retries", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    finalization_recovery::<5>(ObserverExit::FinalizeFailed).await;
                });
        });
    }
    {
        scenario_runtime::check("finalization_panic_retries", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    finalization_recovery::<6>(ObserverExit::FinalizePanicked).await;
                });
        });
    }
    {
        scenario_runtime::check("slow_cleanup_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    resources.initialization.add_permits(1);
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<0>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    state(&mut task.subscribe(), ObserverStatus::Running).await;
                    task.cancel();
                    assert!(
                        tokio::time::timeout(Duration::from_millis(1200), task.join())
                            .await
                            .is_err(),
                        "a cleanup diagnostic must not settle the original owner"
                    );
                    assert_eq!(task.status(), ObserverStatus::Stopping);
                    assert!(!task.is_finished());
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);

                    resources.cleanup.add_permits(1);
                    task.join().await.unwrap();
                    assert!(task.is_finished());
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::Cancelled)
                    );
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("dropped_task_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    resources.initialization.add_permits(1);
                    let task = Instance::<Arc<Resources>, ()>::of::<Watch<0>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    let mut states = task.subscribe();
                    state(&mut states, ObserverStatus::Running).await;
                    drop(task);
                    assert_eq!(*states.borrow(), ObserverStatus::Stopping);
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(1200),
                            states.wait_for(|value| matches!(value, ObserverStatus::Stopped(_))),
                        )
                        .await
                        .is_err(),
                        "dropping a handle must retain cleanup beyond its diagnostic threshold"
                    );
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
                    resources.cleanup.add_permits(1);
                    state(
                        &mut states,
                        ObserverStatus::Stopped(ObserverExit::Cancelled),
                    )
                    .await;
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                });
        });
    }
    {
        scenario_runtime::check("cancelled_cleanup_skips", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<3>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    count(&resources.attempts, 1).await;
                    task.cancel();
                    assert!(
                        tokio::time::timeout(Duration::from_millis(20), task.join())
                            .await
                            .is_err()
                    );
                    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
                    resources.cleanup.add_permits(1);
                    task.join().await.unwrap();
                    assert_eq!(resources.attempts.load(Ordering::SeqCst), 1);
                    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
                    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::Cancelled)
                    );
                });
        });
    }
    {
        scenario_runtime::check("initialization_refusal_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    initialization_retry::<3>().await;
                });
        });
    }
    {
        scenario_runtime::check("initialization_panic_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    initialization_retry::<4>().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_wakes_cadence", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let resources = Resources::new();
                    resources.initialization.add_permits(1);
                    resources.cleanup.add_permits(1);
                    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<0, 10_000>>()
                        .start(Arc::new(resources.clone()), ())
                        .unwrap();
                    state(&mut task.subscribe(), ObserverStatus::Running).await;
                    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
                    tokio::time::timeout(Duration::from_secs(1), task.stop())
                        .await
                        .expect("cancellation must wake the cadence wait")
                        .unwrap();
                    assert_eq!(resources.ticks.load(Ordering::SeqCst), 0);
                    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
                    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
                    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
                    assert_eq!(
                        task.status(),
                        ObserverStatus::Stopped(ObserverExit::Cancelled)
                    );
                    assert!(task.is_finished());
                });
        });
    }
}

async fn stop_recovery<const MODE: u8>(exit: ObserverExit) {
    let resources = Resources::new();
    resources.initialization.add_permits(1);
    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<MODE>>()
        .start(Arc::new(resources.clone()), ())
        .unwrap();
    state(&mut task.subscribe(), ObserverStatus::Running).await;
    assert!(task.stop().await.is_err());
    assert_eq!(task.status(), ObserverStatus::Retained(exit));
    assert!(!task.is_finished());
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
    assert!(task.join().await.is_err());
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 0);
    resources.cleanup.add_permits(1);
    task.recover().await.unwrap();
    task.recover().await.unwrap();
    assert_eq!(task.status(), ObserverStatus::Stopped(exit));
    assert_eq!(resources.stops.load(Ordering::SeqCst), 2);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 1);
    assert_eq!(resources.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
}

async fn finalization_recovery<const MODE: u8>(exit: ObserverExit) {
    let resources = Resources::new();
    resources.initialization.add_permits(1);
    resources.cleanup.add_permits(1);
    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<MODE>>()
        .start(Arc::new(resources.clone()), ())
        .unwrap();
    state(&mut task.subscribe(), ObserverStatus::Running).await;
    assert!(task.stop().await.is_err());
    assert_eq!(task.status(), ObserverStatus::Retained(exit));
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), task.recover())
            .await
            .is_err()
    );
    assert_eq!(task.status(), ObserverStatus::Finalizing);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), task.join())
            .await
            .is_err()
    );
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
    resources.finalization.add_permits(1);
    task.join().await.unwrap();
    task.recover().await.unwrap();
    assert_eq!(task.status(), ObserverStatus::Stopped(exit));
    assert_eq!(resources.stops.load(Ordering::SeqCst), 1);
    assert_eq!(resources.finalizations.load(Ordering::SeqCst), 2);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
}

async fn initialization_retry<const MODE: u8>() {
    let resources = Resources::new();
    let mut task = Instance::<Arc<Resources>, ()>::of::<Watch<MODE>>()
        .start(Arc::new(resources.clone()), ())
        .unwrap();
    count(&resources.attempts, 1).await;
    assert_eq!(task.status(), ObserverStatus::Starting);
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 0);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), count(&resources.attempts, 2))
            .await
            .is_err(),
        "the next initialization cannot overlap partial-owner cleanup"
    );
    resources.cleanup.add_permits(1);
    count(&resources.attempts, 2).await;
    assert_eq!(resources.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 1);
    assert_eq!(task.status(), ObserverStatus::Starting);
    resources.initialization.add_permits(1);
    state(&mut task.subscribe(), ObserverStatus::Running).await;
    resources.cleanup.add_permits(1);
    task.stop().await.unwrap();
    assert_eq!(
        task.status(),
        ObserverStatus::Stopped(ObserverExit::Cancelled)
    );
}

/* === Tests === */
