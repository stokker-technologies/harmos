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
    Arc,
    atomic::{AtomicUsize, Ordering::SeqCst},
};
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::{Bank, assembly};
use harmos_runtime::operations::OperationManager;
use harmos_runtime::services::ServiceDefinition;
use harmos_runtime::{Error, services::Fault, services::Service, services::ServiceStatus};

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
fn register(manager: &OperationManager) {
    manager
        .register_service(
            ServiceDefinition {
                id: "scoped".into(),
                policy: harmos_runtime::services::POLICY,
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

#[tokio::test]
async fn control_retains_join() {
    let runtime = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let resources = Arc::new(Resources::default());
    let first = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    let second = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    first.ready().await.unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(
        manager.resident_key(first.id()),
        manager.resident_key(second.id())
    );
    assert_eq!(
        manager
            .resident(&manager.resident_key(second.id()).unwrap())
            .unwrap()
            .leases,
        2
    );

    first.cancel();
    first.cancel();
    assert!(matches!(
        manager
            .resident(&manager.resident_key(second.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Stopping)
    ));
    (resources.stopping.notified()).await;
    {
        let joining = first.settle();
        tokio::pin!(joining);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(joining.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
    }
    assert_eq!(resources.finished.load(SeqCst), 0);
    resources.cleanup.notify_one();
    (first.settle()).await
        .unwrap();
    first.settle().await.unwrap();
    assert_eq!(resources.finished.load(SeqCst), 1);
    assert!(first.status().settled_at().is_some());

    let replacement = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    replacement.ready().await.unwrap();
    assert_ne!(replacement.id(), first.id());
    first.cancel();
    first.settle().await.unwrap();
    first.release().await.unwrap();
    second.release().await.unwrap();
    assert_eq!(
        manager
            .resident(&manager.resident_key(replacement.id()).unwrap())
            .unwrap()
            .leases,
        1
    );
    assert!(matches!(
        manager
            .resident(&manager.resident_key(replacement.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Running { .. })
    ));
    resources.cleanup.notify_one();
    replacement.release().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn final_release_joins() {
    let runtime = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let resources = Arc::new(Resources::default());
    let (first, second) = tokio::join!(
        manager.submit_with_resources_at("scoped", resources.clone()),
        manager.submit_with_resources_at("scoped", resources.clone())
    );
    let first = first.unwrap();
    let second = second.unwrap();
    first.ready().await.unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(
        manager
            .resident(&manager.resident_key(first.id()).unwrap())
            .unwrap()
            .instance,
        first.id()
    );
    assert_eq!(resources.started.load(SeqCst), 1);
    assert_eq!(
        manager
            .resident(&manager.resident_key(first.id()).unwrap())
            .unwrap()
            .leases,
        2
    );
    first.release().await.unwrap();
    assert_eq!(
        manager
            .resident(&manager.resident_key(second.id()).unwrap())
            .unwrap()
            .leases,
        1
    );
    assert!(matches!(
        manager
            .resident(&manager.resident_key(second.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Running { .. })
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
}

#[tokio::test]
async fn resource_scopes_independent() {
    let runtime = assembly().runtime().await.unwrap();
    let foreign = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let a = Arc::new(Resources::default());
    let b = Arc::new(Resources::default());
    let first = runtime
        .operations
        .submit_with_resources_at("scoped", a.clone())
        .await
        .unwrap();
    let second = runtime
        .operations
        .submit_with_resources_at("scoped", b.clone())
        .await
        .unwrap();
    first.ready().await.unwrap();
    second.ready().await.unwrap();
    assert_ne!(
        manager.resident_key(first.id()),
        manager.resident_key(second.id())
    );
    assert_eq!(a.started.load(SeqCst), 1);
    assert_eq!(b.started.load(SeqCst), 1);
    assert!(matches!(
        manager.submit_resident("scoped").await,
        Err(Error::InvalidInput { .. })
    ));
    assert!(matches!(
        foreign
            .operations
            .submit_with_resources_at("scoped", a.clone())
            .await,
        Err(Error::NotRegistered {
            domain: "resident",
            ..
        })
    ));
    a.cleanup.notify_one();
    manager.retire_resources(&a).await.unwrap();
    assert!(manager.resident_key(first.id()).is_none());
    let revived = manager
        .submit_with_resources_at("scoped", a.clone())
        .await
        .unwrap();
    revived.ready().await.unwrap();
    assert_ne!(revived.id(), first.id());
    assert_eq!(a.started.load(SeqCst), 2);
    assert!(matches!(
        manager
            .resident(&manager.resident_key(second.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Running { .. })
    ));
    first.release().await.unwrap();
    a.cleanup.notify_one();
    revived.release().await.unwrap();
    b.cleanup.notify_one();
    second.release().await.unwrap();
    runtime.stop().await.unwrap();
    foreign.stop().await.unwrap();
}

#[tokio::test]
async fn dropped_lease_cleanup() {
    let runtime = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let resources = Arc::new(Resources::default());
    let first = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    first.ready().await.unwrap();
    let generation = first.id();
    drop(first);
    resources.stopping.notified().await;
    let next = manager.submit_with_resources_at("scoped", resources.clone());
    tokio::pin!(next);
    assert!(futures_util::poll!(&mut next).is_pending());
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
}

#[tokio::test]
async fn scope_stop_closes() {
    let runtime = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let resources = Arc::new(Resources::default());
    let lease = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    lease.ready().await.unwrap();
    {
        let stop = manager.retire_resources(&resources);
        tokio::pin!(stop);
        tokio::select! {
            _ = &mut stop => panic!("cleanup is gated"),
            _ = resources.stopping.notified() => {}
        }
    }
    assert!(matches!(
        manager.submit_with_resources_at("scoped", resources.clone()).await,
        Err(Error::Closing { domain })
            if domain.ends_with("scope")
    ));
    assert_eq!(
        manager
            .resident(&manager.resident_key(lease.id()).unwrap())
            .unwrap()
            .leases,
        0
    );
    resources.cleanup.notify_one();
    manager.retire_resources(&resources).await.unwrap();
    assert_eq!(resources.finished.load(SeqCst), 1);
    lease.release().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn unregister_reserves_name() {
    let runtime = assembly().runtime().await.unwrap();
    let manager = &runtime.operations;
    register(manager);
    let resources = Arc::new(Resources::default());
    let stale = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    stale.ready().await.unwrap();
    {
        let remove = manager.unregister_resident("scoped");
        tokio::pin!(remove);
        tokio::select! {
            _ = &mut remove => panic!("cleanup is gated"),
            _ = resources.stopping.notified() => {}
        }
        assert!(matches!(
            manager.submit_with_resources_at("scoped", resources.clone()).await,
            Err(Error::Closing { domain })
                if domain.ends_with("definition")
        ));
        assert!(matches!(
            manager.register_service(
                ServiceDefinition {
                    id: "scoped".into(),
                    policy: harmos_runtime::services::POLICY
                },
                |_: Arc<Resources>| super::resident
            ),
            Err(Error::AlreadyRegistered { .. })
        ));
        resources.cleanup.notify_one();
        remove.await.unwrap();
    }
    register(manager);
    let replacement = manager
        .submit_with_resources_at("scoped", resources.clone())
        .await
        .unwrap();
    replacement.ready().await.unwrap();
    assert_ne!(replacement.id(), stale.id());
    stale.release().await.unwrap();
    assert_eq!(
        manager
            .resident(&manager.resident_key(replacement.id()).unwrap())
            .unwrap()
            .leases,
        1
    );
    assert!(matches!(
        manager
            .resident(&manager.resident_key(replacement.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Running { .. })
    ));
    resources.cleanup.notify_one();
    replacement.release().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_preserve_typed() {
    #[derive(Debug, thiserror::Error)]
    #[error("initialization refused: {0}")]
    struct Refused(u32);

    fn typed(error: &(dyn std::error::Error + 'static)) -> bool {
        if let Some(Fault::Application(Refused(42))) = error.downcast_ref::<Fault<Refused>>() {
            true
        } else if let Some(Error::TaskFailed { source, .. }) = error.downcast_ref::<Error>() {
            typed(source.as_ref())
        } else {
            error.source().is_some_and(typed)
        }
    }

    let runtime = assembly().runtime().await.unwrap();
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "broken".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<()>| {
                |_service: Service<Bank>| async { Err::<(), _>(Fault::Application(Refused(42))) }
            },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("broken").await.unwrap();
    let error = lease.ready().await.unwrap_err();
    assert!(typed(&error), "{error:?}");
    assert!(typed(&lease.release().await.unwrap_err()));
    for _ in 0..2 {
        assert!(typed(&runtime.stop().await.unwrap_err()));
    }
}

#[tokio::test]
async fn shutdown_signals_scope() {
    let runtime = assembly().runtime().await.unwrap();
    register(&runtime.operations);
    let a = Arc::new(Resources::default());
    let b = Arc::new(Resources::default());
    let first = runtime
        .operations
        .submit_with_resources_at("scoped", a.clone())
        .await
        .unwrap();
    let second = runtime
        .operations
        .submit_with_resources_at("scoped", b.clone())
        .await
        .unwrap();
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
}

#[tokio::test]
async fn unregister_drops_factory() {
    struct Reentrant {
        manager: OperationManager,
        dropped: Arc<AtomicUsize>,
    }
    impl Drop for Reentrant {
        fn drop(&mut self) {
            assert!(self.manager.resident_definition("unused").is_none());
            self.dropped.fetch_add(1, SeqCst);
        }
    }
    let runtime = assembly().runtime().await.unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    let resource = Arc::new(Reentrant {
        manager: runtime.operations.clone(),
        dropped: dropped.clone(),
    });
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "unused".into(),
                policy: harmos_runtime::services::POLICY,
            },
            move |_: Arc<()>| {
                let _owner = resource.clone();
                super::resident
            },
        )
        .unwrap();
    runtime
        .operations
        .unregister_resident("unused")
        .await
        .unwrap();
    assert_eq!(dropped.load(SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn resident_not_reinitialized() {
    let runtime = assembly()
        .service("one-shot", |_service| async { Ok::<(), Fault<&str>>(()) })
        .runtime()
        .await
        .unwrap();
    let key = harmos_runtime::operations::resident::Key::root("one-shot");
    runtime
        .operations
        .watch_residents()
        .wait_for(|values| {
            values
                .get(&key)
                .is_some_and(|instance| instance.health == Some(ServiceStatus::Stopped))
        })
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.submit_resident("one-shot").await,
        Err(Error::NotReady {
            domain: "retired assembly service"
        })
    ));
    assert_eq!(
        runtime.operations.resident(&key).unwrap().health,
        Some(ServiceStatus::Stopped)
    );
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn factory_panic_reported() {
    let runtime = assembly().runtime().await.unwrap();
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "factory".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |fail: Arc<bool>| {
                assert!(!*fail, "factory initialization failed");
                super::resident
            },
        )
        .unwrap();
    let lease = runtime
        .operations
        .submit_with_resources_at("factory", Arc::new(true))
        .await
        .unwrap();
    assert!(matches!(lease.ready().await, Err(Error::TaskFailed { .. })));
    assert!(matches!(
        runtime
            .operations
            .resident(&runtime.operations.resident_key(lease.id()).unwrap())
            .unwrap()
            .health,
        Some(ServiceStatus::Inactive {
            cause: harmos_runtime::services::ServiceExit::Panicked,
            ..
        })
    ));
    assert!(matches!(
        runtime.stop().await,
        Err(Error::TaskFailed { .. })
    ));
}

/* === Tests === */
