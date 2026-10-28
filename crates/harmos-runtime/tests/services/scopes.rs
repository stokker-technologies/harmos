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
use super::{Bank, Ledger, Runtime, assembly};
use harmos_runtime::operations::OperationManager;
use harmos_runtime::services::ServiceDefinition;
use harmos_runtime::{Error, services::ServiceContext, services::ServiceStatus};

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

fn register(manager: &OperationManager<<Bank as harmos_runtime::Application>::Lanes>, resources: Arc<Resources>) {
    manager
        .register_service(
            ServiceDefinition {
                id: "scoped".into(),
                policy: harmos_runtime::services::POLICY,
            },
            move |_: Arc<harmos_sdk::ExecutionContext>| {
                let resources = resources.clone();
                move |mut service: ServiceContext<Bank>| {
                    let resources = resources.clone();
                    async move {
                        resources.started.fetch_add(1, SeqCst);
                        service.ready();
                        service.stopping().await;
                        resources.stopping.notify_one();
                        resources.cleanup.notified().await;
                        resources.finished.fetch_add(1, SeqCst);
                        Ok::<(), std::io::Error>(())
                    }
                }
            },
        )
        .unwrap();
}

#[tokio::test]
async fn control_retains_join() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    let manager = &runtime.operations;
    let resources = Arc::new(Resources::default());
    register(manager, resources.clone());
    let first = manager.submit_resident("scoped").await.unwrap();
    let second = manager.submit_resident("scoped").await.unwrap();
    first.ready().await.unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(manager.resident_key(first.id()), manager.resident_key(second.id()));
    assert_eq!(manager.resident(&manager.resident_key(second.id()).unwrap()).unwrap().leases, 2);

    first.cancel();
    first.cancel();
    assert!(matches!(manager.resident(&manager.resident_key(second.id()).unwrap()).unwrap().health, Some(ServiceStatus::Stopping)));
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
    (first.settle()).await.unwrap();
    first.settle().await.unwrap();
    assert_eq!(resources.finished.load(SeqCst), 1);
    assert!(first.status().settled_at().is_some());

    let replacement = manager.submit_resident("scoped").await.unwrap();
    replacement.ready().await.unwrap();
    assert_ne!(replacement.id(), first.id());
    first.cancel();
    first.settle().await.unwrap();
    first.release().await.unwrap();
    second.release().await.unwrap();
    assert_eq!(manager.resident(&manager.resident_key(replacement.id()).unwrap()).unwrap().leases, 1);
    assert!(matches!(
        manager.resident(&manager.resident_key(replacement.id()).unwrap()).unwrap().health,
        Some(ServiceStatus::Running { .. })
    ));
    resources.cleanup.notify_one();
    replacement.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn final_release_joins() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    let manager = &runtime.operations;
    let resources = Arc::new(Resources::default());
    register(manager, resources.clone());
    let (first, second) = tokio::join!(manager.submit_resident("scoped"), manager.submit_resident("scoped"));
    let first = first.unwrap();
    let second = second.unwrap();
    first.ready().await.unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(manager.resident(&manager.resident_key(first.id()).unwrap()).unwrap().instance, first.id());
    assert_eq!(resources.started.load(SeqCst), 1);
    assert_eq!(manager.resident(&manager.resident_key(first.id()).unwrap()).unwrap().leases, 2);
    first.release().await.unwrap();
    assert_eq!(manager.resident(&manager.resident_key(second.id()).unwrap()).unwrap().leases, 1);
    assert!(matches!(manager.resident(&manager.resident_key(second.id()).unwrap()).unwrap().health, Some(ServiceStatus::Running { .. })));

    let release = second.release();
    tokio::pin!(release);
    tokio::select! {
        result = &mut release => panic!("cleanup must be joined: {result:?}"),
        _ = resources.stopping.notified() => {}
    }
    resources.cleanup.notify_one();
    release.await.unwrap();
    assert_eq!(resources.finished.load(SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn dropped_lease_cleanup() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    let manager = &runtime.operations;
    let resources = Arc::new(Resources::default());
    register(manager, resources.clone());
    let first = manager.submit_resident("scoped").await.unwrap();
    first.ready().await.unwrap();
    let generation = first.id();
    drop(first);
    resources.stopping.notified().await;
    let next = manager.submit_resident("scoped");
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn unregister_reserves_name() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    let manager = &runtime.operations;
    let resources = Arc::new(Resources::default());
    register(manager, resources.clone());
    let stale = manager.submit_resident("scoped").await.unwrap();
    stale.ready().await.unwrap();
    {
        let remove = manager.unregister_resident("scoped");
        tokio::pin!(remove);
        tokio::select! {
            _ = &mut remove => panic!("cleanup is gated"),
            _ = resources.stopping.notified() => {}
        }
        assert!(matches!(
            manager.submit_resident("scoped").await,
            Err(Error::Closing { domain })
                if domain.ends_with("definition")
        ));
        assert!(matches!(
            manager.register_service(
                ServiceDefinition {
                    id: "scoped".into(),
                    policy: harmos_runtime::services::POLICY
                },
                |_: Arc<harmos_sdk::ExecutionContext>| super::resident
            ),
            Err(Error::AlreadyRegistered { .. })
        ));
        resources.cleanup.notify_one();
        remove.await.unwrap();
    }
    register(manager, resources.clone());
    let replacement = manager.submit_resident("scoped").await.unwrap();
    replacement.ready().await.unwrap();
    assert_ne!(replacement.id(), stale.id());
    stale.release().await.unwrap();
    assert_eq!(manager.resident(&manager.resident_key(replacement.id()).unwrap()).unwrap().leases, 1);
    assert!(matches!(
        manager.resident(&manager.resident_key(replacement.id()).unwrap()).unwrap().health,
        Some(ServiceStatus::Running { .. })
    ));
    resources.cleanup.notify_one();
    replacement.release().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_preserve_typed() {
    #[derive(Debug, thiserror::Error)]
    #[error("initialization refused: {0}")]
    struct Refused(u32);

    fn typed(error: &(dyn std::error::Error + 'static)) -> bool {
        if let Some(Refused(42)) = error.downcast_ref::<Refused>() {
            true
        } else if let Some(Error::TaskFailed { source, .. }) = error.downcast_ref::<Error>() {
            typed(source.as_ref())
        } else {
            error.source().is_some_and(typed)
        }
    }

    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "broken".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<harmos_sdk::ExecutionContext>| |_service: ServiceContext<Bank>| async { Err::<(), _>(Refused(42)) },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("broken").await.unwrap();
    let error = lease.ready().await.unwrap_err();
    assert!(typed(&error), "{error:?}");
    assert!(typed(&lease.release().await.unwrap_err()));
    for _ in 0..2 {
        assert!(typed(&runtime.shutdown().await.unwrap_err()));
    }
}

#[tokio::test]
async fn unregister_drops_factory() {
    struct Reentrant {
        manager: OperationManager<<Bank as harmos_runtime::Application>::Lanes>,
        dropped: Arc<AtomicUsize>,
    }
    impl Drop for Reentrant {
        fn drop(&mut self) {
            assert!(self.manager.resident_definition("unused").is_none());
            self.dropped.fetch_add(1, SeqCst);
        }
    }
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
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
            move |_: Arc<harmos_sdk::ExecutionContext>| {
                let _owner = resource.clone();
                super::resident
            },
        )
        .unwrap();
    runtime.operations.unregister_resident("unused").await.unwrap();
    assert_eq!(dropped.load(SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn resident_not_reinitialized() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("one-shot", |_service| async { Ok::<(), std::io::Error>(()) });
    })
    .await
    .unwrap();
    let key = harmos_runtime::operations::resident::Key::root("one-shot");
    runtime
        .operations
        .watch_residents()
        .wait_for(|values| values.get(&key).is_some_and(|instance| instance.health == Some(ServiceStatus::Stopped)))
        .await
        .unwrap();
    assert!(matches!(runtime.operations.submit_resident("one-shot").await, Err(Error::NotReady { domain: "retired assembly service" })));
    assert_eq!(runtime.operations.resident(&key).unwrap().health, Some(ServiceStatus::Stopped));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn factory_panic_reported() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_service(
            ServiceDefinition {
                id: "factory".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<harmos_sdk::ExecutionContext>| {
                assert!(std::hint::black_box(false), "factory initialization failed");
                super::resident
            },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("factory").await.unwrap();
    assert!(matches!(lease.ready().await, Err(Error::TaskFailed { .. })));
    assert!(matches!(
        runtime.operations.resident(&runtime.operations.resident_key(lease.id()).unwrap()).unwrap().health,
        Some(ServiceStatus::Inactive {
            cause: harmos_runtime::services::ServiceExit::Panicked,
            ..
        })
    ));
    assert!(matches!(runtime.shutdown().await, Err(Error::TaskFailed { .. })));
}

/* === Tests === */
