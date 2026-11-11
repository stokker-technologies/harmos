/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Operation Retry Uses the Host Clock

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
async fn retry_preserves_the_request_and_waits_on_the_host_clock() {
    // Connect An Operation That Succeeds On Its Third Attempt
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("retry").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // The Operation's Policy Owns Both Retries And Their Delay
    let input = super::invoke_support::payload(b"retained request");
    let call = host.runtime.operations.submit_encoded("fixture.retry", input.clone()).unwrap();
    let id = call.id();
    let advancing = controller.advance_until(std::time::Duration::from_secs(10), || host.runtime.operations.status(id).is_none());
    let (answer, advanced) = tokio::join!(call, advancing);
    advanced.unwrap();
    assert_eq!(answer.unwrap(), input);
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(4));

    // Stop The Plugin After Settlement
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
