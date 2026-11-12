/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Execution Scope Across Plugin Placements

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
async fn scoped_lifecycle_calls_inherit_scope_and_refuse_unscoped_calls() {
    // Attach a Remote Provider and a Managed Lifecycle Plugin.
    let host = HostFixture::new().await;
    let provider = host.sdk("scoped-provider").await;
    let provider = host.runtime.attach_grpc(Placement::Remote(provider.address.clone()), support::settings()).await.unwrap();
    let managed = host.runtime.attach_grpc(host.managed_sdk("scoped"), support::settings()).await.unwrap();

    // Keep Two Execution Identities Isolated Through Host and Cross-Plugin Children.
    let first = host.runtime.operations.with_execution_scope("execution/α").unwrap();
    let second = host.runtime.operations.with_execution_scope("execution/β").unwrap();
    let (left, right) = tokio::join!(
        first.call_encoded("fixture.lifecycle", payload("execution/α".as_bytes())),
        second.call_encoded("fixture.lifecycle", payload("execution/β".as_bytes()))
    );
    assert_eq!(left.unwrap(), payload("execution/α\0execution/α\0execution/α".as_bytes()));
    assert_eq!(right.unwrap(), payload("execution/β\0execution/β\0execution/β".as_bytes()));

    // Refuse Missing Transport Scope Even When the Payload Names an Execution.
    let error = host.runtime.operations.call_encoded("fixture.lifecycle", payload("execution/α".as_bytes())).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(host.runtime.operations.with_execution_scope("").is_err());

    // Retire the Managed Activation and Attach the Same Contract Remotely.
    managed.stop("Switch Placement").await.unwrap();
    let remote = host.sdk("scoped").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let response = second.call_encoded("fixture.lifecycle", payload("execution/β".as_bytes())).await.unwrap();
    assert_eq!(response, payload("execution/β\0execution/β\0execution/β".as_bytes()));
    let error = host.runtime.operations.call_encoded("fixture.lifecycle", payload("execution/β".as_bytes())).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);

    // Clean Up Both Activations and Their Scoped Call Handles.
    attached.stop("Story Complete").await.unwrap();
    provider.stop("Story Complete").await.unwrap();
    drop((first, second));
    host.runtime.shutdown().await.unwrap();
}
