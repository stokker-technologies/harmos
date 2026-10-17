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

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("cleanup_preserves_peers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    host.control.lock().unwrap().events.clear();
    host.control.lock().unwrap().fail_stop = Some("client".to_owned());
    let next = plan();
    assert!(
        matches!(session.reload_parts(next.clone(), inventories(&next), ["provider".to_owned()], &mut host).await.map_err(failure),
        Err(PluginFailure::Cleanup(failures)) if failures.len() == 1 && failures[0].artifact == "client")
    );
    assert_eq!(
        host.control.lock().unwrap().events,
        ["fence:client,provider", "stop:client"]
    );
    assert!(matches!(
        host.operations()
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::Closing {
            domain: "operation plugin"
        })
    ));
    assert!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    assert!(
        host.operations()
            .call_encoded("other.run", request_payload())
            .await
            .is_ok()
    );
    let operations = host.operations().clone();
    let cleanup = session.cleanup();
    drop(session);
    assert!(cleanup.join().await.is_err());
    assert!(matches!(cleanup.status(), PluginCleanupStatus::Failed(_)));
    assert!(
        operations
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    assert!(matches!(
        operations
            .call_encoded("other.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    host.control.lock().unwrap().fail_stop = None;
    cleanup.retry().await.unwrap();
    assert!(matches!(cleanup.status(), PluginCleanupStatus::Settled));
    let control = host.control.lock().unwrap();
    for owner in ["client", "provider", "other"] {
        assert_eq!(control.backend_drops.get(owner), Some(&1));
    }

});
        });
    }
    {
        scenario_runtime::check("cleanup_quiesces_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    host.control.lock().unwrap().block_stop = Some("client".to_owned());
    let next = plan();
    let entered = host.entered.clone();
    {
        let reload = session.reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".to_owned()],
            &mut host,
        );
        tokio::pin!(reload);
        tokio::select! {
            result = &mut reload => panic!("cleanup completed before release: {result:?}"),
            () = entered.notified() => {}
        }
    }
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("client"), Some(&1));
        assert!(!control.future_drops.contains_key("client"));
        assert!(!control.backend_drops.contains_key("client"));
    }
    assert!(host.control.lock().unwrap().callers.contains_key("client"));
    let retained = session.status("client").unwrap();
    assert_eq!(retained.state, PluginState::Stopping);
    assert!(retained.generation.is_some());
    assert!(retained.active_definition.is_some());
    assert_eq!(
        host.control.lock().unwrap().stop_calls.get("client"),
        Some(&1)
    );
    assert!(
        !host
            .control
            .lock()
            .unwrap()
            .backend_drops
            .contains_key("client")
    );
    assert!(matches!(
        host.operations()
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::Closing {
            domain: "operation plugin"
        })
    ));
    assert!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    host.control.lock().unwrap().block_stop = None;
    host.release.notify_waiters();
    session
        .reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".to_owned()],
            &mut host,
        )
        .await
        .unwrap();
    assert_eq!(session.ready().count(), 3);
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("client"), Some(&1));
        assert_eq!(control.future_drops.get("client"), Some(&1));
        assert_eq!(control.backend_drops.get("client"), Some(&1));
    }
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("failure_retains_causes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host {
                        fail_start: Some("client".into()),
                        control: Arc::new(Mutex::new(Control {
                            fail_stop: Some("client".into()),
                            ..Control::default()
                        })),
                        ..Host::new().await
                    };
                    let error = manager.initialize(&mut host).await.unwrap_err();
                    let PluginFailure::Start {
                        artifact,
                        source,
                        cleanup,
                    } = failure(error)
                    else {
                        panic!("expected initialization failure with cleanup");
                    };
                    assert_eq!(artifact, "client");
                    assert!(matches!(
                        source,
                        Error::Backend {
                            operation: "initialize plugin",
                            kind: harmos_runtime::BackendFailure::Permanent,
                            ..
                        }
                    ));
                    assert_eq!(cleanup.len(), 1);
                    assert_eq!(cleanup[0].artifact, "client");
                    let Error::TaskFailed { source, .. } = &cleanup[0].error else {
                        panic!("expected retained cleanup failure");
                    };
                    let Error::Backend {
                        operation, source, ..
                    } = source.downcast_ref::<Error>().unwrap()
                    else {
                        panic!("expected original physical cleanup source");
                    };
                    assert_eq!(*operation, "shutdown plugin");
                    assert!(source.downcast_ref::<std::io::Error>().is_some());
                    let retained = manager.status("client").unwrap();
                    assert_eq!(retained.state, PluginState::Failed);
                    assert!(retained.generation.is_some());
                    assert!(retained.active_definition.is_some());
                    assert_eq!(
                        host.control.lock().unwrap().stop_calls.get("client"),
                        Some(&1)
                    );
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .backend_drops
                            .contains_key("client")
                    );
                    assert!(host.control.lock().unwrap().callers.contains_key("client"));
                    host.control.lock().unwrap().fail_stop = None;
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cleanup_retains_dependencies", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    let provider = manager.status("provider").unwrap().generation;
                    let client = manager.status("client").unwrap().generation;
                    let caller = host.control.lock().unwrap().callers["client"].clone();
                    host.control.lock().unwrap().events.clear();
                    host.control.lock().unwrap().fail_stop = Some("client".into());

                    let PluginFailure::Cleanup(failures) =
                        failure(manager.shutdown(&mut host).await.unwrap_err())
                    else {
                        panic!("expected retained cleanup failure");
                    };
                    assert_eq!(failures.len(), 1);
                    assert_eq!(failures[0].artifact, "client");
                    assert_eq!(manager.status("provider").unwrap().generation, provider);
                    assert_eq!(manager.status("client").unwrap().generation, client);
                    assert!(
                        manager
                            .status("provider")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(
                        !manager
                            .status("other")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:provider")
                    );
                    assert!(matches!(
                        host.call(&caller, "provider.run", request_payload()).await,
                        Err(Error::OperationCaller {
                            source: DispatchError::Unavailable
                        })
                    ));
                    assert!(
                        host.operations()
                            .call_encoded("provider.run", request_payload())
                            .await
                            .is_ok()
                    );

                    host.control.lock().unwrap().fail_stop = None;
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(manager.statuses().all(|status| status.generation.is_none()));
                    assert!(
                        host.control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:provider")
                    );
                });
        });
    }
    {
        scenario_runtime::check("cleanup_spares_peers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut manager = session();
    let mut host = Host::new().await;
    manager.initialize(&mut host).await.unwrap();
    host.control.lock().unwrap().block_stop = Some("client".into());
    let changed = host.entered.clone();
    let mut events = manager.subscribe();
    let observed = host.control.clone();
    {
        let shutdown = manager.shutdown(&mut host);
        tokio::pin!(shutdown);
        loop {
            let notified = changed.notified();
            let independent_settled = {
                let control = observed.lock().unwrap();
                control.backend_drops.get("other") == Some(&1)
                    && control.stop_calls.get("client") == Some(&1)
            };
            if independent_settled {
                break;
            }
            tokio::select! {
                result = &mut shutdown => panic!("shutdown settled before release: {result:?}"),
                () = notified => {}
            }
        }
    }
    assert_eq!(
        manager.status("provider").unwrap().state,
        PluginState::Ready
    );
    {
        let control = observed.lock().unwrap();
        assert!(!control.stop_calls.contains_key("provider"));
        assert!(!control.backend_drops.contains_key("client"));
        assert!(!control.future_drops.contains_key("client"));
    }
    host.control.lock().unwrap().block_stop = None;
    host.release.notify_waiters();
    assert!(matches!(
        manager.plan_reload(PluginDefinitions::new(manager.definitions().cloned(), []).unwrap()),
        Err(Error::NotReady {
            domain: "plugin retirement"
        })
    ));
    tokio::time::timeout(Duration::from_secs(1), async {
        while let Some(event) = events.next().await {
            if matches!(event, PluginEvent::Changed {
                id, kind: PluginEventKind::Stopped, ..
            } if id == "provider")
            {
                return;
            }
        }
        panic!("the retained driver owns its event publisher");
    })
    .await
    .expect("the owned retirement driver advances after its waiter is dropped");
    let status = manager.status("provider").unwrap();
    assert_eq!(status.state, PluginState::Registered);
    assert!(status.generation.is_none());
    manager.shutdown(&mut host).await.unwrap();
    assert!(manager.statuses().all(|status| status.generation.is_none()));
    let control = observed.lock().unwrap();
    for owner in ["client", "provider", "other"] {
        assert_eq!(control.stop_calls.get(owner), Some(&1));
        assert_eq!(control.backend_drops.get(owner), Some(&1));
    }

});
        });
    }
    {
        scenario_runtime::check("drop_retains_retirement", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    host.control.lock().unwrap().block_stop = Some("client".into());
                    let entered = host.entered.clone();
                    let mut events = manager.subscribe();
                    {
                        let shutdown = manager.shutdown(&mut host);
                        tokio::pin!(shutdown);
                        tokio::select! {
                            result = &mut shutdown => panic!("client cleanup is held: {result:?}"),
                            () = entered.notified() => {}
                        }
                    }
                    drop(manager);
                    host.operations()
                        .call_encoded("provider.run", request_payload())
                        .await
                        .unwrap();
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .backend_drops
                            .contains_key("provider")
                    );
                    host.control.lock().unwrap().block_stop = None;
                    host.release.notify_waiters();
                    tokio::time::timeout(Duration::from_secs(1), async {
                        let mut provider_stopped = false;
                        while let Some(event) = events.next().await {
                            if matches!(event, PluginEvent::Changed {
                id, kind: PluginEventKind::Stopped, ..
            } if id == "provider")
                            {
                                provider_stopped = true;
                            }
                        }
                        assert!(
                            provider_stopped,
                            "provider settlement precedes driver release"
                        );
                    })
                    .await
                    .unwrap();
                    let control = host.control.lock().unwrap();
                    for owner in ["client", "provider", "other"] {
                        assert_eq!(control.stop_calls.get(owner), Some(&1));
                        assert_eq!(control.backend_drops.get(owner), Some(&1));
                    }
                });
        });
    }
    {
        scenario_runtime::check("drop_retains_retry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    {
                        let mut control = host.control.lock().unwrap();
                        control.block_stop = Some("client".into());
                        control.fail_stop = Some("client".into());
                    }
                    let entered = host.entered.clone();
                    let cleanup = manager.cleanup();
                    {
                        let plan = manager.plan_reload(candidate(["client".into()])).unwrap();
                        let reload = manager.apply_reload(plan, &mut host);
                        tokio::pin!(reload);
                        tokio::select! {
                            result = &mut reload => panic!("client cleanup is held: {result:?}"),
                            () = entered.notified() => {}
                        }
                    }
                    drop(manager);
                    host.control.lock().unwrap().block_stop = None;
                    host.release.notify_waiters();
                    assert!(cleanup.join().await.is_err());
                    assert!(cleanup.join().await.is_err());
                    {
                        let control = host.control.lock().unwrap();
                        assert_eq!(control.stop_calls.get("client"), Some(&1));
                        assert_eq!(control.backend_drops.get("client"), None);
                        assert_eq!(control.stop_calls.get("provider"), None);
                        assert_eq!(control.backend_drops.get("provider"), None);
                        assert_eq!(control.backend_drops.get("other"), Some(&1));
                    }
                    host.operations()
                        .call_encoded("provider.run", request_payload())
                        .await
                        .unwrap();
                    host.control.lock().unwrap().fail_stop = None;
                    cleanup.retry().await.unwrap();
                    let control = host.control.lock().unwrap();
                    assert_eq!(control.stop_calls.get("client"), Some(&2));
                    for owner in ["client", "provider", "other"] {
                        assert_eq!(control.backend_drops.get(owner), Some(&1));
                    }
                    for owner in ["provider", "other"] {
                        assert_eq!(control.stop_calls.get(owner), Some(&1));
                    }
                });
        });
    }
    {
        scenario_runtime::check("drop_keeps_providers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    host.control.lock().unwrap().block_stop = Some("client".into());
                    let entered = host.entered.clone();
                    let mut events = manager.subscribe();
                    {
                        let plan = manager.plan_reload(candidate(["client".into()])).unwrap();
                        assert_eq!(plan.affected(), ["client"]);
                        let shutdown = manager.apply_reload(plan, &mut host);
                        tokio::pin!(shutdown);
                        tokio::select! {
                            result = &mut shutdown => panic!("client cleanup is held: {result:?}"),
                            () = entered.notified() => {}
                        }
                    }
                    drop(manager);
                    host.operations()
                        .call_encoded("provider.run", request_payload())
                        .await
                        .unwrap();
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .backend_drops
                            .contains_key("provider")
                    );
                    host.control.lock().unwrap().block_stop = None;
                    host.release.notify_waiters();
                    tokio::time::timeout(Duration::from_secs(1), async {
                        let mut provider_stopped = false;
                        while let Some(event) = events.next().await {
                            if matches!(event, PluginEvent::Changed {
                id, kind: PluginEventKind::Stopped, ..
            } if id == "provider")
                            {
                                provider_stopped = true;
                            }
                        }
                        assert!(
                            provider_stopped,
                            "provider settlement precedes driver release"
                        );
                    })
                    .await
                    .unwrap();
                    let control = host.control.lock().unwrap();
                    for owner in ["client", "provider", "other"] {
                        assert_eq!(control.stop_calls.get(owner), Some(&1));
                        assert_eq!(control.backend_drops.get(owner), Some(&1));
                    }
                });
        });
    }
    {
        scenario_runtime::check("retry_publishes_success", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    host.control.lock().unwrap().fail_stop = Some("client".into());
                    assert!(manager.shutdown(&mut host).await.is_err());
                    {
                        let mut control = host.control.lock().unwrap();
                        control.fail_stop = None;
                        control.block_stop = Some("client".into());
                    }
                    let mut events = manager.subscribe();
                    {
                        let shutdown = manager.shutdown(&mut host);
                        tokio::pin!(shutdown);
                        loop {
                            tokio::select! {
                                result = &mut shutdown => panic!("cleanup is held: {result:?}"),
                                event = events.next() => {
                                    if matches!(event, Some(PluginEvent::Changed {
                                        id, kind: PluginEventKind::Stopping, ..
                                    }) if id == "client") { break; }
                                }
                            }
                        }
                    }
                    host.control.lock().unwrap().block_stop = None;
                    host.release.notify_waiters();
                    tokio::time::timeout(Duration::from_secs(1), async {
                        while let Some(event) = events.next().await {
                            if matches!(event, PluginEvent::Changed {
                id, kind: PluginEventKind::Stopped, ..
            } if id == "client")
                            {
                                return;
                            }
                        }
                        panic!("retirement retains event publication");
                    })
                    .await
                    .unwrap();
                    let status = manager.status("client").unwrap();
                    assert_eq!(status.state, PluginState::Registered);
                    assert!(status.generation.is_none());
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(manager.statuses().all(|status| status.generation.is_none()));
                });
        });
    }
    {
        scenario_runtime::check("shutdown_reuses_adapter", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    manager.shutdown(&mut Shutdown).await.unwrap();
                    assert!(manager.statuses().all(|status| status.generation.is_none()));
                    let control = host.control.lock().unwrap();
                    for owner in ["client", "provider", "other"] {
                        assert_eq!(control.stop_calls.get(owner), Some(&1));
                        assert_eq!(control.backend_drops.get(owner), Some(&1));
                    }
                });
        });
    }
    {
        scenario_runtime::check("panic_retains_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    let generation = manager.status("client").unwrap().generation;
                    host.control.lock().unwrap().panic_stop = Some("client".into());

                    let PluginFailure::Cleanup(failures) =
                        failure(manager.shutdown(&mut host).await.unwrap_err())
                    else {
                        panic!("expected retained cleanup panic");
                    };
                    assert_eq!(failures.len(), 1);
                    assert_eq!(failures[0].artifact, "client");
                    assert_eq!(manager.status("client").unwrap().generation, generation);
                    assert_eq!(manager.status("client").unwrap().state, PluginState::Failed);
                    assert!(
                        manager
                            .status("provider")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    {
                        let control = host.control.lock().unwrap();
                        assert_eq!(control.stop_calls.get("client"), Some(&1));
                        assert_eq!(control.future_drops.get("client"), Some(&1));
                        assert!(!control.backend_drops.contains_key("client"));
                        assert!(!control.stop_calls.contains_key("provider"));
                    }

                    host.control.lock().unwrap().panic_stop = None;
                    manager.shutdown(&mut host).await.unwrap();
                    assert!(manager.statuses().all(|status| status.generation.is_none()));
                    let control = host.control.lock().unwrap();
                    assert_eq!(control.stop_calls.get("client"), Some(&2));
                    assert_eq!(control.backend_drops.get("client"), Some(&1));
                    assert_eq!(control.backend_drops.get("provider"), Some(&1));
                });
        });
    }
}
