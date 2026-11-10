/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Remote Stop Grace on the Host Clock

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Manual, Remote};
use harmos_runtime::{
    clock::Clock,
    plugin::grpc::{Error, Placement},
};

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
async fn an_unanswered_stop_closes_at_the_host_clock_grace_and_records_failure() {
    // Attach A Remote Plugin On A Controlled Host Clock
    let source = Manual::new();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = Remote::start().await;
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let status = attached.status();

    // Let The Host's Grace Expire While Stop Is Unanswered
    let entered = remote.hold_stop();
    let stopping = tokio::spawn(async move { attached.stop("Removed").await });
    entered.await.unwrap();
    source.advance_next().await;
    let error = stopping.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Timeout("Stop")));

    // Cleanup Records The Failure And Leaves The Remote Server Available
    assert!(!activation.is_connected());
    assert!(status.borrow().activation.is_none());
    assert!(matches!(status.borrow().failure.as_deref(), Some(Error::Timeout("Stop"))));
    assert_eq!(remote.stops(), 1);
    assert_eq!(remote.describe().await.id, "remote");
    runtime.shutdown().await.unwrap();
}
