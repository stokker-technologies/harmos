/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Active Plugin Calls Hold Virtual Time Still

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
async fn an_in_flight_plugin_call_blocks_virtual_time_until_settlement() {
    // Start A Plugin Call That Waits For Cooperative Cancellation
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("caller").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();
    let call = host.runtime.operations.submit_encoded("fixture.wait", super::invoke_support::payload(b"pending")).unwrap();
    host.signals.entered.notified().await;

    // A Registered Timer Cannot Overtake Work In Flight
    let timer = clock.sleep(std::time::Duration::from_secs(1));
    let advancing = controller.advance_next();
    tokio::pin!(advancing);
    assert!(super::invoke_support::pending(advancing.as_mut()).await);
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH);

    // Settling The Call Lets The Same Controller Advance
    call.request_cancel();
    let drive = async {
        tokio::join!(advancing, timer);
    };
    let (answer, ()) = tokio::join!(call, drive);
    assert!(answer.is_ok());
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1));
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
