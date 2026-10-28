/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Guest Dependency Admission Against The Original Provider.

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
async fn dependencies_share_admission() {
    let _report = crate::report::system();
    let provider = std::sync::Arc::new(ProviderOwner::default());
    let runtime = runtime_with_provider(provider.clone()).await;
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let mut manager = harmos_runtime::plugin::PluginManager::new(dependency_candidate(&mut lifecycle, "dependencies").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(runtime.operations.call::<EchoOperation>(EchoOperation { say: "dependency:normal".into() }).await.unwrap().said, "normal");
    for (say, message) in [("refuse", "provider refusal"), ("timeout", "deadline elapsed")] {
        let error = runtime.operations.call::<EchoOperation>(EchoOperation { say: format!("dependency:{say}") }).await.unwrap_err();
        assert!(
            matches!(component_error(&error), harmos_runtime::operations::ComponentError::Remote(failure) if failure.code() == harmos_runtime::operations::FailureKind::Permanent && failure.cause().contains(message)),
            "{error:?}"
        );
    }
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), ["initialize", "normal"]);

    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_dependency_admission() {
    let _report = crate::report::system();
    let provider = std::sync::Arc::new(ProviderOwner::default());
    let runtime = runtime_with_provider(provider.clone()).await;
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let mut manager = harmos_runtime::plugin::PluginManager::new(dependency_candidate(&mut lifecycle, "idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let invocation =
        runtime.operations.submit::<super::super::sandbox::EchoOperation>(EchoOperation { say: "dependency:cancel".into() }).await.unwrap();
    provider.started.notified().await;
    invocation.cancel();
    assert!(matches!(invocation.await, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    provider.cancelled.notified().await;
    let after = runtime.operations.call::<EchoOperation>(EchoOperation { say: "after cancellation".into() }).await.unwrap_err();
    assert!(matches!(component_error(&after), harmos_runtime::operations::ComponentError::Unavailable));
    retire_failed_guest(&mut manager, &mut lifecycle).await;

    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn missing_provider_refused() {
    let _report = crate::report::system();
    let provider = std::sync::Arc::new(ProviderOwner::default());
    let runtime = runtime_with_provider(provider.clone()).await;
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let candidate = dependency_candidate(&mut lifecycle, "idle").await;
    let missing = candidate.definitions().filter(|definition| definition.id != "provider").cloned();
    // No Plugin Provides It, So Planning Routes It To The Host, Whose Preflight Refuses It
    let planned = harmos_runtime::plugin::PluginDefinitions::new(missing, []).unwrap();
    let routed = planned
        .host_requirements()
        .map(|(definition, required)| (definition.id.clone(), required.contract.operation().id.to_string()))
        .filter(|(_, operation)| operation == "provider.echo")
        .collect::<Vec<_>>();
    assert_eq!(routed, [("sandbox_guest".to_owned(), "provider.echo".to_owned())]);
    assert!(matches!(
        runtime.operations.validate_requirements(&planned),
        Err(harmos_runtime::Error::NotRegistered { domain: "host provider", .. })
    ));
    assert!(provider.observed.lock().unwrap().is_empty());
    let mut operations: Vec<_> = runtime.operations.definitions().iter().map(|operation| operation.id.to_string()).collect();
    operations.sort();
    assert_eq!(operations, ["host.publish"]);
    drop(lifecycle);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_fences_callers() {
    let _report = crate::report::system();
    use std::sync::Arc;
    let runtime = runtime().await;
    let provider = Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let mut manager =
        harmos_runtime::plugin::PluginManager::new(native_dependency_candidate(&mut lifecycle, "dependencies", true).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let stale = runtime.operations.caller(&manager.caller("client").unwrap()).unwrap();
    assert_eq!(
        runtime.operations.call::<ForwardDeclaration>(ForwardDeclaration { say: "dependency:roundtrip".into() }).await.unwrap().said,
        "roundtrip"
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), ["initialize", "roundtrip"]);
    assert!(stale.call::<EchoOperation>(EchoOperation { say: "closed".into() }).await.is_err());
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert!(!provider.observed.lock().unwrap().iter().any(|say| say == "cleanup"));

    drop((manager, lifecycle, stale));
    runtime.shutdown().await.unwrap();
}

/* === Tests === */
