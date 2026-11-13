/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cancellation Reaches the SDK Plugin

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;

/* === Tests === */

#[tokio::test]
async fn cancellation_reaches_the_plugin_before_settlement() {
    // Connect A Cooperative SDK Operation
    let host = HostFixture::new().await;
    let sdk = host.sdk("caller").await;
    let _attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();

    // Wait For Its Child Call To Confirm The Handler Is Running
    let input = payload(b"started");
    let invocation = host.runtime.operations.submit_encoded("fixture.wait", input.clone()).unwrap();
    host.signals.entered.notified().await;

    // Cancel And Observe The Plugin's Final Response
    invocation.request_cancel();
    let response = invocation.await.unwrap();
    assert_eq!(response, input);

    // Cancellation Leaves The Connection Usable
    let input = payload(b"after cancellation");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    host.runtime.shutdown().await.unwrap();
}
