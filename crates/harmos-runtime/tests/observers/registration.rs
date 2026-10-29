/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

fn backend(id: &'static str) -> Backend {
    Backend { id, mode: Mode::Cooperative }
}

#[tokio::test]
async fn catalog_stays_inert() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let control = runtime.operations.context().resource::<Control>().unwrap();
    let candidate = manager.stage_observers([backend("a"), backend("b")]).unwrap();
    assert!(observers(manager).is_empty());
    assert!(manager.resident_definition("a").is_none());
    assert!(matches!(manager.submit_resident("a").await, Err(Error::NotReady { .. })));
    assert!(matches!(manager.register_observer(backend("b")), Err(Error::AlreadyRegistered { .. })));
    assert_eq!(control.starts.available_permits(), 0);

    candidate.commit().unwrap();
    assert_eq!(observers(manager).len(), 2);
    assert_eq!(control.starts.available_permits(), 0);
    let lease = manager.submit_resident("a").await.unwrap();
    reached(&control.starts).await;
    control.finish.add_permits(1);
    manager.unregister_resident("a").await.unwrap();
    lease.release().await.unwrap();
    assert_eq!(observers(manager).len(), 1);
    assert_eq!(observer_instances(manager), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn candidates_drop_release() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    manager.stage_observers(std::iter::empty::<Backend>()).unwrap().commit().unwrap();
    manager.register_observer(backend("existing")).unwrap();
    assert!(matches!(manager.stage_observers([backend("new"), backend("existing")]), Err(Error::AlreadyRegistered { .. })));
    manager.register_observer(backend("new")).unwrap();
    assert!(matches!(manager.stage_observers([backend("duplicate"), backend("duplicate")]), Err(Error::AlreadyRegistered { .. })));
    manager.register_observer(backend("duplicate")).unwrap();

    let candidate = manager.stage_observers([backend("reserved")]).unwrap();
    drop(candidate);
    manager.register_observer(backend("reserved")).unwrap();
    assert_eq!(observers(manager).len(), 4);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cannot_publish_remove() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let candidate = manager.stage_observers([backend("a"), backend("b")]).unwrap();
    manager.unregister_resident("a").await.unwrap();
    manager.register_observer(backend("a")).unwrap();
    assert!(matches!(candidate.commit(), Err(Error::NotRegistered { .. })));
    assert!(manager.resident_definition("a").is_some());
    manager.register_observer(backend("b")).unwrap();

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn teardown_retains_names() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let control = runtime.operations.context().resource::<Control>().unwrap();
    manager.stage_observers([backend("owned")]).unwrap().commit().unwrap();
    let lease = manager.submit_resident("owned").await.unwrap();
    reached(&control.starts).await;
    phase(manager, &key(manager, &lease), RUNNING).await;
    let mut closing = Box::pin(manager.unregister_resident("owned"));
    tokio::select! {
        result = &mut closing => panic!("cleanup escaped its barrier: {result:?}"),
        _ = reached(&control.stops) => {}
    }
    drop(closing);
    assert!(matches!(manager.register_observer(backend("owned")), Err(Error::AlreadyRegistered { .. })));
    control.finish.add_permits(1);
    manager.unregister_resident("owned").await.unwrap();
    manager.stage_observers([backend("owned")]).unwrap().commit().unwrap();
    lease.release().await.unwrap();
    assert!(manager.resident_definition("owned").is_some(), "old leases preserve the replacement definition");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn reservations_release_backends() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let dropped = Arc::new(AtomicBool::new(false));
    let backend = || {
        ReentrantBackend(Reentrant {
            manager: manager.clone(),
            dropped: dropped.clone(),
        })
    };
    drop(manager.stage_observers([backend()]).unwrap());
    assert!(dropped.swap(false, Ordering::SeqCst));
    manager.stage_observers([backend()]).unwrap().commit().unwrap();
    manager.unregister_resident("retained").await.unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn catalogs_share_runtime() {
    let control = Control::new();
    let (opened, mut scope_ready) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.resource::<Control>(control.clone());
        setup.settings().register(catalog()).service(
            "observer-owner",
            move |mut context: harmos_runtime::services::ServiceContext<Bank>| {
                let opened = opened.clone();
                async move {
                    let manager = context.host();
                    manager
                        .stage_observers([backend("service.watch")])
                        .and_then(|candidate| candidate.commit())
                        .map_err(std::io::Error::other)?;
                    let lease = manager.submit_resident("service.watch").await.map_err(std::io::Error::other)?;
                    opened.send(lease.id()).unwrap();
                    context.stopping().await;
                    manager.unregister_resident("service.watch").await.map_err(std::io::Error::other)?;
                    lease.release().await.map_err(std::io::Error::other)?;
                    Ok::<(), std::io::Error>(())
                }
            },
        );
    })
    .await
    .unwrap();
    let generation = scope_ready.recv().await.unwrap();
    let shared = runtime.handle().operations.submit_resident("service.watch").await.unwrap();
    assert_eq!(shared.id(), generation);
    let shared_key = key(&runtime.operations, &shared);
    phase(&runtime.operations, &shared_key, RUNNING).await;
    assert_eq!(leases(&runtime.operations, &shared_key), 2);
    shared.release().await.unwrap();
    control.finish.add_permits(1);
    runtime.shutdown().await.unwrap();
    assert_eq!(control.completed.load(Ordering::SeqCst), 1);
    assert!(observers(&runtime.operations).is_empty());
    assert_eq!(
        runtime.operations.resident(&Key::root("observer-owner")).and_then(|item| item.health),
        Some(harmos_runtime::services::ServiceStatus::Stopped)
    );
}

#[tokio::test]
async fn foreign_closed_shutdown() {
    let runtime = runtime().await;
    let other = super::runtime().await;
    let manager = &runtime.operations;
    other.operations.stage_observers([backend("foreign")]).unwrap().commit().unwrap();
    assert!(manager.resident_definition("foreign").is_none());
    assert!(matches!(manager.submit_resident("foreign").await, Err(Error::NotRegistered { .. })));

    let candidate = manager.stage_observers([backend("shutdown")]).unwrap();
    runtime.shutdown().await.unwrap();
    assert!(matches!(candidate.commit(), Err(Error::Closing { .. })));
    assert!(observers(manager).is_empty());
    other.shutdown().await.unwrap();
}
