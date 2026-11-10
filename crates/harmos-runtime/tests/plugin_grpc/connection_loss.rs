/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection Loss Fails In-Flight Calls

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
async fn dropped_connection_fails_only_its_in_flight_calls() {
    // Connect Two Independent SDK Plugins
    let host = HostFixture::new().await;
    let caller = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();
    let peer = host.sdk("provider").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();

    // Drop The Caller's Connection After Its Operation Starts
    let invocation = host.runtime.operations.submit_encoded("fixture.wait", payload(b"started")).unwrap();
    host.signals.entered.notified().await;
    caller.disconnect().await;
    let error = invocation.await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Transient);

    // Disconnected Operations Leave The Registry
    let mut status = attached.status();
    support::offline(&mut status).await;
    assert!(host.runtime.operations.definition("fixture.wait").is_err());

    // Other Plugins And Host Operations Remain Available
    let input = payload(b"public");
    assert_eq!(host.runtime.operations.call_encoded("fixture.public", input.clone()).await.unwrap(), input);
    let response = host.runtime.operations.submit::<Host>(Host { value: b"host".to_vec() }).unwrap().await.unwrap();
    assert_eq!(response.value, b"host");
    // A Replacement Connection Registers Fresh Operations
    host.clock.advance_next().await;
    let _replacement = support::activation(&mut status, 2).await;
    let input = payload(b"reconnected");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    host.runtime.shutdown().await.unwrap();
}
