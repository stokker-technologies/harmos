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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn backend(id: &'static str) -> Backend {
    Backend { id, mode: Mode::Cooperative }
}

#[tokio::test]
async fn catalog_stays_inert() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let control = Control::new();
    let candidate = manager.stage_observers(&control, [backend("a"), backend("b")]).unwrap();
    assert!(observers(manager).is_empty());
    assert!(manager.resident_definition("a").is_none());
    assert!(matches!(manager.submit_with_resources_at("a", control.clone()).await, Err(Error::NotReady { .. })));
    assert!(matches!(manager.register_observer(backend("b")), Err(Error::AlreadyRegistered { .. })));
    assert_eq!(control.starts.available_permits(), 0);

    candidate.commit().unwrap();
    assert_eq!(observers(manager).len(), 2);
    assert_eq!(control.starts.available_permits(), 0);
    assert!(matches!(manager.submit_with_resources_at("a", Control::new()).await, Err(Error::InvalidInput { .. })));
    let lease = manager.submit_with_resources_at("a", control.clone()).await.unwrap();
    reached(&control.starts).await;
    control.finish.add_permits(1);
    manager.retire_resources(&control).await.unwrap();
    lease.release().await.unwrap();
    assert!(observers(manager).is_empty());
    assert_eq!(observer_instances(manager), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn candidates_drop_release() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let resources = Control::new();
    manager.stage_observers(&resources, std::iter::empty::<Backend>()).unwrap().commit().unwrap();
    manager.register_observer(backend("existing")).unwrap();
    assert!(matches!(manager.stage_observers(&resources, [backend("new"), backend("existing")]), Err(Error::AlreadyRegistered { .. })));
    manager.register_observer(backend("new")).unwrap();
    assert!(matches!(
        manager.stage_observers(&resources, [backend("duplicate"), backend("duplicate")]),
        Err(Error::AlreadyRegistered { .. })
    ));
    manager.register_observer(backend("duplicate")).unwrap();

    let candidate = manager.stage_observers(&resources, [backend("reserved")]).unwrap();
    drop(candidate);
    manager.register_observer(backend("reserved")).unwrap();
    manager.retire_resources(&resources).await.unwrap();
    assert_eq!(observers(manager).len(), 4, "scope closure preserves global definitions");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cannot_publish_remove() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let resources = Control::new();
    let candidate = manager.stage_observers(&resources, [backend("a"), backend("b")]).unwrap();
    manager.unregister_resident("a").await.unwrap();
    manager.register_observer(backend("a")).unwrap();
    assert!(matches!(candidate.commit(), Err(Error::NotRegistered { .. })));
    assert!(manager.resident_definition("a").is_some());
    manager.register_observer(backend("b")).unwrap();

    let candidate = manager.stage_observers(&resources, [backend("closed")]).unwrap();
    manager.retire_resources(&resources).await.unwrap();
    manager.register_observer(backend("closed")).unwrap();
    assert!(matches!(candidate.commit(), Err(Error::Closing { .. })));
    assert!(manager.resident_definition("closed").is_some());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn teardown_retains_names() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let control = Control::new();
    manager.stage_observers(&control, [backend("owned")]).unwrap().commit().unwrap();
    let lease = manager.submit_with_resources_at("owned", control.clone()).await.unwrap();
    reached(&control.starts).await;
    phase(manager, &key(manager, &lease), RUNNING).await;
    let mut closing = Box::pin(manager.retire_resources(&control));
    tokio::select! {
        result = &mut closing => panic!("cleanup escaped its barrier: {result:?}"),
        _ = reached(&control.stops) => {}
    }
    drop(closing);
    assert!(matches!(manager.register_observer(backend("owned")), Err(Error::AlreadyRegistered { .. })));
    control.finish.add_permits(1);
    manager.retire_resources(&control).await.unwrap();
    let replacement = Control::new();
    manager.stage_observers(&replacement, [backend("owned")]).unwrap().commit().unwrap();
    manager.retire_resources(&control).await.unwrap();
    lease.release().await.unwrap();
    assert!(manager.resident_definition("owned").is_some(), "stale cleanup preserves the new scope's definition");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn reservations_release_backends() {
    let runtime = runtime().await;
    let manager = &runtime.operations;
    let resources = Arc::new(());
    let dropped = Arc::new(AtomicBool::new(false));
    let backend = || {
        ReentrantBackend(Reentrant {
            manager: manager.clone(),
            dropped: dropped.clone(),
        })
    };
    drop(manager.stage_observers(&resources, [backend()]).unwrap());
    assert!(dropped.swap(false, Ordering::SeqCst));
    manager.stage_observers(&resources, [backend()]).unwrap().commit().unwrap();
    manager.retire_resources(&resources).await.unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn catalogs_share_runtime() {
    let control = Control::new();
    let resident_control = control.clone();
    let (opened, mut scope_ready) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .service("observer-owner", move |mut context: harmos_runtime::services::ServiceContext<Bank>| {
            let control = resident_control.clone();
            let opened = opened.clone();
            async move {
                let manager = context.host();
                manager
                    .stage_observers(&control, [backend("service.watch")])
                    .and_then(|candidate| candidate.commit())
                    .map_err(std::io::Error::other)?;
                let lease = manager.submit_with_resources_at("service.watch", control.clone()).await.map_err(std::io::Error::other)?;
                opened.send(lease.id()).unwrap();
                context.stopping().await;
                manager.retire_resources(&control).await.map_err(std::io::Error::other)?;
                lease.release().await.map_err(std::io::Error::other)?;
                Ok::<(), std::io::Error>(())
            }
        })
        .runtime()
        .await
        .unwrap();
    let generation = scope_ready.recv().await.unwrap();
    let shared = runtime.handle().operations.submit_with_resources_at("service.watch", control.clone()).await.unwrap();
    assert_eq!(shared.id(), generation);
    let shared_key = key(&runtime.operations, &shared);
    phase(&runtime.operations, &shared_key, RUNNING).await;
    assert_eq!(leases(&runtime.operations, &shared_key), 2);
    shared.release().await.unwrap();
    control.finish.add_permits(1);
    runtime.stop().await.unwrap();
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
    // Resources staged in another runtime stay foreign to this one.
    let shared = Control::new();
    other.operations.stage_observers(&shared, [backend("foreign")]).unwrap().commit().unwrap();
    assert!(manager.resident_definition("foreign").is_none());
    assert!(matches!(manager.submit_with_resources_at("foreign", shared.clone()).await, Err(Error::NotRegistered { .. })));

    // Resources whose retirement is in flight refuse new catalogs.
    let closed = Control::new();
    manager.stage_observers(&closed, [backend("closing")]).unwrap().commit().unwrap();
    let lease = manager.submit_with_resources_at("closing", closed.clone()).await.unwrap();
    phase(manager, &key(manager, &lease), RUNNING).await;
    pending::parked(std::pin::pin!(manager.retire_resources(&closed))).await;
    assert!(matches!(manager.stage_observers(&closed, [backend("closed")]), Err(Error::Closing { .. })));
    closed.finish.add_permits(1);
    manager.retire_resources(&closed).await.unwrap();
    lease.release().await.unwrap();

    let resources = Control::new();
    let candidate = manager.stage_observers(&resources, [backend("shutdown")]).unwrap();
    runtime.stop().await.unwrap();
    assert!(matches!(candidate.commit(), Err(Error::Closing { .. })));
    assert!(observers(manager).is_empty());
    other.stop().await.unwrap();
}

/* === Tests === */
