/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Independent Stream Intervals

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
async fn independent_intervals_keep_each_streams_order_and_host_time() {
    // Connect Two Observers On One Virtual Host
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock)).await;
    let sdk = host.sdk("twins").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Advance Until Both Independent Streams Have Answered
    controller
        .advance_until(std::time::Duration::from_secs(10), || {
            let stored = host.stored.lock().unwrap();
            stored.iter().filter(|(definition, _)| definition.id() == "fixture.left").count() >= 4
                && stored.iter().filter(|(definition, _)| definition.id() == "fixture.right").count() >= 2
        })
        .await
        .unwrap();
    let stored = host.stored.lock().unwrap().clone();
    for (id, interval, count) in [("fixture.left", 1_000_000, 4), ("fixture.right", 2_000_000, 2)] {
        let rows: Vec<_> = stored.iter().filter(|(definition, _)| definition.id() == id).collect();
        assert_eq!(rows.len(), count);
        for (index, (definition, row)) in rows.into_iter().enumerate() {
            assert_eq!(row.published_unix_micros, Some((index as i64 + 1) * interval));
            let codec = harmos_domain::schema::json::JsonCodec::new(definition.schema().root().unwrap(), &Default::default()).unwrap();
            assert_eq!(codec.decode(&row.payload).unwrap(), serde_json::json!({"value": (index as u64 + 1).to_le_bytes().to_vec()}));
        }
    }

    // An Idle Observer Leaves Operations Available
    let input = super::invoke_support::payload(b"still available");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
