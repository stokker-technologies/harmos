/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Concurrent Scoped Publications and Clocks

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::{Duration, SystemTime};

// Third Party Imports

// Local Imports
use super::{
    invoke_support::{HostFixture, payload, pending},
    support,
};
use harmos_runtime::{clock::Clock, plugin::grpc::Placement};

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
async fn concurrent_units_retain_their_clocks_samples_and_close_boundary() {
    // Attach Once Before Either Scoped Unit Exists.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("scoped-publications").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let _watching = host.watch("fixture.scoped_samples");
    let activation = attached.activation().unwrap();
    let mut status = attached.status();

    // Bind Two Independent Timelines to the Existing Activation.
    let (left_clock, left_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let (right_clock, right_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(100));
    let left = host.runtime.bind_grpc(&activation, "unit/left", left_clock.clone()).await.unwrap();
    left.start_samples(["fixture.scoped_samples"]).await.unwrap();
    let right = host.runtime.bind_grpc(&activation, "unit/right", right_clock.clone()).await.unwrap();
    right.start_samples(["fixture.scoped_samples"]).await.unwrap();
    assert!(host.runtime.bind_grpc(&activation, "unit/left", left_clock.clone()).await.is_err());
    let left_call = left.operations().submit_encoded("fixture.clock", payload(b"clock")).unwrap();
    let right_call = right.operations().submit_encoded("fixture.clock", payload(b"clock")).unwrap();
    let left_id = left_call.id();
    let right_id = right_call.id();
    tokio::pin!(left_call, right_call);
    let rows_for = |scope: &str| {
        host.rows()
            .into_iter()
            .filter(|row| {
                row.context
                    .as_ref()
                    .and_then(|context| serde_json::from_str::<serde_json::Value>(context).ok())
                    .is_some_and(|context| context["scope"] == scope)
            })
            .collect::<Vec<_>>()
    };

    // Keep Left Time Still Until Its Accepted Sample Is Ingested.
    host.signals.storage_hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let advancing = left_time.advance_until(Duration::from_secs(5), || !rows_for("unit/left").is_empty());
    tokio::pin!(advancing);
    tokio::select! {
        result = &mut advancing => panic!("Ingestion Is Still Held: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }
    assert!(pending(advancing.as_mut()).await);
    // Held Scoped Ingestion Is Host Work: the Runtime Clock Cannot Settle Either.
    let settling = controller.advance_by(Duration::ZERO);
    tokio::pin!(settling);
    assert!(pending(settling.as_mut()).await, "Hold the Runtime Clock During Accepted Scoped Ingestion");
    assert!(pending(left_call.as_mut()).await);
    assert!(pending(right_call.as_mut()).await);
    assert_eq!(left_clock.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(1));
    assert!(rows_for("unit/right").is_empty());
    assert_eq!(right_clock.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(100));

    // Releasing Ingestion Lets Only Left's Sleep Complete on Left Time.
    host.signals.storage_release.notify_one();
    advancing.await.unwrap();
    settling.await;
    let drive = left_time.advance_until(Duration::from_secs(5), || left.operations().status(left_id).is_none());
    let (result, advanced) = tokio::join!(left_call, drive);
    advanced.unwrap();
    assert_eq!(result.unwrap(), payload(&3_i64.to_le_bytes()));
    assert!(pending(right_call.as_mut()).await);
    assert_eq!(right_clock.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(100));

    // Hold the Next Accepted Sample While Closing Left's Demand.
    let before_close = rows_for("unit/left").len();
    host.signals.storage_hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let advancing = left_time.advance_until(Duration::from_secs(5), || rows_for("unit/left").len() > before_close);
    tokio::pin!(advancing);
    tokio::select! {
        result = &mut advancing => panic!("Close Ingestion Is Still Held: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }
    let left_operations = left.operations().clone();
    let cancelled = left.operations().submit_encoded("fixture.clock", payload(b"clock")).unwrap();
    let closing = left.close();
    tokio::pin!(closing);
    assert!(pending(closing.as_mut()).await);
    host.signals.storage_release.notify_one();
    let (closed, advanced) = tokio::join!(closing, advancing);
    closed.unwrap();
    advanced.unwrap();
    assert!(cancelled.await.is_err());
    assert!(pending(right_call.as_mut()).await);
    let left_rows = rows_for("unit/left");
    let left_closed_at = left_clock.date();
    assert_eq!(left_rows.len(), before_close + 1);
    assert_eq!(left_rows[0].published_unix_micros, Some(1_000_000));
    assert_eq!(host.samples()[0].1, serde_json::json!({ "value": b"unit/left:1".to_vec() }));
    assert!(left_operations.call_encoded("fixture.clock", payload(b"clock")).await.is_err());

    // Right Alone Settles Its Plugin Sleep; the Runtime and Left Clocks Stay Still.
    let drive = right_time.advance_until(Duration::from_secs(10), || right.operations().status(right_id).is_none());
    let (result, advanced) = tokio::join!(right_call, drive);
    advanced.unwrap();
    assert_eq!(result.unwrap(), payload(&3_i64.to_le_bytes()));
    assert!(!rows_for("unit/right").is_empty());
    assert!(host.samples().iter().any(|(_, row)| *row == serde_json::json!({ "value": b"unit/right:101".to_vec() })));
    assert_eq!(rows_for("unit/left").len(), left_rows.len());
    assert_eq!(left_clock.date(), left_closed_at);
    assert_eq!(clock.date(), SystemTime::UNIX_EPOCH);
    right.close().await.unwrap();

    // The Original Unscoped Demand Continues on the Runtime Clock.
    controller.advance_until(Duration::from_secs(5), || host.rows().iter().any(|row| row.context.is_none())).await.unwrap();
    assert!(activation.is_connected());
    let dropped = host.runtime.bind_grpc(&activation, "unit/dropped", left_clock.clone()).await.unwrap();
    dropped.start_samples(["fixture.scoped_samples"]).await.unwrap();
    drop(dropped);

    // A Lost Connection Still Settles the Scope's Already Accepted Ingestion.
    let rebound = host.runtime.bind_grpc(&activation, "unit/left", left_clock.clone()).await.unwrap();
    rebound.start_samples(["fixture.scoped_samples"]).await.unwrap();
    let previous = rows_for("unit/left").len();
    host.signals.storage_hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let delivering = left_time.advance_until(Duration::from_secs(5), || rows_for("unit/left").len() > previous);
    tokio::pin!(delivering);
    tokio::select! {
        result = &mut delivering => panic!("Disconnected Ingestion Is Still Held: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }
    let network = clock.activity("Await Scoped Disconnect");
    sdk.disconnect().await;
    controller.advance_until(Duration::from_secs(5), || !activation.is_connected()).await.unwrap();
    drop(network);
    let closing = rebound.close();
    tokio::pin!(closing);
    assert!(pending(closing.as_mut()).await);
    host.signals.storage_release.notify_one();
    let (closed, delivered) = tokio::join!(closing, delivering);
    assert!(closed.unwrap_err().to_string().contains("Close Scoped Samples"));
    delivered.unwrap();
    assert_eq!(rows_for("unit/left").len(), previous + 1);

    // Retain the Cleanup Failure While the Remote Attachment Is Disconnected.
    support::offline(&mut status).await;
    assert!(status.borrow().failure.as_ref().unwrap().to_string().contains("Close Scoped Samples"));
    assert!(!activation.is_connected());

    // Reconnect and Bind Fresh Samples After the Old Scope Has Settled.
    controller
        .advance_until(Duration::from_secs(5), || attached.activation().is_some_and(|next| next.number > activation.number))
        .await
        .unwrap();
    let reconnected = attached.activation().unwrap();
    assert_eq!(reconnected.number, activation.number + 1);
    assert!(status.borrow().failure.is_none());
    let resumed = host.runtime.bind_grpc(&reconnected, "unit/left", left_clock).await.unwrap();
    resumed.start_samples(["fixture.scoped_samples"]).await.unwrap();
    let previous = rows_for("unit/left").len();
    left_time.advance_until(Duration::from_secs(5), || rows_for("unit/left").len() > previous).await.unwrap();
    resumed.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
