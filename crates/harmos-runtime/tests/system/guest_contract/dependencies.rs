/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Child Authority

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{contract::*, support};
use harmos::OperationRequest as _;
use prost::Message as _;
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, SystemTime},
};

// Standard Library Imports

// Third Party Imports

// Local Imports

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
async fn guest_dependencies_keep_admission_and_retire_saved_authority() {
    let _report = crate::report::system();
    // Refuse the Declared Requirement Before Activating Without Its Provider
    let runtime = harmos_runtime::Runtime::<crate::support::Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.settings().register(crate::support::catalog());
        setup.operation(
            harmos_runtime::operations::OperationFactory::protobuf::<support::Publisher>().unwrap(),
            harmos_runtime::operations::Lane::default(),
        );
    })
    .await
    .unwrap();
    let loaded = support::host(&runtime).load(&support::sandbox()).unwrap();
    let definition = harmos_runtime::guest::PluginDefinition::guest(&loaded, b"null".to_vec()).unwrap();
    let candidate = harmos_runtime::guest::PluginDefinitions::new([definition], []).unwrap();
    assert!(matches!(runtime.operations.validate_requirements(&candidate), Err(harmos_runtime::Error::NotRegistered {
        domain: "host provider", name,
    }) if name == "sandbox_guest/provider.echo"));
    runtime.shutdown().await.unwrap();

    // Admit the Guest's Declared Host Dependencies
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let owner = Arc::new(support::ProviderOwner::default());
    let lane = harmos_runtime::operations::Lane::default();
    let runtime = support::runtime(owner.clone(), lane, clock).await;
    let host = support::host(&runtime);
    runtime
        .plugin
        .register_guest(
            host.load(&support::sandbox()).unwrap(),
            harmos_runtime::config::PluginSettings {
                grants: vec![harmos_operations::plugin::Grant::of("provider.echo", "allowed")],
                ..support::settings("dependencies")
            },
            lane,
        )
        .await
        .unwrap();
    runtime.plugin.initialize().await.unwrap();
    assert_eq!(*owner.observed.lock().unwrap(), ["initialize"]);

    // Route Current and Retained Generation Calls to the Original Provider
    assert_eq!(support::call(&runtime.operations, EchoOperation { say: "dependency:normal".into() }).await.unwrap().said, "normal");
    assert_eq!(support::call(&runtime.operations, EchoOperation { say: "saved-dependency:saved".into() }).await.unwrap().said, "saved");
    assert!(support::call(&runtime.operations, EchoOperation { say: "saved-submit:forbidden".into() }).await.is_err());

    // The Consumer Grants Only One Resource Scope on That Declared Requirement
    assert_eq!(
        support::call(&runtime.operations, EchoOperation { say: "dependency:scope:allowed".into() }).await.unwrap().said,
        "scope:allowed"
    );
    assert!(support::call(&runtime.operations, EchoOperation { say: "dependency:scope:denied".into() }).await.is_err());

    // Carry the Provider's Refusal Without Losing Its Code
    let refused = support::call(&runtime.operations, EchoOperation { say: "dependency:refuse".into() }).await.unwrap_err();
    let harmos_runtime::operations::ComponentError::Remote(failure) = support::component_error(&refused) else {
        panic!("expected provider refusal")
    };
    assert_eq!(failure.refusal_code(), Some(73));

    // Refuse an Unsafe Retry Override Before Entering the Provider
    let before = owner.prepared.load(Ordering::SeqCst);
    let mut values = harmos_runtime::operations::CallContext::default();
    values.insert_portable(RetryChoice { retries: 1, delay_millis: 1 }).unwrap();
    let caller = runtime.operations.with_context(values);
    assert!(support::call(&caller, EchoOperation { say: "dependency:retry".into() }).await.is_err());
    assert_eq!(owner.prepared.load(Ordering::SeqCst), before);

    // Bound a Child Wait on the Host's Virtual Clock
    let invocation =
        runtime.operations.submit_encoded(EchoOperation::ID, EchoOperation { say: "dependency:timeout".into() }.encode_to_vec()).unwrap();
    let id = invocation.id();
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(invocation, drive);
    let timeout = answer.unwrap_err();
    let harmos_runtime::operations::ComponentError::Remote(failure) = support::component_error(&timeout) else {
        panic!("expected the original child failure")
    };
    assert_eq!(failure.refusal_code(), Some(-32_607));

    // Cancel Admitted Child Work and Observe Its Original Cleanup
    let mut invocation =
        runtime.operations.submit_encoded(EchoOperation::ID, EchoOperation { say: "dependency:cancel".into() }.encode_to_vec()).unwrap();
    let id = invocation.id();
    let cancelling = async {
        tokio::select! {
            _ = owner.started.notified() => {},
            result = &mut invocation => panic!("the child completed before cancellation: {result:?}"),
        }
        invocation.request_cancel();
        invocation.await
    };
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(cancelling, drive);
    assert!(matches!(answer, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
    owner.cancelled.notified().await;

    // A Failed Carrier Fences New Work and Reports Its Retained Cleanup
    assert!(support::call(&runtime.operations, EchoOperation { say: "after cancellation".into() }).await.is_err());
    if let Err(error) = runtime.plugin.stop("sandbox_guest").await {
        let harmos_runtime::Error::PluginLifecycle { source } = error else { panic!("expected the original cleanup failure") };
        let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
        assert_eq!(failures.len(), 1);
    }
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    let retired = runtime.plugin.status("sandbox_guest").unwrap();
    assert!(!retired.cleanup_pending);
    assert!(retired.last_failure.is_some());
    runtime.shutdown().await.unwrap();
}
