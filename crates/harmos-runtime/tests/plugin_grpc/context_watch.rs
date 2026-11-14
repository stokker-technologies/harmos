/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Cannot Observe an Invocation Owned by Another Caller.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;

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
async fn context_watch_refuses_an_invocation_owned_by_another_caller() {
    let host = HostFixture::new().await;
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let worker = host.sdk("caller").await;
    let _worker = host.runtime.attach_grpc(Placement::Remote(worker.address.clone()), support::settings()).await.unwrap();
    let watcher = host.sdk("watch").await;
    let _watcher = host.runtime.attach_grpc(Placement::Remote(watcher.address.clone()), support::settings()).await.unwrap();

    // A Child Call Is the Awaited Evidence That the Target Is Running.
    let target = host.runtime.operations.submit_encoded("fixture.wait", payload(b"target")).unwrap();
    host.signals.entered.notified().await;
    let watching = host.runtime.operations.submit_encoded("fixture.watch", payload(&target.id().value().to_le_bytes())).unwrap();
    let error = watching.await.unwrap_err();
    let harmos_runtime::plugin::grpc::CallError::Remote(failure) = call_error(&error) else {
        panic!("Expected an Invocation Ownership Refusal: {error:?}");
    };
    assert_eq!(failure.code(), harmos_runtime::operations::FailureKind::Permanent, "{failure:?}");
    assert_eq!(failure.context().operation(), Some("harmos.host.v1.Invocations/Watch"), "{failure:?}");
    target.request_cancel();
    assert_eq!(target.await.unwrap(), payload(b"target"));
    host.runtime.shutdown().await.unwrap();
}
