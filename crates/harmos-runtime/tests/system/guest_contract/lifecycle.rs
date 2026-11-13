/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Activate and Reload a Guest

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{contract::*, support};
use harmos::OperationRequest as _;
use prost::Message as _;
use std::{
    sync::Arc,
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
async fn guest_lifecycle_keeps_requests_resources_and_attempt_authority() {
    let _report = crate::report::system();
    // Boot the Host and Collect Inert Guest Metadata
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let owner = Arc::new(support::ProviderOwner::default());
    let lane = harmos_runtime::operations::Lane::default();
    let runtime = support::runtime(owner.clone(), lane, clock).await;
    let host = support::host(&runtime);
    let loaded = host.load(&support::sandbox()).unwrap();
    assert_eq!(loaded.declaration().id, "sandbox_guest");
    assert!(owner.observed.lock().unwrap().is_empty());
    assert!(support::call(&runtime.operations, EchoOperation { say: "inert".into() }).await.is_err());

    // Activate the Authoritative Catalog Without an Authored Host Mirror
    runtime.plugin.register_guest(loaded, support::settings("normal"), lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let first = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(first.state, harmos_runtime::guest::PluginState::Ready);
    assert_eq!(support::call(&runtime.operations, EchoOperation { say: "one".into() }).await.unwrap().said, "heard one");

    // Retry Original Request Clones While Retaining Authored Resource State
    for value in ["first", "second", "third"] {
        let invocation =
            runtime.operations.submit_encoded(ReadOperation::ID, ReadOperation { value: value.into() }.encode_to_vec()).unwrap();
        let id = invocation.id();
        let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
        let (answer, ()) = tokio::join!(invocation, drive);
        assert_eq!(Value::decode(answer.unwrap().as_slice()).unwrap().value, format!("{value}:2"));
    }
    assert_eq!(support::call(&runtime.operations, CountsOperation { value: "counts".into() }).await.unwrap().value, "6");

    // Preserve a Parent's Portable Values Across a Child Overlay
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert_portable(Stage { id: "parent".into() }).unwrap();
    let caller = runtime.operations.with_context(context.clone());
    let answer = support::call(&caller, ReadOperation { value: "context-parent".into() }).await.unwrap();
    assert_eq!(answer.value, "parent:child");
    assert_eq!(context.value::<Stage>().unwrap().unwrap().id, "parent");

    // Refuse a Retired Attempt Capability on the Next Retry
    let invocation =
        runtime.operations.submit_encoded(ReadOperation::ID, ReadOperation { value: "parent".into() }.encode_to_vec()).unwrap();
    let id = invocation.id();
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(invocation, drive);
    assert_eq!(Value::decode(answer.unwrap().as_slice()).unwrap().value, "parent:2:stale:leaf:1");

    // Preserve a Classified Refusal and Keep the Original Instance Available
    let refused = support::call(&runtime.operations, DeclineOperation {}).await.unwrap_err();
    let harmos_runtime::operations::ComponentError::Remote(failure) = support::component_error(&refused) else {
        panic!("expected the guest refusal")
    };
    assert_eq!(failure.refusal_code(), Some(7));
    assert_eq!(
        support::call(&runtime.operations, EchoOperation { say: "after refusal".into() }).await.unwrap().said,
        "heard after refusal"
    );

    // Reload a New Activation and Withdraw Its Operations on Stop
    runtime.plugin.reload("sandbox_guest").await.unwrap();
    let reloaded = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(reloaded.state, harmos_runtime::guest::PluginState::Ready);
    assert_ne!(first.generation, reloaded.generation);
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    assert!(support::call(&runtime.operations, EchoOperation { say: "stopped".into() }).await.is_err());
    runtime.shutdown().await.unwrap();

    // Refuse a Child That Would Reenter Its Parent's Serial Lane
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = crate::support::SERIAL;
    let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
    let host = support::host(&runtime);
    runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), support::settings("normal"), lane).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let invocation =
        runtime.operations.submit_encoded(ReadOperation::ID, ReadOperation { value: "parent".into() }.encode_to_vec()).unwrap();
    let id = invocation.id();
    let drive = async { controller.advance_until(Duration::from_secs(2), || runtime.operations.status(id).is_none()).await.unwrap() };
    let (answer, ()) = tokio::join!(invocation, drive);
    assert!(matches!(answer, Err(harmos_runtime::Error::OperationFailed { .. })));
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    runtime.shutdown().await.unwrap();
}
