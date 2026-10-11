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
        scenario_runtime::check("activation_failure_cleans", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    host.control.lock().unwrap().events.clear();
    host.fail_start = Some("client".to_owned());
    let next = plan();
    assert!(
        matches!(session.reload_parts(next.clone(), inventories(&next), ["provider".to_owned()], &mut host).await.map_err(failure),
        Err(PluginFailure::Start { artifact, cleanup, .. }) if artifact == "client" && cleanup.is_empty())
    );
    assert_eq!(session.ready().collect::<Vec<_>>(), ["other"]);
    assert_eq!(
        host.control.lock().unwrap().events,
        [
            "fence:client,provider",
            "stop:client",
            "stop:provider",
            "start:provider",
            "start:client",
            "stop:client",
            "stop:provider"
        ]
    );
    assert!(matches!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    host.fail_start = None;
    session.initialize(&mut host).await.unwrap();
    assert_eq!(session.ready().count(), 3);
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("start_preserves_providers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host {
        fail_start: Some("client".into()),
        ..Host::new().await
    };
    assert!(
        matches!(session.initialize(&mut host).await.map_err(failure),
        Err(PluginFailure::Start { artifact, cleanup, .. }) if artifact == "client" && cleanup.is_empty())
    );
    assert!(session.ready().any(|id| id == "provider"));
    assert!(!session.ready().any(|id| id == "client"));
    assert!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    assert!(matches!(
        host.operations()
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    assert!(
        !host
            .control
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "stop:provider")
    );

    // The product may remove a failed optional plugin. The next admission
    // starts remaining branches without replaying its healthy provider.
    let next = PluginGraph::resolve(
        plan()
            .ordered()
            .iter()
            .filter(|artifact| artifact.id != "client")
            .cloned(),
    )
    .unwrap();
    session
        .reload_parts(next.clone(), inventories(&next), [], &mut host)
        .await
        .unwrap();
    assert_eq!(
        host.control
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.as_str() == "start:provider")
            .count(),
        1
    );
    assert_eq!(session.ready().collect::<Vec<_>>(), ["other", "provider"]);
    assert!(
        !session
            .router()
            .graph()
            .ordered()
            .iter()
            .any(|definition| definition.id == "client")
    );
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("initialization_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let (started, mut starting) = tokio::sync::watch::channel(String::new());
    let mut host = Host {
        block: Some("provider".to_owned()),
        started: Some(started),
        ..Host::new().await
    };
    {
        let initialization = session.initialize(&mut host);
        tokio::pin!(initialization);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                outcome = &mut initialization => panic!("initialization completed before cancellation: {outcome:?}"),
                reached = starting.wait_for(|owner| owner == "provider") => { reached.unwrap(); }
            }
        }).await.unwrap();
    }
    let original = session.status("provider").unwrap().generation.unwrap();
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("provider"), None);
        assert_eq!(control.backend_drops.get("provider"), None);
    }
    assert!(
        host.control
            .lock()
            .unwrap()
            .callers
            .contains_key("provider")
    );
    assert!(!session.ready().any(|id| id == "provider"));
    assert!(matches!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await,
        Err(Error::NotReady {
            domain: "operation plugin"
        })
    ));
    host.block = None;
    session.initialize(&mut host).await.unwrap();
    assert_ne!(
        session.status("provider").unwrap().generation.unwrap(),
        original
    );
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("provider"), Some(&1));
        assert_eq!(control.backend_drops.get("provider"), Some(&1));
        let transitions: Vec<_> = control
            .events
            .iter()
            .filter(|event| matches!(event.as_str(), "start:provider" | "stop:provider"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            transitions,
            ["start:provider", "stop:provider", "start:provider"]
        );
    }
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("start_retains_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut manager = session();
    let (started, mut starting) = tokio::sync::watch::channel(String::new());
    let mut host = Host {
        block: Some("client".into()),
        started: Some(started),
        ..Host::new().await
    };
    {
        let start = manager.initialize(&mut host);
        tokio::pin!(start);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                outcome = &mut start => panic!("initialization completed before cancellation: {outcome:?}"),
                reached = starting.wait_for(|owner| owner == "client") => { reached.unwrap(); }
            }
        }).await.unwrap();
    }

    let retained = manager.status("client").unwrap();
    assert_eq!(retained.state, PluginState::Initializing);
    assert!(retained.generation.is_some());
    assert!(retained.active_definition.is_some());
    assert!(host.control.lock().unwrap().callers.contains_key("client"));
    let caller = host.control.lock().unwrap().callers["client"].clone();
    let operations = host.operations().clone();
    assert!(matches!(
        operations
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotReady {
            domain: "operation plugin"
        })
    ));
    assert!(
        host.call(&caller, "provider.run", request_payload())
            .await
            .is_ok()
    );

    // Final Manager Drop Settles Partial Setup Before Invalidating Callers.
    let cleanup = manager.cleanup();
    drop(manager);
    cleanup.join().await.unwrap();
    {
        let control = host.control.lock().unwrap();
        assert!(!control.callers.contains_key("client"));
        for owner in ["client", "provider"] {
            assert_eq!(control.stop_calls.get(owner), Some(&1));
            assert_eq!(control.backend_drops.get(owner), Some(&1));
        }
    }
    assert!(matches!(
        operations
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    assert!(matches!(
        host.call(&caller, "provider.run", request_payload()).await,
        Err(Error::OperationCaller {
            source: DispatchError::Unavailable
        })
    ));

});
        });
    }
    {
        scenario_runtime::check("inventory_refuses_commit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let graph = plan();
                    let candidate = candidate_parts(&graph, inventories(&graph), []).unwrap();
                    let mut definitions: Vec<_> = candidate.definitions().cloned().collect();
                    let client = definitions
                        .iter_mut()
                        .find(|definition| definition.id == "client")
                        .unwrap();
                    client
                        .contributions
                        .streams
                        .push(harmos_contract::StreamDeclaration {
                            id: "client.discovery".into(),
                            schema: 1,
                            window: 4,
                            metadata: harmos::Metadata::new(
                                harmos::Identifier::new("client.discovery").unwrap(),
                                "Discovery",
                                None,
                            ),
                            row_metadata: Vec::new(),
                            fields: Vec::new(),
                        });
                    let mut manager =
                        PluginManager::new(PluginDefinitions::new(definitions, []).unwrap())
                            .unwrap();
                    let mut host = Host::new().await;
                    let PluginFailure::Start {
                        artifact,
                        source,
                        cleanup,
                    } = failure(manager.initialize(&mut host).await.unwrap_err())
                    else {
                        panic!("expected retained initialization failure");
                    };
                    assert_eq!(artifact, "client");
                    assert!(cleanup.is_empty());
                    assert!(matches!(failure(source), PluginFailure::Routing(
        DispatchError::IncompleteStreams { owner }) if owner == "client"));
                    assert_eq!(manager.status("client").unwrap().state, PluginState::Failed);
                    assert!(
                        !manager
                            .status("client")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(
                        host.control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:client")
                    );
                    assert!(manager.caller("provider").is_ok());
                    assert!(
                        !host
                            .operations()
                            .definitions()
                            .iter()
                            .any(|definition| definition.id.as_str() == "client.run")
                    );
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("mismatch_cleans_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host {
                        wrong_backend: Some("client".into()),
                        ..Host::new().await
                    };
                    let error = manager.initialize(&mut host).await.unwrap_err();
                    assert!(matches!(
                        failure(error),
                        PluginFailure::Start {
                            source: Error::InvalidInput {
                                field: "plugin backend",
                                ..
                            },
                            ..
                        }
                    ));
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "start:client")
                    );
                    assert!(
                        host.control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:client")
                    );
                    assert_eq!(manager.status("client").unwrap().state, PluginState::Failed);
                    assert!(
                        !manager
                            .status("client")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(manager.caller("provider").is_ok());
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("outbound_caller_ready", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let graph = plan();
                    let mut inventory = inventories(&graph);
                    inventory
                        .iter_mut()
                        .find(|(id, _)| id == "client")
                        .unwrap()
                        .1
                        .clear();
                    let mut manager =
                        PluginManager::new(candidate_parts(&graph, inventory, []).unwrap())
                            .unwrap();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    let caller = manager.caller("client").unwrap();
                    assert_eq!(
                        Some(caller.generation()),
                        manager.status("client").unwrap().generation
                    );
                    assert!(
                        host.call(&caller, "provider.run", request_payload())
                            .await
                            .is_ok()
                    );
                    assert!(matches!(
                        host.operations()
                            .call_encoded("client.run", request_payload())
                            .await,
                        Err(Error::NotRegistered {
                            domain: "operation",
                            ..
                        })
                    ));
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
}
