/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host-Ticked Samples Reach Stream Storage

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
async fn host_ticks_are_stored_in_order_with_host_timestamps() {
    // Admit An SDK Observer On A Virtual Host
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    host.signals.storage_hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let sdk = host.sdk("interval").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Keep Virtual Time Still While Storage Work Is Outstanding
    let advancing = controller.advance_until(std::time::Duration::from_secs(10), || host.rows().len() >= 3);
    tokio::pin!(advancing);
    tokio::select! {
        result = &mut advancing => panic!("Storage Has Not Been Released: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }
    assert!(super::invoke_support::pending(advancing.as_mut()).await);
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1));

    // Read Three Answers Through The Existing Storage Consumer
    host.signals.storage_release.notify_one();
    advancing.await.unwrap();
    assert_eq!(
        host.values(),
        (1_u64..=3).map(|sequence| serde_json::json!({ "value": sequence.to_le_bytes().to_vec() })).collect::<Vec<_>>()
    );
    assert_eq!(
        host.rows().iter().map(|row| row.published_unix_micros).collect::<Vec<_>>(),
        [Some(1_000_000), Some(2_000_000), Some(3_000_000)]
    );
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3));

    // A Stream Awaiting A Tick Leaves Invoke Usable
    let input = super::invoke_support::payload(b"echo");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    attached.stop("Story Complete").await.unwrap();
    controller.advance_by(std::time::Duration::from_secs(10)).await;
    assert_eq!(host.rows().len(), 3);
    host.runtime.shutdown().await.unwrap();
}
