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
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{app::App, service_status};
use harmos_runtime::operations::{OperationOutcome, OperationPhase};
use harmos_runtime::{Runtime, services::Resident, services::ServiceExit, services::ServiceStatus};

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
    type Error = std::io::Error;

    async fn run(&mut self, context: harmos_runtime::services::ServiceContext<App>) -> Result<(), Self::Error> {
        let gate = self.gate.take().expect("one original blocking worker");
        let completed = self.completed.clone();
        self.join = Some(tokio::task::spawn_blocking(move || {
            gate.blocking_recv().expect("test releases the worker");
            completed.store(true, Ordering::Release);
        }));
        context.ready();
        if self.environmental {
            return Err(std::io::Error::other("original transport exit"));
        }
        std::future::pending().await
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            if self.panic_first {
                panic!("first original cleanup panics");
            }
            return Err(std::io::Error::other("first original cleanup fails"));
        }
        self.finalizing.notify_one();
        if let Some(join) = &mut self.join {
            join.await.map_err(std::io::Error::other)?;
            self.join = None;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn stop_cleanup(
    runtime: &Runtime<App>, scope: &Arc<()>, request: CleanupRequest,
    lease: &harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>,
) -> harmos_runtime::Result<()> {
    match request {
        CleanupRequest::Instance => {
            lease.cancel();
            lease.settle().await
        }
        CleanupRequest::Definition => runtime.operations.unregister_resident("retained-cleanup").await,
        CleanupRequest::Scope => runtime.operations.retire_resources(scope).await,
        CleanupRequest::Runtime => runtime.stop().await.map(|_| ()),
    }
}

/* === Tests === */

#[tokio::test]
async fn unregister_retains_cleanup() {
    for retry in [true, false] {
        let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let cleaned = Arc::new(AtomicBool::new(false));
        let signals = (entered.clone(), release.clone(), cleaned.clone());
        runtime
            .operations
            .register_service(
                harmos_runtime::services::ServiceDefinition { id: "reload".into(), policy: harmos_runtime::services::POLICY },
                move |_: Arc<()>| {
                    let signals = signals.clone();
                    move |mut context: harmos_runtime::services::ServiceContext<App>| {
                        let (entered, release, cleaned) = signals.clone();
                        async move {
                            context.ready();
                            context.stopping().await;
                            entered.notify_one();
                            release.notified().await;
                            cleaned.store(true, Ordering::SeqCst);
                            Ok::<(), std::io::Error>(())
                        }
                    }
                },
            )
            .unwrap();
        let _lease = runtime.operations.submit_resident("reload").await.unwrap();
        service_status(runtime.operations.watch_residents(), "reload", |status| matches!(status, ServiceStatus::Running { .. })).await;

        {
            let stopping = runtime.operations.unregister_resident("reload");
            tokio::pin!(stopping);
            tokio::select! {
                _ = &mut stopping => panic!("cleanup has not been released"),
                _ = entered.notified() => {}
            }
        }
        assert!(!cleaned.load(Ordering::SeqCst));
        if retry {
            {
                let stopping = runtime.operations.unregister_resident("reload");
                tokio::pin!(stopping);
                let pending =
                    std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(stopping.as_mut(), cx).is_pending())).await;
                assert!(pending, "retry must join the existing cleanup");
                release.notify_one();
                stopping.await.unwrap();
            }
            assert!(cleaned.load(Ordering::SeqCst));
            assert!(
                !runtime.operations.watch_residents().borrow().contains_key(&harmos_runtime::operations::resident::Key::root("reload"))
            );
            runtime.stop().await.unwrap();
        } else {
            let stopping = runtime.stop();
            tokio::pin!(stopping);
            let pending =
                std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(stopping.as_mut(), cx).is_pending())).await;
            assert!(pending, "shutdown must join the existing cleanup");
            release.notify_one();
            stopping.await.unwrap();
            assert!(cleaned.load(Ordering::SeqCst));
        }
    }
}

#[tokio::test]
async fn stop_joins_cleanup() {
    for tenant in [false, true] {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let cleaned = Arc::new(AtomicBool::new(false));
        let signals = (entered.clone(), release.clone(), cleaned.clone());
        let resident = move |mut context: harmos_runtime::services::ServiceContext<App>| {
            let (entered, release, cleaned) = signals.clone();
            async move {
                context.ready();
                context.stopping().await;
                entered.notify_one();
                release.notified().await;
                cleaned.store(true, Ordering::SeqCst);
                Ok::<(), std::io::Error>(())
            }
        };
        let mut lease = None;
        let runtime = if tenant {
            let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
            runtime
                .operations
                .register_service(
                    harmos_runtime::services::ServiceDefinition { id: "cleanup".into(), policy: harmos_runtime::services::POLICY },
                    move |_: Arc<()>| resident.clone(),
                )
                .unwrap();
            lease = Some(runtime.operations.submit_resident("cleanup").await.unwrap());
            runtime
        } else {
            Runtime::<App>::builder((), ()).service("cleanup", resident).runtime().await.unwrap()
        };
        service_status(runtime.operations.watch_residents(), "cleanup", |status| matches!(status, ServiceStatus::Running { .. })).await;

        {
            let stop = runtime.stop();
            tokio::pin!(stop);
            tokio::select! {
                _ = &mut stop => panic!("cleanup has not finished"),
                _ = entered.notified() => {}
            }
        }
        let stop = runtime.stop();
        tokio::pin!(stop);
        assert!(futures_util::poll!(&mut stop).is_pending());
        assert!(!cleaned.load(Ordering::SeqCst));
        release.notify_one();
        stop.await.unwrap();
        drop(lease);
        assert!(cleaned.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn timeout_retains_cleanup() {
    for (panic_first, request) in [false, true].into_iter().flat_map(|panic_first| {
        [CleanupRequest::Instance, CleanupRequest::Definition, CleanupRequest::Scope, CleanupRequest::Runtime]
            .map(|request| (panic_first, request))
    }) {
        let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
        let (release, gate) = tokio::sync::oneshot::channel();
        let gate = Arc::new(Mutex::new(Some(gate)));
        let factories = Arc::new(AtomicUsize::new(0));
        let attempts = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        let finalizing = Arc::new(Notify::new());
        let resources = (gate, factories.clone(), attempts.clone(), completed.clone(), finalizing.clone());
        runtime
            .operations
            .register_service(
                harmos_runtime::services::ServiceDefinition {
                    id: "retained-cleanup".into(),
                    policy: harmos_runtime::ServicePolicy { stop_timeout: Duration::from_millis(10), ..harmos_runtime::services::POLICY },
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
        let scope = runtime.operations.root_resources();
        let lease = runtime.operations.submit_resident("retained-cleanup").await.unwrap();
        lease.ready().await.unwrap();
        assert!(stop_cleanup(&runtime, &scope, request, &lease).await.is_err());
        assert_eq!(lease.status().phase(), OperationPhase::Finalizing);
        assert_eq!(lease.status().outcome(), Some(OperationOutcome::TimedOut));
        assert!(lease.status().cleanup_pending());
        assert!(lease.status().settled_at().is_none());
        assert!(!completed.load(Ordering::Acquire));
        assert_eq!(attempts.load(Ordering::Acquire), 1);
        if !matches!(request, CleanupRequest::Instance) {
            assert!(matches!(runtime.operations.submit_resident("retained-cleanup").await, Err(harmos_runtime::Error::Closing { .. })));
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
        let error = stop_cleanup(&runtime, &scope, request, &lease).await.unwrap_err();
        let mut cause = &error;
        loop {
            let harmos_runtime::Error::TaskFailed { source, .. } = cause else {
                panic!("{request:?} lost original failure: {error:?}");
            };
            if let Some(exit) = source.downcast_ref::<ServiceExit>() {
                assert!(matches!(exit, ServiceExit::StopTimedOut));
                break;
            }
            cause = source.downcast_ref::<harmos_runtime::Error>().expect("original structured service cause");
        }
        assert!(completed.load(Ordering::Acquire));
        assert_eq!(lease.status().phase(), OperationPhase::Stopped);
        assert_eq!(lease.status().outcome(), Some(OperationOutcome::TimedOut));
        assert!(!lease.status().cleanup_pending());
        assert!(lease.status().settled_at().is_some());
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        assert_eq!(factories.load(Ordering::Acquire), 1);
        if matches!(request, CleanupRequest::Definition | CleanupRequest::Scope) {
            assert!(runtime.operations.resident(&harmos_runtime::operations::resident::Key::root("retained-cleanup")).is_none());
        }
        drop(lease);
        if matches!(request, CleanupRequest::Instance | CleanupRequest::Runtime) {
            assert!(runtime.stop().await.is_err());
        } else {
            runtime.stop().await.unwrap();
        }
    }
}

#[tokio::test]
async fn exit_cause_preserved() {
    let runtime = Runtime::<App>::builder((), ()).runtime().await.unwrap();
    let (release, gate) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let attempts = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let observed = completed.clone();
    runtime
        .operations
        .register_service(
            harmos_runtime::services::ServiceDefinition { id: "environmental-cleanup".into(), policy: harmos_runtime::services::POLICY },
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
    let lease = runtime.operations.submit_resident("environmental-cleanup").await.unwrap();
    assert!(lease.ready().await.is_err());
    assert!(!observed.load(Ordering::Acquire));
    release.send(()).unwrap();
    let error = runtime.operations.unregister_resident("environmental-cleanup").await.unwrap_err();
    let harmos_runtime::Error::TaskFailed { source, .. } = error else {
        panic!("original generation has a structured failure");
    };
    let Some(harmos_runtime::Error::TaskFailed { source, .. }) = source.downcast_ref::<harmos_runtime::Error>() else {
        panic!("original attempt failure remains available");
    };
    assert_eq!(source.to_string(), "original transport exit");
    assert!(observed.load(Ordering::Acquire));
    drop(lease);
    runtime.stop().await.unwrap();
}
