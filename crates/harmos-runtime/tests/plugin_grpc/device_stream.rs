/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Device Samples Preserve Host and Source Time

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
async fn device_samples_need_no_tick_and_keep_source_time_separate() {
    // Open Device Demand Without Advancing Host Time
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("device").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Await Storage Delivery Through Its Publication Watch
    controller.advance_until(std::time::Duration::from_secs(1), || host.rows().len() >= 2).await.unwrap();
    assert_eq!(host.values(), [serde_json::json!({"value": b"device"}), serde_json::json!({"value": b"source"})]);
    assert_eq!(host.rows().iter().map(|row| row.published_unix_micros).collect::<Vec<_>>(), [Some(0), Some(0)]);
    assert_eq!(host.rows()[0].context, None);
    assert_eq!(host.rows()[1].context.as_deref(), Some("{\"source_time\":{\"seconds\":7,\"nanos\":0}}"));
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH);

    // Stop Closes Demand Before The SDK Stop Hook Replies
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
