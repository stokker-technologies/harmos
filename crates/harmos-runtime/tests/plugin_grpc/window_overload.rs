/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Overload and Plugin Window Failures Fail Their Demand; the Unscoped Demand Samples Only While Watched

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, SystemTime},
};

// Third Party Imports
use tokio::sync::Notify;
use tracing_subscriber::layer::SubscriberExt as _;

// Local Imports
use super::{invoke_support::HostFixture, support};
use harmos_runtime::{
    clock::{Clock, Source as _},
    plugin::grpc::{Attached, Error, IntervalLag, Placement, PlacementSettings},
};

/* === Constants === */

const SECOND: i64 = 1_000_000_000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Notifies When a Warning Is Recorded on the Story's Thread, Such as a Demand Ending With a Failure.
struct Warned(Arc<Notify>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warned {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if *event.metadata().level() == tracing::Level::WARN {
            self.0.notify_one();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn windows(length: Duration) -> PlacementSettings {
    PlacementSettings {
        interval_window: length,
        ..support::settings()
    }
}

/// Wait Until Storage Holds at Least `count` Samples of `stream`.
async fn stored(host: &HostFixture, stream: &str, count: usize) {
    loop {
        let ingested = host.signals.storage_ingested.notified();
        tokio::pin!(ingested);
        ingested.as_mut().enable();
        if host.samples_of(stream).len() >= count {
            return;
        }
        ingested.await;
    }
}

/// The Failure the Attachment Reports for One of Its Unscoped Demands.
async fn failure(attached: &Attached) -> Arc<Error> {
    let mut status = attached.status();
    let status = status.wait_for(|status| status.failure.is_some()).await.unwrap();
    status.failure.clone().unwrap()
}

/* === Tests === */

#[tokio::test]
async fn a_window_past_the_lag_limit_fails_its_demand_and_a_backlog_within_it_keeps_every_sample() {
    // One-Second Samples in Four-Second Windows That May End at Most Two Windows Late, on a Clock That Jumps.
    let host = HostFixture::new().await;
    let sdk = host.sdk("interval").await;
    let settings = PlacementSettings {
        interval_lag: IntervalLag::new(NonZeroU32::new(2).unwrap()),
        ..windows(Duration::from_secs(4))
    };
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), settings).await.unwrap();
    let origin = host.clock.read().instant;
    let watching = host.watch("fixture.interval");
    host.clock.registered_at(origin + Duration::from_secs(4)).await;

    // Eight Seconds Past the Window End Is Within the Limit: One Window Takes the Whole Backlog.
    host.clock.advance_by(Duration::from_secs(12));
    stored(&host, "fixture.interval", 12).await;
    let counts = host.batches_of("fixture.interval").into_iter().map(|(_, count, _)| count).collect::<Vec<_>>();
    assert_eq!(counts, [12], "Keep Every Sample of a Backlog Within the Limit");

    // Nine Seconds Past the Next End Is Beyond It: the Demand Fails, Naming Its Stream, and Grants Nothing.
    host.clock.registered_at(origin + Duration::from_secs(16)).await;
    host.clock.advance_by(Duration::from_secs(13));
    let failure = failure(&attached).await;
    assert!(
        matches!(failure.as_ref(), Error::Lagging { stream, lag, limit } if stream == "fixture.interval" && lag > limit),
        "Fail a Demand Whose Window Ends Past the Lag Limit: {failure}"
    );
    assert_eq!(host.samples_of("fixture.interval").len(), 12, "Grant No Window Past the Limit");
    assert!(attached.activation().is_some(), "Keep the Plugin Attached While One Stream Fails");
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_full_publication_pool_fails_the_demand_with_its_stream_and_limits() {
    // One Seventy-Two-Second Window Holds 72 One-Mebibyte Samples, More Than the Shared Pool Retains.
    let host = HostFixture::new().await;
    let sdk = host.sdk("bulky").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), windows(Duration::from_secs(72))).await.unwrap();
    let origin = host.clock.read().instant;
    let watching = host.watch("fixture.bulky");
    host.clock.registered_at(origin + Duration::from_secs(72)).await;

    // Storage Drains Nothing While the Window Is Admitted, So Every Admitted Row Stays in the Pool.
    host.clock.advance_by(Duration::from_secs(72));

    // The Owner Reads Which Stream Filled the Pool and Its Limits; Other Streams Would Keep Publishing.
    let failure = failure(&attached).await;
    assert!(
        matches!(failure.as_ref(), Error::PoolFull { stream, .. } if stream == "fixture.bulky"),
        "Fail the Demand That Filled the Pool: {failure}"
    );
    assert!(attached.activation().is_some(), "Keep the Plugin Attached While One Stream Fails");
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_window_the_empty_pool_cannot_hold_is_refused_before_sampling() {
    // Ten-Second Windows of One-Microsecond Readings Need Eighty Megabytes Even at Their Smallest.
    let host = HostFixture::new().await;
    let sdk = host.sdk("dense").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), windows(Duration::from_secs(10))).await.unwrap();
    let origin = host.clock.read().instant;
    let watching = host.watch("fixture.dense");
    host.clock.registered_at(origin + Duration::from_secs(10)).await;
    host.clock.advance_by(Duration::from_secs(10));

    // The Host Refuses the Grant Itself: the Plugin Samples Nothing and Storage Holds Nothing.
    let failure = failure(&attached).await;
    assert!(
        matches!(failure.as_ref(), Error::WindowExceedsPool { stream, samples, .. } if stream == "fixture.dense" && *samples == 10_000_000),
        "Refuse a Window Larger Than the Pool: {failure}"
    );
    assert!(host.batches_of("fixture.dense").is_empty(), "Sample Nothing for a Refused Window");
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_window_the_plugin_fails_names_its_stream_and_the_plugins_status() {
    // One-Second Samples in Four-Second Windows Whose Observer Always Fails.
    let host = HostFixture::new().await;
    let sdk = host.sdk("failing").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), windows(Duration::from_secs(4))).await.unwrap();
    let origin = host.clock.read().instant;
    let watching = host.watch("fixture.failing");
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));

    // The Owner Reads Which Stream Failed and the Plugin's Own Status and Message; the Plugin Stays Attached.
    let failure = failure(&attached).await;
    assert!(
        matches!(
            failure.as_ref(),
            Error::WindowFailed { stream, status: tonic::Code::Internal, message } if stream == "fixture.failing" && message == "Sensor Offline"
        ),
        "Carry the Failed Window's Stream and the Plugin's Status: {failure:?}"
    );
    assert!(attached.activation().is_some(), "Keep the Plugin Attached While One Stream Fails");
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_scope_reads_the_plugins_failed_window_from_stop_samples_and_close() {
    // Observe This Current-Thread Story's Warnings; the Unobserved Dispatcher Keeps Parallel Callsites Enabled.
    let warned = Arc::new(Notify::new());
    let _unobserved = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _observed = tracing::subscriber::set_default(tracing_subscriber::registry().with(Warned(warned.clone())));

    // A Scope Samples a Stream Whose Observer Always Fails; Nothing Watches It, So Only the Scope Samples.
    let host = HostFixture::new().await;
    let sdk = host.sdk("failing").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), windows(Duration::from_secs(4))).await.unwrap();
    let unit_time = support::Manual::new();
    let unit = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/failing", Clock::with_clock(unit_time.clone())).await.unwrap();
    let failed = |error: &Error| {
        matches!(
            error,
            Error::WindowFailed { stream, status: tonic::Code::Internal, message } if stream == "fixture.failing" && message == "Sensor Offline"
        )
    };

    // The Failed Window Ends the Scope's Demand, and Stopping Its Samples Returns the Failure.
    unit.start_samples(["fixture.failing"]).await.unwrap();
    unit_time.registered_at(unit_time.read().instant + Duration::from_secs(4)).await;
    unit_time.advance_by(Duration::from_secs(4));
    warned.notified().await;
    let stopped = unit.stop_samples().await.unwrap_err();
    assert!(failed(&stopped), "Return the Failed Window From stop_samples(): {stopped:?}");

    // Started Again, the Next Failure Comes Back From close().
    unit.start_samples(["fixture.failing"]).await.unwrap();
    unit_time.registered_at(unit_time.read().instant + Duration::from_secs(4)).await;
    unit_time.advance_by(Duration::from_secs(4));
    warned.notified().await;
    let closed = unit.close().await.unwrap_err();
    assert!(failed(&closed), "Return the Failed Window From close(): {closed:?}");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn the_unscoped_demand_samples_only_while_its_stream_is_watched() {
    // One-Second Samples in Four-Second Windows on a Controlled Clock.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("interval").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), windows(Duration::from_secs(4))).await.unwrap();
    let starts =
        |host: &HostFixture| host.batches_of("fixture.interval").into_iter().map(|(start, count, _)| (start, count)).collect::<Vec<_>>();

    // Attached but Unwatched, Twenty Seconds Pass Without a Window.
    controller.advance_by(Duration::from_secs(20)).await;
    assert!(starts(&host).is_empty(), "Grant No Window While Nothing Watches the Stream");

    // A Watcher Starts the Schedule From the Instant It Appeared.
    let watching = host.watch("fixture.interval");
    controller.advance_until(Duration::from_secs(30), || starts(&host).len() == 2).await.unwrap();
    assert_eq!(starts(&host), [(21 * SECOND, 4), (25 * SECOND, 4)], "Start Windows When a Watcher Appears");

    // When the Last Watcher Leaves, the Schedule Stops Again.
    drop(watching);
    controller.advance_by(Duration::from_secs(20)).await;
    assert_eq!(starts(&host).len(), 2, "Stop Granting Windows When the Last Watcher Leaves");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
