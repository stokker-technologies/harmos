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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::{
    Lanes, assembly, service_status,
    support::{Bank, Noted},
};
use super::{Ledger, Runtime};
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Error, Refusal, services::ServiceExit, services::ServiceStatus, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Sequence {
    events: Arc<Mutex<Vec<&'static str>>>,
    started: Arc<Notify>,
}

struct OrderedGuard {
    running: bool,
    sequence: Arc<Sequence>,
}

struct Dropped(Arc<AtomicBool>);

#[derive(Default)]
struct Cleanup {
    attempts: AtomicUsize,
    finalized: AtomicUsize,
    stop_future_dropped: AtomicBool,
    dropped: AtomicBool,
    failed: AtomicBool,
    running: Notify,
    finalizing: Notify,
    release: Notify,
}

struct Owned(Arc<Cleanup>);
struct PhysicalStop(Arc<Cleanup>);
struct StopWait(Arc<Cleanup>);

/* === Enums === */

#[harmos_runtime::operation(resources = [Sequence], response = (),
        id = "shutdown.ordered",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct OrderedContractDeclaration {
    running: bool,
}

#[harmos_runtime::catalog]
enum Operations {
    OrderedContractDeclaration(OrderedContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for OrderedContractDeclaration {
    type Error = std::convert::Infallible;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let sequence = ctx.resource::<Sequence>();
        let _guard = OrderedGuard {
            running: self.running,
            sequence: sequence.clone(),
        };
        if self.running {
            sequence.events.lock().unwrap().push("job-start");
            sequence.started.notify_one();
            std::future::pending::<()>().await;
        } else {
            sequence.events.lock().unwrap().push("queued");
        }
        Ok(().into())
    }
}
impl Drop for OrderedGuard {
    fn drop(&mut self) {
        if self.running {
            self.sequence.events.lock().unwrap().push("job-end");
        }
    }
}

impl harmos_runtime::services::Resident<super::Bank> for Owned {
    type Error = std::io::Error;

    async fn run(&mut self, mut context: harmos_runtime::services::ServiceContext<super::Bank>) -> Result<(), Self::Error> {
        context.ready();
        self.0.running.notify_one();
        context.stopping().await;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.attempts.fetch_add(1, Ordering::SeqCst);
        if self.0.failed.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("physical cleanup failed"));
        }
        self.0.finalizing.notify_one();
        self.0.release.notified().await;
        Ok(())
    }
}

impl harmos_runtime::services::Resident<Bank> for PhysicalStop {
    type Error = std::io::Error;

    async fn run(&mut self, mut context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), Self::Error> {
        context.ready();
        self.0.running.notify_one();
        context.stopping().await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        let _original = StopWait(self.0.clone());
        self.0.attempts.fetch_add(1, Ordering::SeqCst);
        self.0.finalizing.notify_one();
        self.0.release.notified().await;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for PhysicalStop {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::SeqCst);
    }
}
impl Drop for StopWait {
    fn drop(&mut self) {
        self.0.stop_future_dropped.store(true, Ordering::SeqCst);
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::SeqCst);
    }
}

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn service_stop_observer() {
    struct Blocked(Arc<Cleanup>);
    impl harmos_runtime::observers::ObserverBackend for Blocked {
        fn definition(&self) -> harmos_runtime::Result<harmos_runtime::observers::ObserverDefinition> {
            Ok(harmos_runtime::observers::ObserverDefinition {
                id: "blocked-observer".into(),
                interval: Duration::from_secs(10),
                streams: vec![],
                demand_keys: vec![],
            })
        }
        async fn run(
            &self, _: Arc<harmos_sdk::ExecutionContext>, mut activation: harmos_runtime::observers::ObserverActivation,
        ) -> harmos_runtime::Result<()> {
            activation.ready();
            let gates = &self.0;
            gates.running.notify_one();
            activation.stopping().await;
            gates.finalizing.notify_one();
            gates.release.notified().await;
            Ok(())
        }
    }
    let service = Arc::new(Cleanup::default());
    let observer = Arc::new(Cleanup::default());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("independent", PhysicalStop(service.clone()));
    })
    .await
    .unwrap();
    service.running.notified().await;
    runtime.operations.register_observer(Blocked(observer.clone())).unwrap();
    let _lease = runtime.operations.submit_resident("blocked-observer").await.unwrap();
    observer.running.notified().await;
    {
        let stop = runtime.shutdown();
        tokio::pin!(stop);
        (async {
            tokio::select! {
                result = &mut stop => panic!("cleanup remains held: {result:?}"),
                _ = async {
                    observer.finalizing.notified().await;
                    service.finalizing.notified().await;
                } => {}
            }
        })
        .await;
    }
    assert_eq!(service.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(service.finalized.load(Ordering::SeqCst), 0);
    observer.release.notify_one();
    service.release.notify_one();
    runtime.shutdown().await.unwrap();
    assert_eq!(service.finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interruption_retains_stop() {
    let cleanup = Arc::new(Cleanup::default());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("physical-stop", PhysicalStop(cleanup.clone()));
    })
    .await
    .unwrap();
    cleanup.running.notified().await;
    let mut participant = runtime.stop_participant();
    {
        let stopping = runtime.shutdown();
        tokio::pin!(stopping);
        (async {
            tokio::select! {
                result = &mut stopping => panic!("physical stop is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await;
    }
    assert!(!cleanup.stop_future_dropped.load(Ordering::SeqCst));
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 0);
    let statuses = runtime.operations.residents();
    let state = statuses.iter().find(|(key, _)| key.definition == "physical-stop").unwrap().1;
    assert_eq!(state.lifecycle.phase(), harmos_runtime::operations::OperationPhase::Stopping);
    assert!(state.lifecycle.settled_at().is_none());

    cleanup.release.notify_one();
    (participant.stopping()).await;
    participant.finish();
    (runtime.shutdown()).await.unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 1);
    assert!(cleanup.stop_future_dropped.load(Ordering::SeqCst));
    assert!(cleanup.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cleanup_releases_resident() {
    let cleanup = Arc::new(Cleanup::default());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("owned", Owned(cleanup.clone()));
    })
    .await
    .unwrap();
    cleanup.running.notified().await;
    {
        let stopping = runtime.shutdown();
        tokio::pin!(stopping);
        (async {
            tokio::select! {
                result = &mut stopping => panic!("cleanup is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await;
        assert!(!cleanup.dropped.load(Ordering::SeqCst));
        cleanup.release.notify_one();
        (&mut stopping).await.unwrap();
    }
    assert!(cleanup.dropped.load(Ordering::SeqCst));
    runtime.shutdown().await.unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cleanup_retry_survives() {
    let cleanup = Arc::new(Cleanup::default());
    cleanup.failed.store(true, Ordering::SeqCst);
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("owned", Owned(Arc::clone(&cleanup)));
    })
    .await
    .unwrap();
    cleanup.running.notified().await;
    let mut participant = runtime.stop_participant();
    let failure = runtime.shutdown().await.unwrap_err();
    let failure = std::iter::successors(Some(&failure as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert!(failure.contains("physical cleanup failed"), "{failure}");
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);

    cleanup.failed.store(false, Ordering::SeqCst);
    {
        let waiting = runtime.shutdown();
        tokio::pin!(waiting);
        (async {
            tokio::select! {
                result = &mut waiting => panic!("physical cleanup is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await;
    }
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    cleanup.release.notify_one();
    (participant.stopping()).await;
    participant.finish();
    (runtime.shutdown()).await.unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 2);
    assert!(cleanup.dropped.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn uncooperative_resident_aborted() {
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service_with(
            "stalled",
            harmos_runtime::ServicePolicy {
                stop_timeout: Duration::from_millis(10),
                ..harmos_runtime::services::POLICY
            },
            {
                let entered = entered.clone();
                let dropped = dropped.clone();
                move |_context| {
                    let entered = entered.clone();
                    let dropped = dropped.clone();
                    async move {
                        let _guard = Dropped(dropped);
                        entered.notify_one();
                        std::future::pending::<Result<(), std::io::Error>>().await
                    }
                }
            },
        );
    })
    .await
    .unwrap();
    entered.notified().await;
    let statuses = runtime.operations.watch_residents();

    assert!(matches!((runtime.shutdown()).await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));

    assert!(dropped.load(Ordering::SeqCst), "shutdown joined cancellation before returning");
    assert!(matches!(
        statuses.borrow().get(&harmos_runtime::operations::resident::Key::root("stalled")).and_then(|item| item.health.as_ref()),
        Some(ServiceStatus::Inactive { cause: ServiceExit::StopTimedOut, .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn cleanup_finish_within() {
    let entered = Arc::new(Notify::new());
    let cleaned = Arc::new(AtomicBool::new(false));
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service_with(
            "cleanup",
            harmos_runtime::ServicePolicy {
                stop_timeout: Duration::from_millis(50),
                ..harmos_runtime::services::POLICY
            },
            {
                let entered = entered.clone();
                let cleaned = cleaned.clone();
                move |mut context: harmos_runtime::services::ServiceContext<super::Bank>| {
                    let entered = entered.clone();
                    let cleaned = cleaned.clone();
                    async move {
                        entered.notify_one();
                        context.stopping().await;
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        cleaned.store(true, Ordering::SeqCst);
                        Ok::<(), std::io::Error>(())
                    }
                }
            },
        );
    })
    .await
    .unwrap();
    entered.notified().await;
    let statuses = runtime.operations.watch_residents();

    runtime.shutdown().await.unwrap();

    assert!(cleaned.load(Ordering::SeqCst));
    assert!(matches!(
        statuses.borrow().get(&harmos_runtime::operations::resident::Key::root("cleanup")).and_then(|item| item.health.as_ref()),
        Some(ServiceStatus::Stopped)
    ));
}

#[tokio::test(start_paused = true)]
async fn unregister_retains_ownership() {
    let entered = Arc::new(Notify::new());
    let stopping = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            assembly(setup);
        })
        .await
        .unwrap(),
    );
    runtime
        .operations
        .register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "tenant".into(),
                policy: harmos_runtime::ServicePolicy {
                    stop_timeout: Duration::from_millis(10),
                    ..harmos_runtime::services::POLICY
                },
            },
            {
                let entered = entered.clone();
                let stopping = stopping.clone();
                let dropped = dropped.clone();
                move |_: Arc<harmos_sdk::ExecutionContext>| {
                    let entered = entered.clone();
                    let stopping = stopping.clone();
                    let dropped = dropped.clone();
                    move |mut context: harmos_runtime::services::ServiceContext<super::Bank>| {
                        let entered = entered.clone();
                        let stopping = stopping.clone();
                        let dropped = dropped.clone();
                        async move {
                            let _guard = Dropped(dropped);
                            entered.notify_one();
                            context.stopping().await;
                            stopping.notify_one();
                            std::future::pending::<Result<(), std::io::Error>>().await
                        }
                    }
                }
            },
        )
        .unwrap();
    let _lease = runtime.operations.submit_resident("tenant").await.unwrap();
    entered.notified().await;
    let waiter = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime.operations.unregister_resident("tenant").await.unwrap_err();
        }
    });
    stopping.notified().await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());

    (runtime.operations.unregister_resident("tenant")).await.unwrap_err();

    assert!(dropped.load(Ordering::SeqCst));
    assert!(!runtime.operations.watch_residents().borrow().contains_key(&harmos_runtime::operations::resident::Key::root("tenant")));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn domains_outlives_clones() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let service_events = Arc::clone(&events);
    let started = Arc::new(Notify::new());
    let sequence = Arc::new(Sequence {
        events: events.clone(),
        started: started.clone(),
    });
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.resource::<Sequence>(sequence);
        setup.operations::<Operations>(Lanes::Serial);
        setup.settings().service("resident", move |mut context: harmos_runtime::services::ServiceContext<Bank>| {
            let events = Arc::clone(&service_events);
            async move {
                context.ready();
                assert!(context.rest(Duration::from_secs(60)).await);
                events.lock().expect("event lock is healthy").push("service");
                Ok::<(), std::io::Error>(())
            }
        });
    })
    .await
    .expect("the work declarations are valid");
    service_status(runtime.operations.watch_residents(), "resident", |status| matches!(status, ServiceStatus::Running { .. })).await;

    let running = runtime.operations.submit::<OrderedContractDeclaration>(OrderedContractDeclaration { running: true }).unwrap();
    started.notified().await;
    let queued = runtime.operations.submit::<OrderedContractDeclaration>(OrderedContractDeclaration { running: false }).unwrap();

    let mut participant = runtime.stop_participant();
    let participant_events = Arc::clone(&events);
    let host = tokio::spawn(async move {
        participant.stopping().await;
        participant_events.lock().expect("event lock is healthy").push("participant");
        participant.finish();
    });
    runtime.shutdown().await.unwrap();
    host.await.expect("the host participant does not panic");
    assert!(matches!(running.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    let trace = events.lock().expect("event lock is healthy").clone();
    assert!(
        matches!(trace.as_slice(), ["job-start", "job-end", "service", "participant"] | ["job-start", "service", "job-end", "participant"]),
        "unexpected shutdown trace: {trace:?}"
    );

    // A retained handle does not hold shutdown open: the stop seals the journal
    // rather than waiting for the last clone to go, so a host that keeps one by
    // design still stops, and stops promptly.
    let runtime = Arc::new(
        Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            assembly(setup);
        })
        .await
        .expect("the boot is valid"),
    );
    let retained = runtime.handle();
    let stopping = tokio::spawn({
        let runtime = Arc::clone(&runtime);

        async move { runtime.shutdown().await.unwrap() }
    });

    assert_eq!((stopping).await.expect("runtime stop does not panic"), Position::ORIGIN);
    assert!(
        matches!(retained.records.publish(Noted("after".to_owned())).await, Err(harmos_runtime::Error::Unavailable)),
        "a sealed journal refuses a late write rather than leaving it hanging"
    );
    assert_eq!(runtime.shutdown().await.unwrap(), Position::ORIGIN, "a repeat stop reports the frontier the first one observed");
}

/* === Tests === */
