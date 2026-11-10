/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Waits Share Virtual Host Time

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
async fn plugin_sleep_and_now_share_the_virtual_host_clock() {
    // Connect An SDK Plugin To A Virtual Host
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("caller").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Advance Only Through The Plugin's Host Sleep
    let call = host.runtime.operations.submit_encoded("fixture.clock", super::invoke_support::payload(b"clock")).unwrap();
    let id = call.id();
    let drive = controller.advance_until(std::time::Duration::from_secs(7200), || host.runtime.operations.status(id).is_none());
    let (result, advanced) = tokio::join!(call, drive);
    advanced.unwrap();

    // The Two Readings Bracket Exactly One Hour
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3600));
    assert_eq!(result.unwrap(), super::invoke_support::payload(&3600_i64.to_le_bytes()));
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
