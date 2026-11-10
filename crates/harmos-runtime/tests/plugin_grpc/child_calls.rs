/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Calls Host and Public Peer

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
async fn child_calls_reach_host_and_public_peer_without_start_order() {
    // Admit The Caller Before Its Peer Exists
    let host = HostFixture::new().await;
    let caller = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();

    // Call A Host Operation Through The Parent Invoke
    let input = payload(b"host");
    assert_eq!(host.runtime.operations.call_encoded("fixture.forward", input.clone()).await.unwrap(), input);

    // Refuse An Incompatible Peer That Arrives After Its Caller
    let incompatible = host.sdk("incompatible").await;
    let refused = host.runtime.attach_grpc(Placement::Remote(incompatible.address.clone()), support::settings()).await;
    assert!(matches!(refused, Err(harmos_runtime::plugin::grpc::Error::Invalid(_))));

    // Admit A Peer And Call Its Public Operation
    let peer = host.sdk("provider").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();
    let input = payload(b"public");
    assert_eq!(host.runtime.operations.call_encoded("fixture.forward", input.clone()).await.unwrap(), input);

    // Missing Operations Fail With Their Identifier
    let error = host.runtime.operations.call_encoded("fixture.forward", payload(b"unknown")).await.unwrap_err();
    let failure = call_error(&error);
    assert_eq!(failure.classification(), harmos_protocol::v1::Classification::Refused);
    assert!(failure.to_string().contains("fixture.missing"));
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
