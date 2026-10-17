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
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::{
    Error, Refusal, services::Fault, services::ServiceExit, services::ServiceStatus,
    state::Position,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Sequence {
    events: Arc<Mutex<Vec<&'static str>>>,
    started: Arc<Notify>,
}

struct Ordered {
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
#[harmos::operation(
        request = bool,
        response = (),
        id = "shutdown.ordered",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct OrderedContract;

#[harmos::catalog]
enum Operations {
    OrderedContract(OrderedContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Ordered {
    type Declaration = OrderedContract;
    type Resources = Sequence;
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        sequence: Arc<Sequence>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            running: false,
            sequence,
        })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        running: &mut bool,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.running = *running;
            if self.running {
                self.sequence.events.lock().unwrap().push("job-start");
                self.sequence.started.notify_one();
                std::future::pending::<()>().await;
            } else {
                self.sequence.events.lock().unwrap().push("queued");
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.running {
            self.sequence.events.lock().unwrap().push("job-end");
        }
        Ok(())
    }
}

impl harmos::services::Resident<super::Bank> for Owned {
    type Error = std::io::Error;

    async fn run(
        &mut self,
        mut context: harmos::services::Service<super::Bank>,
    ) -> Result<(), Fault<Self::Error>> {
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

impl harmos::services::Resident<Bank> for PhysicalStop {
    type Error = std::io::Error;

    async fn run(
        &mut self,
        mut context: harmos::services::Service<Bank>,
    ) -> Result<(), Fault<Self::Error>> {
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
    struct Blocked;
    impl harmos::observers::ObserverBackend for Blocked {
        type Resources = Cleanup;
        fn definition(&self) -> harmos::Result<harmos::observers::ObserverDefinition> {
            Ok(harmos::observers::ObserverDefinition {
                id: "blocked-observer".into(),
                interval: Duration::from_secs(10),
                streams: vec![],
                demand_keys: vec![],
            })
        }
        async fn run(
            &self,
            gates: Arc<Cleanup>,
            mut activation: harmos::observers::ObserverActivation,
        ) -> harmos::Result<()> {
            activation.ready();
            gates.running.notify_one();
            activation.stopping().await;
            gates.finalizing.notify_one();
            gates.release.notified().await;
            Ok(())
        }
    }
    let service = Arc::new(Cleanup::default());
    let observer = Arc::new(Cleanup::default());
    let runtime = assembly()
        .service("independent", PhysicalStop(service.clone()))
        .runtime()
        .await
        .unwrap();
    service.running.notified().await;
    runtime.operations.register_observer(Blocked).unwrap();
    let _lease = runtime
        .operations
        .submit_with_resources_at("blocked-observer", observer.clone())
        .await
        .unwrap();
    observer.running.notified().await;
    {
        let stop = runtime.stop();
        tokio::pin!(stop);
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = &mut stop => panic!("cleanup remains held: {result:?}"),
                _ = async {
                    observer.finalizing.notified().await;
                    service.finalizing.notified().await;
                } => {}
            }
        })
        .await
        .expect("independent physical stop is not held by observer cleanup");
    }
    assert_eq!(service.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(service.finalized.load(Ordering::SeqCst), 0);
    observer.release.notify_one();
    service.release.notify_one();
    runtime.stop().await.unwrap();
    assert_eq!(service.finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interruption_retains_stop() {
    let cleanup = Arc::new(Cleanup::default());
    let runtime = assembly()
        .service("physical-stop", PhysicalStop(cleanup.clone()))
        .runtime()
        .await
        .unwrap();
    cleanup.running.notified().await;
    let mut participant = runtime.stop_participant();
    {
        let stopping = runtime.stop();
        tokio::pin!(stopping);
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = &mut stopping => panic!("physical stop is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await
        .unwrap();
    }
    assert!(!cleanup.stop_future_dropped.load(Ordering::SeqCst));
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 0);
    let statuses = runtime.operations.residents();
    let state = statuses
        .iter()
        .find(|(key, _)| key.definition == "physical-stop")
        .unwrap()
        .1;
    assert_eq!(
        state.lifecycle.phase(),
        harmos::operations::OperationPhase::Stopping
    );
    assert!(state.lifecycle.settled_at().is_none());

    cleanup.release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), participant.stopping())
        .await
        .expect("the original driver advances without another shutdown waiter");
    participant.finish();
    tokio::time::timeout(Duration::from_secs(2), runtime.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(cleanup.finalized.load(Ordering::SeqCst), 1);
    assert!(cleanup.stop_future_dropped.load(Ordering::SeqCst));
    assert!(cleanup.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cleanup_releases_resident() {
    let cleanup = Arc::new(Cleanup::default());
    let runtime = assembly()
        .service("owned", Owned(cleanup.clone()))
        .runtime()
        .await
        .unwrap();
    cleanup.running.notified().await;
    {
        let stopping = runtime.stop();
        tokio::pin!(stopping);
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = &mut stopping => panic!("cleanup is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await
        .unwrap();
        assert!(!cleanup.dropped.load(Ordering::SeqCst));
        cleanup.release.notify_one();
        tokio::time::timeout(Duration::from_secs(2), &mut stopping)
            .await
            .unwrap()
            .unwrap();
    }
    assert!(cleanup.dropped.load(Ordering::SeqCst));
    runtime.stop().await.unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cleanup_retry_survives() {
    let cleanup = Arc::new(Cleanup::default());
    cleanup.failed.store(true, Ordering::SeqCst);
    let runtime = assembly()
        .service("owned", Owned(Arc::clone(&cleanup)))
        .runtime()
        .await
        .unwrap();
    cleanup.running.notified().await;
    let mut participant = runtime.stop_participant();
    let failure = runtime.stop().await.unwrap_err();
    let failure = std::iter::successors(
        Some(&failure as &(dyn std::error::Error + 'static)),
        |error| error.source(),
    )
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(": ");
    assert!(failure.contains("physical cleanup failed"), "{failure}");
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 1);

    cleanup.failed.store(false, Ordering::SeqCst);
    {
        let waiting = runtime.stop();
        tokio::pin!(waiting);
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = &mut waiting => panic!("physical cleanup is gated: {result:?}"),
                _ = cleanup.finalizing.notified() => {},
            }
        })
        .await
        .unwrap();
    }
    assert!(!cleanup.dropped.load(Ordering::SeqCst));
    cleanup.release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), participant.stopping())
        .await
        .expect("the original driver advances without another shutdown waiter");
    participant.finish();
    tokio::time::timeout(Duration::from_secs(2), runtime.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cleanup.attempts.load(Ordering::SeqCst), 2);
    assert!(cleanup.dropped.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn uncooperative_resident_aborted() {
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let runtime = assembly()
        .service_with(
            "stalled",
            harmos::operations::OperationPolicy {
                timeout: Duration::from_millis(10),
                ..harmos::services::POLICY
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
                        std::future::pending::<Result<(), Fault<&str>>>().await
                    }
                }
            },
        )
        .runtime()
        .await
        .unwrap();
    entered.notified().await;
    let statuses = runtime.operations.watch_residents();

    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), runtime.stop())
            .await
            .unwrap(),
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));

    assert!(
        dropped.load(Ordering::SeqCst),
        "shutdown joined cancellation before returning"
    );
    assert!(matches!(
        statuses
            .borrow()
            .get(&harmos::operations::resident::Key::root("stalled"))
            .and_then(|item| item.health.as_ref()),
        Some(ServiceStatus::Inactive {
            cause: ServiceExit::StopTimedOut,
            ..
        })
    ));
}

#[tokio::test(start_paused = true)]
async fn cleanup_finish_within() {
    let entered = Arc::new(Notify::new());
    let cleaned = Arc::new(AtomicBool::new(false));
    let runtime = assembly()
        .service_with(
            "cleanup",
            harmos::operations::OperationPolicy {
                timeout: Duration::from_millis(50),
                ..harmos::services::POLICY
            },
            {
                let entered = entered.clone();
                let cleaned = cleaned.clone();
                move |mut context: harmos::services::Service<super::Bank>| {
                    let entered = entered.clone();
                    let cleaned = cleaned.clone();
                    async move {
                        entered.notify_one();
                        context.stopping().await;
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        cleaned.store(true, Ordering::SeqCst);
                        Ok::<(), Fault<&str>>(())
                    }
                }
            },
        )
        .runtime()
        .await
        .unwrap();
    entered.notified().await;
    let statuses = runtime.operations.watch_residents();

    runtime.stop().await.unwrap();

    assert!(cleaned.load(Ordering::SeqCst));
    assert!(matches!(
        statuses
            .borrow()
            .get(&harmos::operations::resident::Key::root("cleanup"))
            .and_then(|item| item.health.as_ref()),
        Some(ServiceStatus::Stopped)
    ));
}

#[tokio::test(start_paused = true)]
async fn unregister_retains_ownership() {
    let entered = Arc::new(Notify::new());
    let stopping = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(assembly().runtime().await.unwrap());
    runtime
        .operations
        .register_service(
            harmos::services::ServiceDefinition {
                id: "tenant".into(),
                policy: harmos::operations::OperationPolicy {
                    timeout: Duration::from_millis(10),
                    ..harmos::services::POLICY
                },
            },
            {
                let entered = entered.clone();
                let stopping = stopping.clone();
                let dropped = dropped.clone();
                move |_: Arc<()>| {
                    let entered = entered.clone();
                    let stopping = stopping.clone();
                    let dropped = dropped.clone();
                    move |mut context: harmos::services::Service<super::Bank>| {
                        let entered = entered.clone();
                        let stopping = stopping.clone();
                        let dropped = dropped.clone();
                        async move {
                            let _guard = Dropped(dropped);
                            entered.notify_one();
                            context.stopping().await;
                            stopping.notify_one();
                            std::future::pending::<Result<(), Fault<&str>>>().await
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
            runtime
                .operations
                .unregister_resident("tenant")
                .await
                .unwrap_err();
        }
    });
    stopping.notified().await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());

    tokio::time::timeout(
        Duration::from_secs(1),
        runtime.operations.unregister_resident("tenant"),
    )
    .await
    .unwrap()
    .unwrap_err();

    assert!(dropped.load(Ordering::SeqCst));
    assert!(
        !runtime
            .operations
            .watch_residents()
            .borrow()
            .contains_key(&harmos::operations::resident::Key::root("tenant"))
    );
    runtime.stop().await.unwrap();
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
    let runtime = assembly()
        .operation::<Ordered>("app", Lanes::Serial, sequence)
        .service(
            "resident",
            move |mut context: harmos::services::Service<Bank>| {
                let events = Arc::clone(&service_events);
                async move {
                    context.ready();
                    assert!(context.rest(Duration::from_secs(60)).await);
                    events
                        .lock()
                        .expect("event lock is healthy")
                        .push("service");
                    Ok::<(), Fault<&'static str>>(())
                }
            },
        )
        .runtime()
        .await
        .expect("the work declarations are valid");
    service_status(runtime.operations.watch_residents(), "resident", |status| {
        matches!(status, ServiceStatus::Running { .. })
    })
    .await;

    let running = runtime
        .operations
        .submit_as::<OrderedContract>(true)
        .unwrap();
    started.notified().await;
    let queued = runtime
        .operations
        .submit_as::<OrderedContract>(false)
        .unwrap();

    let mut participant = runtime.stop_participant();
    let participant_events = Arc::clone(&events);
    let host = tokio::spawn(async move {
        participant.stopping().await;
        participant_events
            .lock()
            .expect("event lock is healthy")
            .push("participant");
        participant.finish();
    });
    runtime.stop().await.unwrap();
    host.await.expect("the host participant does not panic");
    assert!(matches!(
        running.await,
        Err(Error::OperationRefused(Refusal::Cancelled))
    ));
    assert!(matches!(
        queued.await,
        Err(Error::OperationRefused(Refusal::Cancelled))
    ));
    let trace = events.lock().expect("event lock is healthy").clone();
    assert!(
        matches!(
            trace.as_slice(),
            ["job-start", "job-end", "service", "participant"]
                | ["job-start", "service", "job-end", "participant"]
        ),
        "unexpected shutdown trace: {trace:?}"
    );

    // A retained handle does not hold shutdown open: the stop seals the journal
    // rather than waiting for the last clone to go, so a host that keeps one by
    // design still stops, and stops promptly.
    let runtime = Arc::new(assembly().runtime().await.expect("the boot is valid"));
    let retained = runtime.handle();
    let stopping = tokio::spawn({
        let runtime = Arc::clone(&runtime);

        async move { runtime.stop().await.unwrap() }
    });

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), stopping)
            .await
            .expect("a live handle does not hold the stop open")
            .expect("runtime stop does not panic"),
        Position::ORIGIN
    );
    assert!(
        matches!(
            retained.records.publish(Noted("after".to_owned())).await,
            Err(harmos::Error::Unavailable)
        ),
        "a sealed journal refuses a late write rather than leaving it hanging"
    );
    assert_eq!(
        runtime.stop().await.unwrap(),
        Position::ORIGIN,
        "a repeat stop reports the frontier the first one observed"
    );
}

/* === Tests === */
