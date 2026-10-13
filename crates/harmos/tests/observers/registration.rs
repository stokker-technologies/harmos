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
    Backend {
        id,
        mode: Mode::Cooperative,
    }
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("catalog_stays_inert", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    let control = Control::new();
                    let scope = manager.scope(control.clone()).unwrap();
                    let candidate = manager.stage(&scope, [backend("a"), backend("b")]).unwrap();
                    assert!(manager.definitions().is_empty());
                    assert!(manager.definition("a").is_none());
                    assert!(matches!(
                        manager.activate("a", &scope).await,
                        Err(Error::NotReady { .. })
                    ));
                    assert!(matches!(
                        manager.register(backend("b")),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    assert_eq!(control.starts.available_permits(), 0);

                    candidate.commit().unwrap();
                    assert_eq!(manager.definitions().len(), 2);
                    assert_eq!(control.starts.available_permits(), 0);
                    let foreign = manager.scope(Control::new()).unwrap();
                    assert!(matches!(
                        manager.activate("a", &foreign).await,
                        Err(Error::InvalidInput { .. })
                    ));
                    let lease = manager.activate("a", &scope).await.unwrap();
                    reached(&control.starts).await;
                    control.finish.add_permits(1);
                    manager.stop_scope(&scope).await.unwrap();
                    lease.release().await.unwrap();
                    assert!(manager.definitions().is_empty());
                    assert!(manager.statuses().is_empty());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("candidates_drop_release", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    let scope = manager.scope(Control::new()).unwrap();
                    manager
                        .stage(&scope, std::iter::empty::<Backend>())
                        .unwrap()
                        .commit()
                        .unwrap();
                    manager.register(backend("existing")).unwrap();
                    assert!(matches!(
                        manager.stage(&scope, [backend("new"), backend("existing")]),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    manager.register(backend("new")).unwrap();
                    assert!(matches!(
                        manager.stage(&scope, [backend("duplicate"), backend("duplicate")]),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    manager.register(backend("duplicate")).unwrap();

                    let candidate = manager.stage(&scope, [backend("reserved")]).unwrap();
                    drop(candidate);
                    manager.register(backend("reserved")).unwrap();
                    manager.stop_scope(&scope).await.unwrap();
                    assert_eq!(
                        manager.definitions().len(),
                        4,
                        "scope closure preserves global definitions"
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cannot_publish_remove", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    let scope = manager.scope(Control::new()).unwrap();
                    let candidate = manager.stage(&scope, [backend("a"), backend("b")]).unwrap();
                    manager.unregister("a").await.unwrap();
                    manager.register(backend("a")).unwrap();
                    assert!(matches!(
                        candidate.commit(),
                        Err(Error::NotRegistered { .. })
                    ));
                    assert!(manager.definition("a").is_some());
                    manager.register(backend("b")).unwrap();

                    let candidate = manager.stage(&scope, [backend("closed")]).unwrap();
                    manager.stop_scope(&scope).await.unwrap();
                    manager.register(backend("closed")).unwrap();
                    assert!(matches!(candidate.commit(), Err(Error::Closing { .. })));
                    assert!(manager.definition("closed").is_some());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("teardown_retains_names", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    let control = Control::new();
                    let scope = manager.scope(control.clone()).unwrap();
                    manager
                        .stage(&scope, [backend("owned")])
                        .unwrap()
                        .commit()
                        .unwrap();
                    let lease = manager.activate("owned", &scope).await.unwrap();
                    reached(&control.starts).await;
                    phase(manager, lease.key(), ObserverPhase::Running).await;
                    let mut closing = Box::pin(manager.stop_scope(&scope));
                    tokio::select! {
                        result = &mut closing => panic!("cleanup escaped its barrier: {result:?}"),
                        _ = reached(&control.stops) => {}
                    }
                    drop(closing);
                    assert!(matches!(
                        manager.register(backend("owned")),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    control.finish.add_permits(1);
                    manager.stop_scope(&scope).await.unwrap();
                    let replacement_scope = manager.scope(Control::new()).unwrap();
                    manager
                        .stage(&replacement_scope, [backend("owned")])
                        .unwrap()
                        .commit()
                        .unwrap();
                    manager.stop_scope(&scope).await.unwrap();
                    lease.release().await.unwrap();
                    assert!(
                        manager.definition("owned").is_some(),
                        "stale cleanup preserves the new scope's definition"
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("reservations_release_backends", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = &runtime.observers;
                    let scope = manager.scope(Arc::new(())).unwrap();
                    let dropped = Arc::new(AtomicBool::new(false));
                    let backend = || {
                        ReentrantBackend(Reentrant {
                            manager: manager.clone(),
                            dropped: dropped.clone(),
                        })
                    };
                    drop(manager.stage(&scope, [backend()]).unwrap());
                    assert!(dropped.swap(false, Ordering::SeqCst));
                    manager
                        .stage(&scope, [backend()])
                        .unwrap()
                        .commit()
                        .unwrap();
                    manager.stop_scope(&scope).await.unwrap();
                    assert!(dropped.load(Ordering::SeqCst));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("catalogs_share_runtime", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let control = Control::new();
                    let resident_control = control.clone();
                    let (opened, mut scope_ready) = tokio::sync::mpsc::unbounded_channel();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .service(
                            "observer-owner",
                            move |mut context: harmos::services::Service<Bank>| {
                                let control = resident_control.clone();
                                let opened = opened.clone();
                                async move {
                                    let manager = context.observers().clone();
                                    let scope = manager.scope(control).map_err(|error| {
                                        harmos::services::Fault::Application(error.to_string())
                                    })?;
                                    manager
                                        .stage(&scope, [backend("service.watch")])
                                        .and_then(|candidate| candidate.commit())
                                        .map_err(|error| {
                                            harmos::services::Fault::Application(error.to_string())
                                        })?;
                                    let lease = manager
                                        .activate("service.watch", &scope)
                                        .await
                                        .map_err(|error| {
                                            harmos::services::Fault::Application(error.to_string())
                                        })?;
                                    opened.send((scope.clone(), lease.id())).unwrap();
                                    context.stopping().await;
                                    manager.stop_scope(&scope).await.map_err(|error| {
                                        harmos::services::Fault::Application(error.to_string())
                                    })?;
                                    lease.release().await.map_err(|error| {
                                        harmos::services::Fault::Application(error.to_string())
                                    })?;
                                    Ok::<(), harmos::services::Fault<String>>(())
                                }
                            },
                        )
                        .runtime()
                        .await
                        .unwrap();
                    let (scope, generation) =
                        tokio::time::timeout(Duration::from_secs(2), scope_ready.recv())
                            .await
                            .unwrap()
                            .unwrap();
                    let shared = runtime
                        .handle()
                        .observers
                        .activate("service.watch", &scope)
                        .await
                        .unwrap();
                    assert_eq!(shared.id(), generation);
                    phase(&runtime.observers, shared.key(), ObserverPhase::Running).await;
                    assert_eq!(runtime.observers.status(shared.key()).unwrap().leases, 2);
                    shared.release().await.unwrap();
                    control.finish.add_permits(1);
                    runtime.stop().await.unwrap();
                    assert_eq!(control.completed.load(Ordering::SeqCst), 1);
                    assert!(runtime.observers.definitions().is_empty());
                    assert_eq!(
                        runtime
                            .services
                            .status(&harmos::services::ServiceKey::root("observer-owner"))
                            .map(|item| item.status),
                        Some(harmos::services::ServiceStatus::Stopped)
                    );
                });
        });
    }
    {
        scenario_runtime::check("foreign_closed_shutdown", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let other = super::runtime().await;
                    let manager = &runtime.observers;
                    let foreign = other.observers.scope(Control::new()).unwrap();
                    assert!(matches!(
                        manager.stage(&foreign, [backend("foreign")]),
                        Err(Error::InvalidInput { .. })
                    ));
                    let closed = manager.scope(Control::new()).unwrap();
                    manager.stop_scope(&closed).await.unwrap();
                    assert!(matches!(
                        manager.stage(&closed, [backend("closed")]),
                        Err(Error::Closing { .. })
                    ));
                    let scope = manager.scope(Control::new()).unwrap();
                    let candidate = manager.stage(&scope, [backend("shutdown")]).unwrap();
                    runtime.stop().await.unwrap();
                    assert!(matches!(candidate.commit(), Err(Error::Closing { .. })));
                    assert!(manager.definitions().is_empty());
                    other.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
