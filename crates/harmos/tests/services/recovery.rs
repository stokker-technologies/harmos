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
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{app::App, service_status};
use harmos::services::{ServiceOutcome, ServicePhase};
use harmos::{Fault, Resident, Runtime, ServiceExit, ServicePolicy, ServiceStatus};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct RetainedCleanup {
    gate: Option<tokio::sync::oneshot::Receiver<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
    finalizing: Arc<Notify>,
    attempts: Arc<AtomicUsize>,
    completed: Arc<AtomicBool>,
    panic_first: bool,
    environmental: bool,
}

/* === Enums === */
#[derive(Clone, Copy, Debug)]
enum CleanupRequest {
    Instance,
    Definition,
    Scope,
    Runtime,
}

/* === Traits === */

/* === Implementations === */
impl Resident<App> for RetainedCleanup {
    type Error = String;

    async fn attempt(&mut self, context: harmos::Service<App>) -> Result<(), Fault<String>> {
        let gate = self.gate.take().expect("one original blocking worker");
        let completed = self.completed.clone();
        self.join = Some(tokio::task::spawn_blocking(move || {
            gate.blocking_recv().expect("test releases the worker");
            completed.store(true, Ordering::Release);
        }));
        context.ready();
        if self.environmental {
            return Err(Fault::Environmental("original transport exit".into()));
        }
        std::future::pending().await
    }

    async fn finalize(&mut self) -> Result<(), String> {
        if self.attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            if self.panic_first {
                panic!("first original cleanup panics");
            }
            return Err("first original cleanup fails".into());
        }
        self.finalizing.notify_one();
        if let Some(join) = &mut self.join {
            join.await.map_err(|error| error.to_string())?;
            self.join = None;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn stop_cleanup(
    runtime: &Runtime<App>,
    scope: &harmos::services::ServiceScope<()>,
    request: CleanupRequest,
    lease: &harmos::services::ServiceLease<App>,
) -> harmos::Result<()> {
    match request {
        CleanupRequest::Instance => {
            let instance = lease.instance();
            instance.cancel();
            instance.join().await
        }
        CleanupRequest::Definition => runtime.services.unregister("retained-cleanup").await,
        CleanupRequest::Scope => runtime.services.stop_scope(scope).await,
        CleanupRequest::Runtime => runtime.stop().await.map(|_| ()),
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("unregister_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for retry in [true, false] {
                        let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
                        let entered = Arc::new(Notify::new());
                        let release = Arc::new(Notify::new());
                        let cleaned = Arc::new(AtomicBool::new(false));
                        let signals = (entered.clone(), release.clone(), cleaned.clone());
                        runtime
                            .services
                            .register(
                                harmos::services::ServiceDefinition {
                                    id: "reload".into(),
                                    policy: ServicePolicy::default(),
                                },
                                move |_: Arc<()>| {
                                    let signals = signals.clone();
                                    move |mut context: harmos::Service<App>| {
                                        let (entered, release, cleaned) = signals.clone();
                                        async move {
                                            context.ready();
                                            context.stopping().await;
                                            entered.notify_one();
                                            release.notified().await;
                                            cleaned.store(true, Ordering::SeqCst);
                                            Ok::<(), Fault<String>>(())
                                        }
                                    }
                                },
                            )
                            .unwrap();
                        let _lease = runtime
                            .services
                            .activate("reload", &runtime.services.root_scope())
                            .await
                            .unwrap();
                        service_status(runtime.services.subscribe(), "reload", |status| {
                            matches!(status, ServiceStatus::Running { .. })
                        })
                        .await;

                        {
                            let stopping = runtime.services.unregister("reload");
                            tokio::pin!(stopping);
                            tokio::select! {
                                _ = &mut stopping => panic!("cleanup has not been released"),
                                _ = entered.notified() => {}
                            }
                        }
                        assert!(!cleaned.load(Ordering::SeqCst));
                        if retry {
                            {
                                let stopping = runtime.services.unregister("reload");
                                tokio::pin!(stopping);
                                let pending = std::future::poll_fn(|cx| {
                                    std::task::Poll::Ready(
                                        std::future::Future::poll(stopping.as_mut(), cx)
                                            .is_pending(),
                                    )
                                })
                                .await;
                                assert!(pending, "retry must join the existing cleanup");
                                release.notify_one();
                                stopping.await.unwrap();
                            }
                            assert!(cleaned.load(Ordering::SeqCst));
                            assert!(
                                !runtime
                                    .services
                                    .subscribe()
                                    .borrow()
                                    .contains_key(&harmos::services::ServiceKey::root("reload"))
                            );
                            runtime.stop().await.unwrap();
                        } else {
                            let stopping = runtime.stop();
                            tokio::pin!(stopping);
                            let pending = std::future::poll_fn(|cx| {
                                std::task::Poll::Ready(
                                    std::future::Future::poll(stopping.as_mut(), cx).is_pending(),
                                )
                            })
                            .await;
                            assert!(pending, "shutdown must join the existing cleanup");
                            release.notify_one();
                            stopping.await.unwrap();
                            assert!(cleaned.load(Ordering::SeqCst));
                        }
                    }
                });
        });
    }
    {
        scenario_runtime::check("shutdown_retains_resident", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for tenant in [false, true] {
                        let entered = Arc::new(Notify::new());
                        let release = Arc::new(Notify::new());
                        let cleaned = Arc::new(AtomicBool::new(false));
                        let signals = (entered.clone(), release.clone(), cleaned.clone());
                        let resident = move |mut context: harmos::Service<App>| {
                            let (entered, release, cleaned) = signals.clone();
                            async move {
                                context.ready();
                                context.stopping().await;
                                entered.notify_one();
                                release.notified().await;
                                cleaned.store(true, Ordering::SeqCst);
                                Ok::<(), Fault<String>>(())
                            }
                        };
                        let mut lease = None;
                        let runtime = if tenant {
                            let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
                            runtime
                                .services
                                .register(
                                    harmos::services::ServiceDefinition {
                                        id: "cleanup".into(),
                                        policy: ServicePolicy::default(),
                                    },
                                    move |_: Arc<()>| resident.clone(),
                                )
                                .unwrap();
                            lease = Some(
                                runtime
                                    .services
                                    .activate("cleanup", &runtime.services.root_scope())
                                    .await
                                    .unwrap(),
                            );
                            runtime
                        } else {
                            Runtime::<App>::builder((), ())
                                .service("cleanup", resident)
                                .runtime()
                                .await
                                .unwrap()
                        };
                        service_status(runtime.services.subscribe(), "cleanup", |status| {
                            matches!(status, ServiceStatus::Running { .. })
                        })
                        .await;

                        {
                            let stop = runtime.stop();
                            tokio::pin!(stop);
                            tokio::select! {
                                _ = &mut stop => panic!("cleanup has not finished"),
                                _ = entered.notified() => {}
                            }
                        }
                        assert!(
                            runtime
                                .services
                                .activate("late", &runtime.services.root_scope())
                                .await
                                .is_err()
                        );
                        let stop = runtime.stop();
                        tokio::pin!(stop);
                        assert!(
                            tokio::time::timeout(Duration::from_millis(10), &mut stop)
                                .await
                                .is_err()
                        );
                        assert!(!cleaned.load(Ordering::SeqCst));
                        release.notify_one();
                        stop.await.unwrap();
                        drop(lease);
                        assert!(cleaned.load(Ordering::SeqCst));
                    }
                });
        });
    }
    {
        scenario_runtime::check("resident_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    for (panic_first, request) in [false, true].into_iter().flat_map(|panic_first| {
        [
            CleanupRequest::Instance,
            CleanupRequest::Definition,
            CleanupRequest::Scope,
            CleanupRequest::Runtime,
        ]
        .map(|request| (panic_first, request))
    }) {
        let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
        let (release, gate) = tokio::sync::oneshot::channel();
        let gate = Arc::new(Mutex::new(Some(gate)));
        let factories = Arc::new(AtomicUsize::new(0));
        let attempts = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        let finalizing = Arc::new(Notify::new());
        let resources = (
            gate,
            factories.clone(),
            attempts.clone(),
            completed.clone(),
            finalizing.clone(),
        );
        runtime
            .services
            .register(
                harmos::services::ServiceDefinition {
                    id: "retained-cleanup".into(),
                    policy: ServicePolicy {
                        stop_timeout: Duration::from_millis(10),
                        ..ServicePolicy::default()
                    },
                },
                move |_: Arc<()>| {
                    let (gate, factories, attempts, completed, finalizing) = &resources;
                    factories.fetch_add(1, Ordering::AcqRel);
                    RetainedCleanup {
                        gate: gate.lock().unwrap().take(),
                        join: None,
                        finalizing: finalizing.clone(),
                        attempts: attempts.clone(),
                        completed: completed.clone(),
                        panic_first,
                        environmental: false,
                    }
                },
            )
            .unwrap();
        let scope = runtime.services.root_scope();
        let lease = runtime
            .services
            .activate("retained-cleanup", &scope)
            .await
            .unwrap();
        lease.ready().await.unwrap();
        assert!(
            stop_cleanup(&runtime, &scope, request, &lease)
                .await
                .is_err()
        );
        assert_eq!(lease.lifecycle().phase(), ServicePhase::Finalizing);
        assert_eq!(lease.lifecycle().outcome(), Some(ServiceOutcome::TimedOut));
        assert!(lease.lifecycle().cleanup_pending());
        assert!(lease.lifecycle().settled_at().is_none());
        assert!(!completed.load(Ordering::Acquire));
        assert_eq!(attempts.load(Ordering::Acquire), 1);
        if !matches!(request, CleanupRequest::Instance) {
            assert!(matches!(
                runtime.services.activate("retained-cleanup", &scope).await,
                Err(harmos::Error::Closing { .. })
            ));
        }
        {
            let retry = stop_cleanup(&runtime, &scope, request, &lease);
            tokio::pin!(retry);
            tokio::select! {
                _ = &mut retry => panic!("the original blocking owner is still running"),
                _ = finalizing.notified() => {},
            }
        }
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        assert_eq!(factories.load(Ordering::Acquire), 1);
        release.send(()).unwrap();
        // Physical cleanup completes, while the original timeout still reports
        // failure. The cancelled retry never starts another finalizer or factory.
        let error = stop_cleanup(&runtime, &scope, request, &lease)
            .await
            .unwrap_err();
        let mut cause = &error;
        loop {
            let harmos::Error::TaskFailed { source, .. } = cause else {
                panic!("{request:?} lost original failure: {error:?}");
            };
            if let Some(exit) = source.downcast_ref::<ServiceExit>() {
                assert!(matches!(exit, ServiceExit::StopTimedOut));
                break;
            }
            cause = source
                .downcast_ref::<harmos::Error>()
                .expect("original structured service cause");
        }
        assert!(completed.load(Ordering::Acquire));
        assert_eq!(lease.lifecycle().phase(), ServicePhase::Settled);
        assert_eq!(lease.lifecycle().outcome(), Some(ServiceOutcome::TimedOut));
        assert!(!lease.lifecycle().cleanup_pending());
        assert!(lease.lifecycle().settled_at().is_some());
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        assert_eq!(factories.load(Ordering::Acquire), 1);
        if matches!(request, CleanupRequest::Definition | CleanupRequest::Scope) {
            assert!(
                runtime
                    .services
                    .status(&harmos::services::ServiceKey::root("retained-cleanup"))
                    .is_none()
            );
        }
        drop(lease);
        if matches!(request, CleanupRequest::Instance | CleanupRequest::Runtime) {
            assert!(runtime.stop().await.is_err());
        } else {
            runtime.stop().await.unwrap();
        }
    }

});
        });
    }
    {
        scenario_runtime::check("exit_survives_physical", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
    let (release, gate) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let attempts = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let observed = completed.clone();
    runtime
        .services
        .register(
            harmos::services::ServiceDefinition {
                id: "environmental-cleanup".into(),
                policy: ServicePolicy::default(),
            },
            move |_: Arc<()>| RetainedCleanup {
                gate: gate.lock().unwrap().take(),
                join: None,
                finalizing: Arc::new(Notify::new()),
                attempts: attempts.clone(),
                completed: completed.clone(),
                panic_first: false,
                environmental: true,
            },
        )
        .unwrap();
    let lease = runtime
        .services
        .activate("environmental-cleanup", &runtime.services.root_scope())
        .await
        .unwrap();
    assert!(lease.ready().await.is_err());
    assert!(!observed.load(Ordering::Acquire));
    release.send(()).unwrap();
    let error = runtime
        .services
        .unregister("environmental-cleanup")
        .await
        .unwrap_err();
    let harmos::Error::TaskFailed { source, .. } = error else {
        panic!("original generation has a structured failure");
    };
    let Some(harmos::Error::TaskFailed { source, .. }) = source.downcast_ref::<harmos::Error>()
    else {
        panic!("original attempt failure remains available");
    };
    assert!(
        matches!(source.downcast_ref::<Fault<String>>(), Some(Fault::Environmental(message)) if message == "original transport exit")
    );
    assert!(observed.load(Ordering::Acquire));
    drop(lease);
    runtime.stop().await.unwrap();

});
        });
    }
}
