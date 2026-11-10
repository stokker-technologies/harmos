/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Publication Demand Reopens After Reconnect

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
async fn publish_reopens_with_a_fresh_sequence_after_connection_loss() {
    // Capture One Interval Answer
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock)).await;
    let sdk = host.sdk("interval").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();
    let mut status = attached.status();
    controller.advance_until(std::time::Duration::from_secs(10), || host.rows().len() == 1).await.unwrap();
    let old = attached.activation().unwrap();

    // Drop The Physical Connection And Advance Its Reconnect Timer
    sdk.disconnect().await;
    super::support::offline(&mut status).await;
    controller
        .advance_until(std::time::Duration::from_secs(10), || attached.activation().is_some_and(|activation| activation.number == 2))
        .await
        .unwrap();
    assert!(!old.is_connected());

    // A Reopened Demand Starts At Sequence One
    controller.advance_until(std::time::Duration::from_secs(10), || host.rows().len() >= 2).await.unwrap();
    assert_eq!(
        host.values(),
        [serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()}), serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()})]
    );
    assert_eq!(host.rows().iter().map(|row| row.published_unix_micros).collect::<Vec<_>>(), [Some(1_000_000), Some(3_000_000)]);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
