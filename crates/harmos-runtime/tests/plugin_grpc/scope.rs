/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Invocation Scope Across Plugin Placements

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
async fn bound_scope_reaches_handlers_and_children_and_missing_scope_is_refused() {
    // Attach a Remote Provider and a Managed Plugin.
    let host = HostFixture::new().await;
    let provider = host.sdk("scoped-provider").await;
    let provider = host.runtime.attach_grpc(Placement::Remote(provider.address.clone()), support::settings()).await.unwrap();
    let managed = host.runtime.attach_grpc(host.managed_sdk("scoped"), support::settings()).await.unwrap();

    // Keep Two Opaque Scopes Isolated Through Host and Cross-Plugin Children.
    let first = host.runtime.operations.with_scope("scope/α").unwrap();
    let second = host.runtime.operations.with_scope("scope/β").unwrap();
    let (left, right) = tokio::join!(
        first.call_encoded("fixture.scoped", payload("scope/α".as_bytes())),
        second.call_encoded("fixture.scoped", payload("scope/β".as_bytes()))
    );
    assert_eq!(left.unwrap(), payload("scope/α\0scope/α\0scope/α".as_bytes()));
    assert_eq!(right.unwrap(), payload("scope/β\0scope/β\0scope/β".as_bytes()));

    // Refuse Missing Transport Scope Even When the Payload Contains a Scope.
    let error = host.runtime.operations.call_encoded("fixture.scoped", payload("scope/α".as_bytes())).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(matches!(call_error(&error), harmos_runtime::plugin::grpc::CallError::Remote(failure) if failure.code == "scope_required"));
    assert!(host.runtime.operations.with_scope("").is_err());

    // Retire the Managed Activation and Attach the Same Contract Remotely.
    managed.stop("Switch Placement").await.unwrap();
    let remote = host.sdk("scoped").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let response = second.call_encoded("fixture.scoped", payload("scope/β".as_bytes())).await.unwrap();
    assert_eq!(response, payload("scope/β\0scope/β\0scope/β".as_bytes()));
    let error = host.runtime.operations.call_encoded("fixture.scoped", payload("scope/β".as_bytes())).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(matches!(call_error(&error), harmos_runtime::plugin::grpc::CallError::Remote(failure) if failure.code == "scope_required"));

    // Clean Up Both Activations and Their Scoped Call Handles.
    attached.stop("Story Complete").await.unwrap();
    provider.stop("Story Complete").await.unwrap();
    drop((first, second));
    host.runtime.shutdown().await.unwrap();
}
