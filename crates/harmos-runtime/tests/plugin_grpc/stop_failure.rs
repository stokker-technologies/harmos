/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cleanup Failure Leaves Peers Available

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

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
async fn failed_cleanup_closes_only_its_plugin_and_preserves_the_failure() {
    // Connect A Failing Plugin And An Independent Peer
    let host = super::invoke_support::HostFixture::new().await;
    let sdk = host.sdk("stop-failure").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();
    let activation = attached.activation().unwrap();
    let status = attached.status();
    let peer = host.sdk("provider").await;
    let peer = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(peer.address.clone()), super::support::settings())
        .await
        .unwrap();

    // A Cleanup Failure Is Reported After Closing Its Connection
    let error = attached.stop("Removed").await.unwrap_err();
    assert!(matches!(error.as_ref(), harmos_runtime::plugin::grpc::Error::Transport(_)));
    assert!(!activation.is_connected());
    assert!(status.borrow().activation.is_none());
    assert!(status.borrow().failure.is_some());
    assert!(host.runtime.operations.definition("fixture.echo").is_err());

    // Other Plugins And Host Operations Still Run
    let input = super::invoke_support::payload(b"peer survives");
    assert_eq!(host.runtime.operations.call_encoded("fixture.public", input.clone()).await.unwrap(), input);
    let response = host
        .runtime
        .operations
        .submit::<super::invoke_support::Host>(super::invoke_support::Host { value: b"host survives".to_vec() })
        .unwrap()
        .await
        .unwrap();
    assert_eq!(response.value, b"host survives");
    peer.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
