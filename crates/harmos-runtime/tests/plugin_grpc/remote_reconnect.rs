/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Remote Plugin Connection Replacement

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Manual, Remote};
use harmos_runtime::{clock::Clock, plugin::grpc::Placement};

/* === Tests === */

#[tokio::test]
async fn remote_reconnect_exposes_process_identity_and_retires_old_handles() {
    // Connect A Remote Plugin Using The Host Clock
    let source = Manual::new();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = Remote::start().await;
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let first = attached.activation().unwrap();
    let mut status = attached.status();

    // Hold A Call Across The Drop
    let entered = remote.hold_description();
    let retained = first.clone();
    let inflight = tokio::spawn(async move { retained.describe().await });
    entered.await.unwrap();

    // Reconnect To The Same Process Without Reusing The Old Connection
    remote.drop_connection(100).await;
    support::offline(&mut status).await;
    assert!(!first.is_connected());
    assert!(inflight.await.unwrap().is_err());
    source.advance_next().await;
    let second = support::activation(&mut status, 2).await;
    assert_eq!(second.description.started_at, first.description.started_at);
    assert_eq!(second.previous_started_at, first.description.started_at);
    assert_eq!(second.description.catalog, first.description.catalog);
    assert!(first.describe().await.is_err());
    assert!(second.describe().await.is_ok());

    // A Restart Exposes Both Start Times In A Fresh Activation
    remote.drop_connection(200).await;
    support::offline(&mut status).await;
    source.advance_next().await;
    let third = support::activation(&mut status, 3).await;
    assert_ne!(third.description.started_at, second.description.started_at);
    assert_eq!(third.previous_started_at, second.description.started_at);
    assert!(second.describe().await.is_err());

    // Removing A Remote Plugin Calls Stop And Leaves Its Server Running
    attached.stop("Removed").await.unwrap();
    assert_eq!(remote.stops(), 1);
    assert_eq!(remote.describe().await.started_at, third.description.started_at);
    runtime.shutdown().await.unwrap();
}
