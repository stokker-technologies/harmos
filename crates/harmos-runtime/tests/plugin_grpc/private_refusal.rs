/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Peer Operation Is Refused

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;

/* === Tests === */

#[tokio::test]
async fn private_peer_operation_is_refused_across_plugins() {
    // Connect Caller And Peer SDK Plugins
    let host = HostFixture::new().await;
    let peer = host.sdk("provider").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();

    // Refuse The Call Across Plugin Owners
    let error = host.runtime.operations.call_encoded("fixture.forward", payload(b"secret")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(call_error(&error).to_string().contains("fixture.private"));

    // The Host Can Still Invoke That Private Operation
    let input = payload(b"host access");
    assert_eq!(host.runtime.operations.call_encoded("fixture.private", input.clone()).await.unwrap(), input);
    host.runtime.shutdown().await.unwrap();
}
