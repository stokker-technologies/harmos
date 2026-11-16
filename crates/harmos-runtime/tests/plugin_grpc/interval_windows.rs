/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Interval Windows: Exact Boundaries, Per-Stream Lengths, Message Splitting and One Schedule per Stream

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::{Duration, SystemTime};

// Third Party Imports

// Local Imports
use super::{invoke_support::HostFixture, support};
use harmos_runtime::{
    clock::{Clock, Source as _},
    plugin::grpc::{Placement, PlacementSettings},
};

/* === Constants === */

const SECOND: i64 = 1_000_000_000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/// The Host's Window Length Is Four Seconds; a Stream's Own Setting May Replace It.
fn settings() -> PlacementSettings {
    PlacementSettings {
        interval_window: Duration::from_secs(4),
        ..support::settings()
    }
}

fn sequences(host: &HostFixture, stream: &str) -> Vec<u64> {
    host.samples_of(stream)
        .into_iter()
        .map(|(_, row)| {
            let bytes = row["value"].as_array().unwrap().iter().map(|byte| byte.as_u64().unwrap() as u8).collect::<Vec<_>>();
            u64::from_le_bytes(bytes.try_into().unwrap())
        })
        .collect()
}

/* === Tests === */

#[tokio::test]
async fn windows_cover_exact_sample_ranges_on_a_controlled_clock() {
    // One-Second Samples: the Host Grants Four-Second Windows, the Plugin's Setting Three-Second Ones.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("windows").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), settings()).await.unwrap();
    let _watching = (host.watch("fixture.counts"), host.watch("fixture.override"));

    // Twelve Seconds Hold Three Host Windows and Four Overridden Windows, Each Stored Whole.
    controller.advance_until(Duration::from_secs(30), || host.batches_of("fixture.counts").len() == 3).await.unwrap();
    controller.advance_until(Duration::from_secs(30), || host.batches_of("fixture.override").len() == 4).await.unwrap();
    assert_eq!(clock.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(12), "End Each Window on Its Grid");
    let starts = |stream| host.batches_of(stream).into_iter().map(|(start, count, _)| (start, count)).collect::<Vec<_>>();
    assert_eq!(starts("fixture.counts"), [(SECOND, 4), (5 * SECOND, 4), (9 * SECOND, 4)]);
    assert_eq!(starts("fixture.override"), [(SECOND, 3), (4 * SECOND, 3), (7 * SECOND, 3), (10 * SECOND, 3)]);

    // A Step Short of the Next Window End Grants Nothing; Reaching It Grants Exactly That Window.
    controller.advance_by(Duration::from_secs(2)).await;
    assert_eq!(host.batches_of("fixture.counts").len(), 3, "Grant No Window Before Its End");
    controller.advance_until(Duration::from_secs(30), || host.batches_of("fixture.counts").len() == 4).await.unwrap();
    assert_eq!(starts("fixture.counts")[3], (13 * SECOND, 4));

    // Every Sample Appears Once, on the Fixed Grid, With Consecutive Sequences.
    let samples = host.samples_of("fixture.counts");
    assert_eq!(samples.iter().map(|(at, _)| *at).collect::<Vec<_>>(), (1..=16).map(|second| second * SECOND).collect::<Vec<_>>());
    assert_eq!(sequences(&host, "fixture.counts"), (1..=16).collect::<Vec<_>>());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_window_larger_than_one_message_arrives_in_contiguous_parts() {
    // Two Million One-Microsecond Samples Exceed the Plugin's Maximum Message Size.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("split").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), settings()).await.unwrap();
    let _watching = host.watch("fixture.split");
    let total = |host: &HostFixture| host.batches_of("fixture.split").iter().map(|(_, count, _)| count).sum::<u64>();
    controller.advance_until(Duration::from_secs(5), || total(&host) >= 2_000_000).await.unwrap();

    // The Parts Share One Window: Each Starts Where the Previous Ended, and Together They Hold It Exactly.
    let batches = host.batches_of("fixture.split");
    assert!(batches.len() > 1, "Split the Window at the Message Size");
    let mut next = 1_000;
    for (start, count, _) in &batches {
        assert_eq!(*start, next, "Start Each Part Where the Previous Part Ended");
        next += *count as i64 * 1_000;
    }
    assert_eq!(total(&host), 2_000_000, "Store Exactly the Granted Samples");
    let values = host.values();
    let first = &values[0]["columns"]["value"];
    let last = &values[values.len() - 1]["columns"]["value"];
    assert_eq!(first[0].as_f64(), Some(1.0));
    assert_eq!(last[last.as_array().unwrap().len() - 1].as_f64(), Some(2_000_000.0));
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_sampling_scope_pauses_the_unscoped_schedule_without_duplicate_windows() {
    // The Activation's Unscoped Demand and One Scope Each Run on Their Own Clock.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("windows").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), settings()).await.unwrap();
    let _watching = host.watch("fixture.counts");
    let (unit_clock, unit_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000));
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/windows", unit_clock).await.unwrap();
    let scoped = |host: &HostFixture| host.batches_of("fixture.counts").into_iter().filter(|(_, _, scope)| scope.is_some()).count();
    let unscoped = |host: &HostFixture| {
        host.batches_of("fixture.counts")
            .into_iter()
            .filter(|(_, _, scope)| scope.is_none())
            .map(|(start, count, _)| (start, count))
            .collect::<Vec<_>>()
    };

    // While the Scope Samples, Runtime Time Passes Without Unscoped Windows.
    unit.start_samples().await.unwrap();
    controller.advance_by(Duration::from_secs(20)).await;
    assert!(unscoped(&host).is_empty(), "Grant No Unscoped Window While a Scope Samples");
    unit_time.advance_until(Duration::from_secs(30), || scoped(&host) == 2).await.unwrap();
    let windows = host.batches_of("fixture.counts").into_iter().map(|(start, count, _)| (start, count)).collect::<Vec<_>>();
    assert_eq!(windows, [(1_001 * SECOND, 4), (1_005 * SECOND, 4)], "Grant the Scope Its Own Windows Only");

    // Stopping the Scope Resumes the Unscoped Schedule From a Fresh Origin.
    unit.stop_samples().await.unwrap();
    controller.advance_until(Duration::from_secs(30), || unscoped(&host).len() == 2).await.unwrap();
    assert_eq!(unscoped(&host), [(21 * SECOND, 4), (25 * SECOND, 4)], "Resume at the Instant the Last Scope Stopped");
    assert_eq!(scoped(&host), 2);
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_controlled_step_longer_than_the_window_is_one_window() {
    // A Scope Samples on a Clock That Jumps Straight to Each Advanced Instant.
    let host = HostFixture::new().await;
    let sdk = host.sdk("windows").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), settings()).await.unwrap();
    let unit_time = support::Manual::new();
    let origin = unit_time.read().instant;
    let origin_date = unit_time.read().date.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos() as i64;
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/step", Clock::with_clock(unit_time.clone())).await.unwrap();
    unit.start_samples().await.unwrap();
    unit_time.registered_at(origin + Duration::from_secs(4)).await;

    // One Ten-Second Step Past the Four-Second Window End Grants One Window of Ten Samples.
    unit_time.advance_by(Duration::from_secs(10));
    loop {
        let ingested = host.signals.storage_ingested.notified();
        tokio::pin!(ingested);
        ingested.as_mut().enable();
        if !host.batches_of("fixture.counts").is_empty() {
            break;
        }
        ingested.await;
    }
    let expected = [(origin_date + SECOND, 10, Some("unit/step".to_owned()))];
    assert_eq!(host.batches_of("fixture.counts"), expected, "Cover Exactly the Advanced Step");
    unit.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
