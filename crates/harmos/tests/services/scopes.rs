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
    atomic::{AtomicUsize, Ordering::SeqCst},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::{Bank, assembly};
use harmos::services::{ServiceDefinition, ServiceManager};
use harmos::{Error, Fault, Service, ServicePolicy, ServiceStatus};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Resources {
    started: AtomicUsize,
    finished: AtomicUsize,
    stopping: Notify,
    cleanup: Notify,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn register(manager: &ServiceManager<Bank>) {
    manager
        .register(
            ServiceDefinition {
                id: "scoped".into(),
                policy: ServicePolicy::default(),
            },
            |resources: Arc<Resources>| {
                move |mut service: Service<Bank>| {
                    let resources = resources.clone();
                    async move {
                        resources.started.fetch_add(1, SeqCst);
                        service.ready();
                        service.stopping().await;
                        resources.stopping.notify_one();
                        resources.cleanup.notified().await;
                        resources.finished.fetch_add(1, SeqCst);
                        Ok::<(), Fault<&'static str>>(())
                    }
                }
            },
        )
        .unwrap();
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("control_retains_join", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let resources = Arc::new(Resources::default());
                    let scope = manager.scope(resources.clone()).unwrap();
                    let first = manager.activate("scoped", &scope).await.unwrap();
                    let second = manager.activate("scoped", &scope).await.unwrap();
                    first.ready().await.unwrap();
                    let instance = first.instance();
                    let retained = instance.clone();
                    assert_eq!(instance.id(), second.id());
                    assert_eq!(instance.key(), second.key());
                    drop(instance);
                    assert_eq!(manager.status(second.key()).unwrap().leases, 2);

                    retained.cancel();
                    retained.cancel();
                    assert!(matches!(
                        manager.status(second.key()).unwrap().status,
                        ServiceStatus::Stopping
                    ));
                    tokio::time::timeout(Duration::from_secs(1), resources.stopping.notified())
                        .await
                        .unwrap();
                    {
                        let joining = retained.join();
                        tokio::pin!(joining);
                        std::future::poll_fn(|cx| {
                            assert!(std::future::Future::poll(joining.as_mut(), cx).is_pending());
                            std::task::Poll::Ready(())
                        })
                        .await;
                    }
                    assert_eq!(resources.finished.load(SeqCst), 0);
                    resources.cleanup.notify_one();
                    tokio::time::timeout(Duration::from_secs(1), retained.join())
                        .await
                        .unwrap()
                        .unwrap();
                    retained.join().await.unwrap();
                    assert_eq!(resources.finished.load(SeqCst), 1);
                    assert!(retained.status().settled_at().is_some());

                    let replacement = manager.activate("scoped", &scope).await.unwrap();
                    replacement.ready().await.unwrap();
                    assert_ne!(replacement.id(), retained.id());
                    retained.cancel();
                    retained.join().await.unwrap();
                    first.release().await.unwrap();
                    second.release().await.unwrap();
                    assert_eq!(manager.status(replacement.key()).unwrap().leases, 1);
                    assert!(matches!(
                        manager.status(replacement.key()).unwrap().status,
                        ServiceStatus::Running { .. }
                    ));
                    resources.cleanup.notify_one();
                    replacement.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("final_release_joins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let resources = Arc::new(Resources::default());
                    let scope = manager.scope(resources.clone()).unwrap();
                    let (first, second) = tokio::join!(
                        manager.activate("scoped", &scope),
                        manager.activate("scoped", &scope)
                    );
                    let first = first.unwrap();
                    let second = second.unwrap();
                    first.ready().await.unwrap();
                    assert_eq!(first.id(), second.id());
                    assert_eq!(manager.status(first.key()).unwrap().id, first.id());
                    assert_eq!(resources.started.load(SeqCst), 1);
                    assert_eq!(manager.status(first.key()).unwrap().leases, 2);
                    first.release().await.unwrap();
                    assert_eq!(manager.status(second.key()).unwrap().leases, 1);
                    assert!(matches!(
                        manager.status(second.key()).unwrap().status,
                        ServiceStatus::Running { .. }
                    ));

                    let release = second.release();
                    tokio::pin!(release);
                    tokio::select! {
                        result = &mut release => panic!("cleanup must be joined: {result:?}"),
                        _ = resources.stopping.notified() => {}
                    }
                    resources.cleanup.notify_one();
                    release.await.unwrap();
                    assert_eq!(resources.finished.load(SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("resource_scopes_independent", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let foreign = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let a = Arc::new(Resources::default());
                    let b = Arc::new(Resources::default());
                    let sa = manager.scope(a.clone()).unwrap();
                    let sb = manager.scope(b.clone()).unwrap();
                    let first = manager.activate("scoped", &sa).await.unwrap();
                    let second = manager.activate("scoped", &sb).await.unwrap();
                    first.ready().await.unwrap();
                    second.ready().await.unwrap();
                    assert_ne!(first.key(), second.key());
                    assert_eq!(a.started.load(SeqCst), 1);
                    assert_eq!(b.started.load(SeqCst), 1);
                    assert!(matches!(
                        manager.activate("scoped", &manager.root_scope()).await,
                        Err(Error::InvalidInput { .. })
                    ));
                    let other = foreign
                        .services
                        .scope(Arc::new(Resources::default()))
                        .unwrap();
                    assert!(matches!(
                        manager.activate("scoped", &other).await,
                        Err(Error::InvalidInput { .. })
                    ));
                    a.cleanup.notify_one();
                    manager.stop_scope(&sa).await.unwrap();
                    assert!(matches!(
                        manager.activate("scoped", &sa).await,
                        Err(Error::Closing {
                            domain: "service scope"
                        })
                    ));
                    assert!(matches!(
                        manager.status(second.key()).unwrap().status,
                        ServiceStatus::Running { .. }
                    ));
                    first.release().await.unwrap();
                    b.cleanup.notify_one();
                    second.release().await.unwrap();
                    runtime.stop().await.unwrap();
                    foreign.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("dropped_lease_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let resources = Arc::new(Resources::default());
                    let scope = manager.scope(resources.clone()).unwrap();
                    let first = manager.activate("scoped", &scope).await.unwrap();
                    first.ready().await.unwrap();
                    let generation = first.id();
                    drop(first);
                    resources.stopping.notified().await;
                    let next = manager.activate("scoped", &scope);
                    tokio::pin!(next);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), &mut next)
                            .await
                            .is_err()
                    );
                    assert_eq!(resources.started.load(SeqCst), 1);
                    resources.cleanup.notify_one();
                    let second = next.await.unwrap();
                    second.ready().await.unwrap();
                    assert_ne!(second.id(), generation);
                    assert_eq!(resources.finished.load(SeqCst), 1);
                    assert_eq!(resources.started.load(SeqCst), 2);
                    resources.cleanup.notify_one();
                    second.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scope_stop_closes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let resources = Arc::new(Resources::default());
                    let scope = manager.scope(resources.clone()).unwrap();
                    let lease = manager.activate("scoped", &scope).await.unwrap();
                    lease.ready().await.unwrap();
                    {
                        let stop = manager.stop_scope(&scope);
                        tokio::pin!(stop);
                        tokio::select! {
                            _ = &mut stop => panic!("cleanup is gated"),
                            _ = resources.stopping.notified() => {}
                        }
                    }
                    assert!(matches!(
                        manager.activate("scoped", &scope).await,
                        Err(Error::Closing {
                            domain: "service scope"
                        })
                    ));
                    assert_eq!(manager.status(lease.key()).unwrap().leases, 0);
                    resources.cleanup.notify_one();
                    manager.stop_scope(&scope).await.unwrap();
                    assert_eq!(resources.finished.load(SeqCst), 1);
                    lease.release().await.unwrap();
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
                    let runtime = assembly().runtime().await.unwrap();
                    let manager = &runtime.services;
                    register(manager);
                    let resources = Arc::new(Resources::default());
                    let scope = manager.scope(resources.clone()).unwrap();
                    let stale = manager.activate("scoped", &scope).await.unwrap();
                    stale.ready().await.unwrap();
                    {
                        let remove = manager.unregister("scoped");
                        tokio::pin!(remove);
                        tokio::select! {
                            _ = &mut remove => panic!("cleanup is gated"),
                            _ = resources.stopping.notified() => {}
                        }
                        assert!(matches!(
                            manager.activate("scoped", &scope).await,
                            Err(Error::Closing {
                                domain: "service definition"
                            })
                        ));
                        assert!(matches!(
                            manager.register(
                                ServiceDefinition {
                                    id: "scoped".into(),
                                    policy: ServicePolicy::default()
                                },
                                |_: Arc<Resources>| super::resident
                            ),
                            Err(Error::AlreadyRegistered { .. })
                        ));
                        resources.cleanup.notify_one();
                        remove.await.unwrap();
                    }
                    register(manager);
                    let replacement = manager.activate("scoped", &scope).await.unwrap();
                    replacement.ready().await.unwrap();
                    assert_ne!(replacement.id(), stale.id());
                    stale.release().await.unwrap();
                    assert_eq!(manager.status(replacement.key()).unwrap().leases, 1);
                    assert!(matches!(
                        manager.status(replacement.key()).unwrap().status,
                        ServiceStatus::Running { .. }
                    ));
                    resources.cleanup.notify_one();
                    replacement.release().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_preserve_typed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    #[derive(Debug, thiserror::Error)]
                    #[error("initialization refused: {0}")]
                    struct Refused(u32);

                    fn typed(error: &(dyn std::error::Error + 'static)) -> bool {
                        if let Some(Fault::Application(Refused(42))) =
                            error.downcast_ref::<Fault<Refused>>()
                        {
                            true
                        } else if let Some(Error::TaskFailed { source, .. }) =
                            error.downcast_ref::<Error>()
                        {
                            typed(source.as_ref())
                        } else {
                            error.source().is_some_and(typed)
                        }
                    }

                    let runtime = assembly().runtime().await.unwrap();
                    runtime
                        .services
                        .register(
                            ServiceDefinition {
                                id: "broken".into(),
                                policy: ServicePolicy::default(),
                            },
                            |_: Arc<()>| {
                                |_service: Service<Bank>| async {
                                    Err::<(), _>(Fault::Application(Refused(42)))
                                }
                            },
                        )
                        .unwrap();
                    let lease = runtime
                        .services
                        .activate("broken", &runtime.services.root_scope())
                        .await
                        .unwrap();
                    let error = lease.ready().await.unwrap_err();
                    assert!(typed(&error), "{error:?}");
                    assert!(typed(&lease.release().await.unwrap_err()));
                    for _ in 0..2 {
                        assert!(typed(&runtime.stop().await.unwrap_err()));
                    }
                });
        });
    }
    {
        scenario_runtime::check("shutdown_signals_scope", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly().runtime().await.unwrap();
    register(&runtime.services);
    let a = Arc::new(Resources::default());
    let b = Arc::new(Resources::default());
    let sa = runtime.services.scope(a.clone()).unwrap();
    let sb = runtime.services.scope(b.clone()).unwrap();
    let first = runtime.services.activate("scoped", &sa).await.unwrap();
    let second = runtime.services.activate("scoped", &sb).await.unwrap();
    first.ready().await.unwrap();
    second.ready().await.unwrap();
    let stop = runtime.stop();
    tokio::pin!(stop);
    tokio::select! {
        _ = &mut stop => panic!("both cleanups are gated"),
        _ = async { a.stopping.notified().await; b.stopping.notified().await; } => {}
    }
    a.cleanup.notify_one();
    b.cleanup.notify_one();
    stop.await.unwrap();
    assert_eq!(a.finished.load(SeqCst), 1);
    assert_eq!(b.finished.load(SeqCst), 1);

});
        });
    }
    {
        scenario_runtime::check("unregister_drops_factory", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    struct Reentrant {
                        manager: ServiceManager<Bank>,
                        dropped: Arc<AtomicUsize>,
                    }
                    impl Drop for Reentrant {
                        fn drop(&mut self) {
                            assert!(self.manager.definition("unused").is_none());
                            self.dropped.fetch_add(1, SeqCst);
                        }
                    }
                    let runtime = assembly().runtime().await.unwrap();
                    let dropped = Arc::new(AtomicUsize::new(0));
                    let resource = Arc::new(Reentrant {
                        manager: runtime.services.clone(),
                        dropped: dropped.clone(),
                    });
                    runtime
                        .services
                        .register(
                            ServiceDefinition {
                                id: "unused".into(),
                                policy: ServicePolicy::default(),
                            },
                            move |_: Arc<()>| {
                                let _owner = resource.clone();
                                super::resident
                            },
                        )
                        .unwrap();
                    runtime.services.unregister("unused").await.unwrap();
                    assert_eq!(dropped.load(SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("resident_not_reinitialized", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .service("one-shot", |_service| async { Ok::<(), Fault<&str>>(()) })
                        .runtime()
                        .await
                        .unwrap();
                    let key = harmos::services::ServiceKey::root("one-shot");
                    runtime
                        .services
                        .subscribe()
                        .wait_for(|values| {
                            values
                                .get(&key)
                                .is_some_and(|instance| instance.status == ServiceStatus::Stopped)
                        })
                        .await
                        .unwrap();
                    assert!(matches!(
                        runtime
                            .services
                            .activate("one-shot", &runtime.services.root_scope())
                            .await,
                        Err(Error::NotReady {
                            domain: "retired assembly service"
                        })
                    ));
                    assert_eq!(
                        runtime.services.status(&key).unwrap().status,
                        ServiceStatus::Stopped
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("factory_panic_reported", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    runtime
                        .services
                        .register(
                            ServiceDefinition {
                                id: "factory".into(),
                                policy: ServicePolicy::default(),
                            },
                            |fail: Arc<bool>| {
                                assert!(!*fail, "factory initialization failed");
                                super::resident
                            },
                        )
                        .unwrap();
                    let scope = runtime.services.scope(Arc::new(true)).unwrap();
                    let lease = runtime.services.activate("factory", &scope).await.unwrap();
                    assert!(matches!(lease.ready().await, Err(Error::TaskFailed { .. })));
                    assert!(matches!(
                        runtime.services.status(lease.key()).unwrap().status,
                        ServiceStatus::Inactive {
                            cause: harmos::ServiceExit::Panicked,
                            ..
                        }
                    ));
                    assert!(matches!(
                        runtime.stop().await,
                        Err(Error::TaskFailed { .. })
                    ));
                });
        });
    }
}

/* === Tests === */
