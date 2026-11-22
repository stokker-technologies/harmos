/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Publication Demand Reopens After Reconnect

/* === Tests === */

#[tokio::test]
async fn publish_reopens_with_a_fresh_sequence_after_connection_loss() {
    // Capture One Interval Answer
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock)).await;
    *host.signals.publications.lock().unwrap() = Some(Vec::new());
    let sdk = host.sdk("interval").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();
    let _watching = host.watch("fixture.interval");
    let mut status = attached.status();
    controller.advance_until(std::time::Duration::from_secs(10), || host.rows().len() == 1).await.unwrap();
    let old = attached.activation().unwrap();
    let original = host.signals.publications.lock().unwrap().as_ref().unwrap().first().unwrap().clone();
    assert_eq!(original.1.generation(), Some(old.number));
    assert_eq!(original.1.owner(), original.0.owner().unwrap());
    assert_eq!(original.1.schema(), original.0.version());
    let mut feed = host.runtime.streams.follow(harmos_runtime::storage::Cursor::ORIGIN);
    let delayed = feed.next().await.unwrap();
    drop(feed);

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
        host.samples().into_iter().map(|(_, row)| row).collect::<Vec<_>>(),
        [serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()}), serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()})]
    );
    assert_eq!(host.rows().iter().map(|row| row.published_unix_micros).collect::<Vec<_>>(), [Some(1_000_000), Some(3_000_000)]);
    let restarted = host.signals.publications.lock().unwrap().as_ref().unwrap().last().unwrap().clone();
    assert_eq!(restarted.1.generation(), Some(attached.activation().unwrap().number));
    assert_ne!(restarted.1.generation(), original.1.generation());
    assert_eq!(restarted.1.owner(), original.1.owner());
    assert_eq!(restarted.1.schema(), restarted.0.version());
    assert_eq!(original.1.generation(), Some(old.number));
    let (definition, _, delivered) = delayed.publications().find(|(definition, _, _)| definition.id() == original.0.id()).unwrap();
    assert_eq!(delivered.value.owner(), original.1.owner());
    assert_eq!(delivered.value.generation(), Some(old.number));
    assert_eq!(delivered.value.schema(), original.1.schema());
    assert_eq!(definition.schema(), original.0.schema());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
