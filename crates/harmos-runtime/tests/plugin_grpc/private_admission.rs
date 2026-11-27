/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Private Operation Admits Only Its Declaring Plugin, Through the Host.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, time::Duration};

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_conformance::host::invocations_client::InvocationsClient;
use harmos_protocol::host::InvocationsCreateRequest;
use harmos_protocol::model::{Classification, Visibility};
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
async fn private_calls_use_host_admission_and_retry_scheduling() {
    // Declare the Private Operation Beside a Peer That Requires It
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("private").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let peer = host.sdk("private-peer").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let declarations = activation.description.declarations.as_ref().unwrap();
    let private = declarations.operations.iter().find(|operation| operation.id == "fixture.private").unwrap();
    assert_eq!(private.visibility(), Visibility::Private);

    // The Declaring Plugin's Call Crosses the Host and Retries on the Host Clock
    let input = payload(b"private request");
    let call = host.runtime.operations.submit_encoded("fixture.own", input.clone()).unwrap();
    let id = call.id();
    let advancing = controller.advance_until(Duration::from_secs(10), || host.runtime.operations.status(id).is_none());
    let (answer, advanced) = tokio::join!(call, advancing);
    advanced.unwrap();
    assert_eq!(answer.unwrap(), input);
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(4));

    // Host-Side Callers and Other Plugins Are Refused
    assert!(matches!(
        host.runtime.operations.submit_encoded("fixture.private", input.clone()),
        Err(harmos_runtime::Error::OperationUnauthorized { .. })
    ));
    let error = host.runtime.operations.call_encoded("fixture.peer_private", input.clone()).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), Classification::Refused);
    assert!(call_error(&error).to_string().contains("fixture.private"), "{error}");

    // A Client Knowing the Fingerprint Is Refused Without a Client-Facing Method
    host.runtime.operations.register_catalog(harmos_conformance::product::catalog(), Default::default()).await.unwrap();
    let listener = host
        .runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            operations: Default::default(),
            reflection: false,
            address: "127.0.0.1:0".parse().unwrap(),
            tls: None,
            connections: NonZeroUsize::new(4).unwrap(),
            calls_per_connection: NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let request = InvocationsCreateRequest {
        operation: private.id.clone(),
        fingerprint: private.fingerprint.clone(),
        payload: input,
    };
    let error = InvocationsClient::new(channel).create(request).await.unwrap_err();
    assert!(error.to_string().contains("fixture.private"), "{error}");

    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
