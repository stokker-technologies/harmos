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

/* === Tests === */

#[tokio::test]
async fn reload_orders_lifecycle() {
    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    let provider = host
        .control
        .lock()
        .unwrap()
        .events
        .iter()
        .position(|event| event == "start:provider")
        .unwrap();
    let client = host
        .control
        .lock()
        .unwrap()
        .events
        .iter()
        .position(|event| event == "start:client")
        .unwrap();
    assert!(provider < client);
    let unchanged = host.control.lock().unwrap().callers["other"].clone();
    host.control.lock().unwrap().events.clear();
    let next = plan();
    session
        .reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".to_owned()],
            &mut host,
        )
        .await
        .unwrap();
    assert_eq!(
        host.control.lock().unwrap().events,
        [
            "fence:client,provider",
            "stop:client",
            "stop:provider",
            "start:provider",
            "start:client"
        ]
    );
    // Its Original Generation Remains Occupied And Callable
    assert!(matches!(
        session.router().enter("other"),
        Err(DispatchError::Occupied(_))
    ));
    assert!(
        host.operations()
            .call_encoded("other.run", request_payload())
            .await
            .is_ok()
    );
    drop(unchanged);
    session.shutdown(&mut host).await.unwrap();
    assert!(host.control.lock().unwrap().callers.is_empty());
}

#[tokio::test]
async fn preflight_preserves_activation() {
    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    host.control.lock().unwrap().events.clear();
    let next = plan();
    let mut inventory = inventories(&next);
    inventory[1].1[0].id = inventory[0].1[0].id.clone();
    assert!(matches!(
        session
            .reload_parts(next.clone(), inventory, [], &mut host)
            .await
            .map_err(failure),
        Err(PluginFailure::Plan(
            harmos::plugin::Error::OperationConflict { .. }
        ))
    ));
    assert!(host.control.lock().unwrap().events.is_empty());
    let mut inventory = inventories(&next);
    inventory
        .iter_mut()
        .find(|(id, _)| id == "provider")
        .unwrap()
        .1[0]
        .policy = OperationPolicy {
        timeout: Duration::from_millis(200),
        attempt_timeout: Duration::from_millis(200),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    session
        .reload_parts(next, inventory, [], &mut host)
        .await
        .unwrap();
    assert_eq!(
        host.control.lock().unwrap().events,
        [
            "fence:client,provider",
            "stop:client",
            "stop:provider",
            "start:provider",
            "start:client"
        ]
    );
    session.shutdown(&mut host).await.unwrap();
}

#[tokio::test]
async fn restrictions_revoke_dependents() {
    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    let old = host.control.lock().unwrap().callers["client"].clone();
    assert!(
        host.call(&old, "provider.run", request_payload())
            .await
            .is_ok()
    );
    host.control.lock().unwrap().events.clear();

    let next = plan();
    let mut inventory = inventories(&next);
    inventory
        .iter_mut()
        .find(|(id, _)| id == "provider")
        .unwrap()
        .1[0]
        .visibility = Visibility::Private;
    host.control.lock().unwrap().skip_dependency_calls = true;
    session
        .reload_parts(next, inventory, [], &mut host)
        .await
        .unwrap();

    assert_eq!(
        host.control.lock().unwrap().events,
        [
            "fence:client,provider",
            "stop:client",
            "stop:provider",
            "start:provider",
            "start:client"
        ]
    );
    assert!(
        host.call(&old, "provider.run", request_payload())
            .await
            .is_err()
    );
    let current = host.control.lock().unwrap().callers["client"].clone();
    assert!(matches!(
        host.call(&current, "provider.run", request_payload()).await,
        Err(Error::OperationUnauthorized { .. })
    ));
    assert!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    session.shutdown(&mut host).await.unwrap();
}

#[tokio::test]
async fn reload_preserves_calls() {
    let mut manager = session();
    let mut host = Host::new().await;
    manager.initialize(&mut host).await.unwrap();
    assert_eq!(host.transitions, [PluginTransition::Initialize]);
    let old = host.control.lock().unwrap().callers["client"].clone();
    let operations = host.operations().clone();
    let old_operations = operations.caller(&old).unwrap();
    let before = manager.resolve().unwrap();
    let next = plan();
    host.control.lock().unwrap().events.clear();
    host.block_fence = true;
    {
        let mut reload = Box::pin(manager.reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".into()],
            &mut host,
        ));
        std::future::poll_fn(|context| {
            assert!(reload.as_mut().poll(context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        assert!(
            old_operations
                .call_encoded("provider.run", request_payload())
                .await
                .is_ok()
        );
        assert!(
            operations
                .call_encoded("client.run", request_payload())
                .await
                .is_ok()
        );
    }
    assert_eq!(manager.resolve().unwrap(), before);
    assert_eq!(manager.ready().count(), 3);
    assert_eq!(
        host.control.lock().unwrap().events,
        ["fence:client,provider"]
    );
    assert_eq!(host.transitions.last(), Some(&PluginTransition::Reload));

    host.block_fence = false;
    host.fail_fence = true;
    let error = manager
        .reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".into()],
            &mut host,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        failure(error),
        PluginFailure::Fence(Error::ResourceExhausted {
            resource: "plugin execution drain"
        })
    ));
    assert_eq!(manager.resolve().unwrap(), before);
    assert_eq!(manager.ready().count(), 3);
    assert!(
        host.control
            .lock()
            .unwrap()
            .events
            .iter()
            .all(|event| event.starts_with("fence:"))
    );
    assert!(
        host.call(&old, "provider.run", request_payload())
            .await
            .is_ok()
    );

    host.fail_fence = false;
    manager
        .reload_parts(
            next.clone(),
            inventories(&next),
            ["provider".into()],
            &mut host,
        )
        .await
        .unwrap();
    assert!(matches!(
        host.call(&old, "provider.run", request_payload()).await,
        Err(Error::OperationCaller {
            source: DispatchError::Unavailable
        })
    ));
    manager.shutdown(&mut host).await.unwrap();
    assert_eq!(host.transitions.last(), Some(&PluginTransition::Shutdown));
}

#[tokio::test(start_paused = true)]
async fn generations_guard_plans() {
    {
        let mut manager = session();
        let other = session();
        let foreign = other.plan_reload(candidate([])).unwrap();
        let stale = manager.plan_reload(candidate([])).unwrap();
        let mut host = Host::new().await;
        assert!(matches!(
            manager.apply_reload(foreign, &mut host).await,
            Err(Error::InvalidInput {
                field: "plugin reload plan",
                ..
            })
        ));
        assert!(host.control.lock().unwrap().events.is_empty());
        manager.initialize(&mut host).await.unwrap();
        host.control.lock().unwrap().events.clear();
        let statuses: Vec<_> = manager.statuses().collect();
        assert!(matches!(
            failure(manager.apply_reload(stale, &mut host).await.unwrap_err()),
            PluginFailure::StalePlan { .. }
        ));
        assert!(host.control.lock().unwrap().events.is_empty());
        assert_eq!(manager.statuses().collect::<Vec<_>>(), statuses);
        manager.shutdown(&mut host).await.unwrap();
    }
    {
        let mut manager = session();
        let mut host = Host::new().await;
        assert!(
            manager.statuses().all(
                |status| status.state == PluginState::Registered && status.generation.is_none()
            )
        );
        assert_eq!(manager.dependencies("client").unwrap(), ["provider"]);
        assert_eq!(manager.dependents("provider").unwrap(), ["client"]);
        assert!(manager.dependencies("unknown").is_err());
        manager.initialize(&mut host).await.unwrap();
        let old = manager.caller("client").unwrap();
        let old_status = manager.status("client").unwrap();
        assert_eq!(Some(old.generation()), old_status.generation);
        assert_eq!(old_status.definition, old_status.active_definition);
        let independent = manager.status("other").unwrap();
        let plan = manager.plan_reload(candidate(["provider".into()])).unwrap();
        assert_eq!(plan.affected(), ["client", "provider"]);
        assert_eq!(plan.candidate().definitions().count(), 3);
        manager.apply_reload(plan, &mut host).await.unwrap();
        assert_eq!(manager.status("other").unwrap(), independent);
        assert_ne!(
            manager.status("client").unwrap().generation,
            old_status.generation
        );
        assert!(matches!(
            host.call(&old, "provider.run", request_payload()).await,
            Err(Error::OperationCaller {
                source: DispatchError::Unavailable
            })
        ));
        let current = manager.caller("client").unwrap();
        assert_eq!(
            Some(current.generation()),
            manager.status("client").unwrap().generation
        );
        manager.shutdown(&mut host).await.unwrap();
        assert!(manager.statuses().all(|status| status.generation.is_none()));
    }
    {
        let mut manager = session();
        let mut host = Host::new().await;
        manager.initialize(&mut host).await.unwrap();
        let independent = manager.status("other").unwrap();
        let old = manager.status("client").unwrap().generation;
        let changes: [fn(&mut PluginDefinition); 3] = [
            |definition| {
                definition.artifact = harmos::plugin::PluginArtifact::Embedded {
                    revision: "compiled-v2".into(),
                }
            },
            |definition| definition.configuration = br#"{ "setting": -0.0 }"#.to_vec(),
            |definition| {
                definition
                    .contributions
                    .registrations
                    .push(harmos_domain::Registration {
                        namespace: "owner.catalog".into(),
                        schema: 1,
                        payload: b"{}".to_vec(),
                    })
            },
        ];
        for change in changes {
            let definitions = manager.definitions().cloned().map(|mut definition| {
                if definition.id == "provider" {
                    change(&mut definition);
                }
                definition
            });
            let candidate = PluginDefinitions::new(definitions, []).unwrap();
            let plan = manager.plan_reload(candidate).unwrap();
            assert_eq!(plan.affected(), ["client", "provider"]);
            manager.apply_reload(plan, &mut host).await.unwrap();
            assert_eq!(manager.status("other").unwrap(), independent);
        }
        assert_ne!(manager.status("client").unwrap().generation, old);
        assert_eq!(
            manager.definition("provider").unwrap().configuration,
            br#"{ "setting": -0.0 }"#
        );
        manager.shutdown(&mut host).await.unwrap();
    }
}
