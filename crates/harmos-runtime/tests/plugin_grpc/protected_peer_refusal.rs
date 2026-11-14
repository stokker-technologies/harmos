/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protected Admission Retains Owner Access and Refuses a Peer.

/* === Imports === */

use harmos_protocol::model::Classification;

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;

/* === Tests === */

#[tokio::test]
async fn protected_peer_operation_is_refused_across_plugins() {
    // Connect Caller And Peer SDK Plugins
    let host = HostFixture::new().await;
    let peer = host.sdk("provider").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();

    // Refuse The Call Across Plugin Owners
    let error = host.runtime.operations.call_encoded("fixture.forward", payload(b"secret")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), Classification::Refused);
    assert!(call_error(&error).to_string().contains("example.plugin.v1.Lifecycle/Initialize"));

    // The Host Can Still Invoke That Protected Operation
    let input = payload(b"host access");
    assert_eq!(host.runtime.operations.call_encoded("example.plugin.v1.Lifecycle/Initialize", input.clone()).await.unwrap(), input);
    // The Declaring Plugin Also Calls It Through the Host Carrier.
    assert_eq!(host.runtime.operations.call_encoded("fixture.self", input.clone()).await.unwrap(), input);
    host.runtime.shutdown().await.unwrap();
}
