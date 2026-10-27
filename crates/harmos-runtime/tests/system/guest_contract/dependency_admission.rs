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
    let runtime = runtime().await;
    let provider = std::sync::Arc::new(ProviderOwner::default());
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_dependency_admission() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let provider = std::sync::Arc::new(ProviderOwner::default());
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn background_cleanup_retained() {
    let _report = crate::report::system();
    use std::sync::{Arc, atomic::Ordering};
    let runtime = runtime().await;
    let provider = Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let mut manager = harmos_runtime::plugin::PluginManager::new(dependency_candidate(&mut lifecycle, "idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let prepared = provider.prepared.load(Ordering::SeqCst);

    // The Saved Initialization Context Uses The Original Generation And Provider.
    let answered = runtime
        .operations
        .call::<EchoOperation>(EchoOperation {
            say: "saved-dependency:original-background".into(),
        })
        .await
        .unwrap();
    assert_eq!(answered.said, "original-background");
    provider.fail_cleanup.store(true, Ordering::SeqCst);
    let refused = runtime
        .operations
        .call::<EchoOperation>(EchoOperation {
            say: "saved-dependency:physical-cleanup".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(component_error(&refused), harmos_runtime::operations::ComponentError::Remote { .. }));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);

    // Destroying The Actual Guest Store Does Not Settle Its Failed Host Debt.
    let trapped = runtime.operations.call::<FailOperation>(FailOperation {}).await.unwrap_err();
    assert!(execution_error(&trapped).downcast_ref::<wasmtime::Trap>().is_some());
    let first = manager.shutdown(&mut lifecycle).await.unwrap_err();
    let harmos_runtime::Error::PluginLifecycle { source } = first else {
        panic!("the failed guest must report its original shutdown refusal");
    };
    let harmos_runtime::plugin::PluginFailure::Cleanup(failures) = *source else {
        panic!("the failed guest must retain its original ending");
    };
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].artifact, "sandbox_guest");

    let harmos_runtime::Error::TaskFailed { domain, source } = &failures[0].error else {
        panic!("the manager preserves its original cleanup failure");
    };
    assert_eq!(*domain, "plugin cleanup");
    let original = source.downcast_ref::<harmos_runtime::Error>().expect("the original SDK error remains available");
    assert!(matches!(component_error(original), harmos_runtime::operations::ComponentError::Unavailable));

    for _ in 0..2 {
        let harmos_runtime::Error::PluginLifecycle { source } = manager.shutdown(&mut lifecycle).await.unwrap_err() else {
            panic!("the original host cleanup must remain retryable");
        };
        let harmos_runtime::plugin::PluginFailure::Cleanup(failures) = *source else {
            panic!("the original cleanup failure must remain inspectable");
        };
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].artifact, "sandbox_guest");
        let mut source = Some(&failures[0].error as &dyn std::error::Error);
        let mut original_refusal = false;
        while let Some(error) = source {
            original_refusal |= error.to_string().contains("fixture physical cleanup refused");
            source = error.source();
        }
        assert!(original_refusal);
        assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
        assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);
    }
    let failed_attempts = provider.cleanup_attempts.load(Ordering::SeqCst);
    assert!(failed_attempts >= 2);
    assert_eq!(provider.observed.lock().unwrap().as_slice(), ["original-background", "physical-cleanup"]);

    // The Original Finalizer Settles Once Its Physical Cleanup Succeeds
    provider.fail_cleanup.store(false, Ordering::SeqCst);
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
    assert_eq!(provider.cleanup_attempts.load(Ordering::SeqCst), failed_attempts + 1);
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 1);
    drop((answered, manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn missing_provider_refused() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let provider = std::sync::Arc::new(ProviderOwner::default());
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
    runtime.stop().await.unwrap();
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_shutdown_cleanup() {
    let _report = crate::report::system();
    use std::sync::{Arc, atomic::Ordering};
    let runtime = runtime().await;
    let provider = Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone()).await;
    let mut manager =
        harmos_runtime::plugin::PluginManager::new(native_dependency_candidate(&mut lifecycle, "dependencies", false).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let prepared = provider.prepared.load(Ordering::SeqCst);
    provider.hold_cleanup.store(true, Ordering::SeqCst);
    let invocation = runtime
        .operations
        .submit::<super::super::sandbox::ProviderEchoOperation>(ProviderEchoOperation { say: "physical-cleanup".into() })
        .await
        .unwrap();
    provider.cleanup_started.notified().await;
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
    let mut waiter = Box::pin(manager.shutdown(&mut lifecycle));
    tokio::select! {
        biased;
        result = &mut waiter => panic!("shutdown returned before original physical cleanup settled: {result:?}"),
        () = std::future::ready(()) => {},
    }
    drop(waiter);
    assert!(!provider.hold_cleanup.load(Ordering::SeqCst));
    assert!(runtime.operations.call::<EchoOperation>(EchoOperation { say: "closed".into() }).await.is_err());
    provider.cleanup_release.notify_one();
    manager.shutdown(&mut lifecycle).await.unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(invocation.await.unwrap().said, "physical-cleanup");
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
    assert_eq!(provider.cleanup_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 1);

    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

/* === Tests === */
