/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Managed Plugin Announces Its Listener by Calling the Host With Its Launch Credential

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, time::Duration};

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::{TcpListener, TcpStream},
};

// Local Imports
use super::{
    invoke_support::pending,
    support::{self, App, Manual},
};
use harmos_protocol::plugin::{LaunchesAnnounceRequest, launches_client::LaunchesClient};
use harmos_runtime::{
    Runtime,
    clock::{Clock, Source},
    operations::OperationPolicy,
    plugin::grpc::{Error, GrpcPluginPolicy, GrpcPluginSettings, Managed, Placement, StartupOutput},
};

/* === Constants === */

const LOAD: Duration = Duration::from_secs(10);

/// The Standard-Error Tail Bound of These Stories, Far Below the Stub's Hundred Lines.
const TAIL_LINES: usize = 5;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/// A Runtime on `source` With a `LOAD` Startup Deadline and a `TAIL_LINES` Standard-Error Tail.
async fn configured(source: &Manual) -> Runtime<App> {
    let clock = Clock::with_clock(source.clone());
    crate::support::booted(Default::default(), |setup| {
        let load = OperationPolicy {
            timeout: LOAD,
            attempt_timeout: LOAD,
            ..GrpcPluginPolicy::default().load
        };
        let startup_output = StartupOutput {
            lines: NonZeroUsize::new(TAIL_LINES).unwrap(),
            bytes: NonZeroUsize::new(4096).unwrap(),
        };
        setup.settings().clock(clock).grpc_plugin_settings(GrpcPluginSettings {
            policy: GrpcPluginPolicy { load },
            startup_output,
        });
    })
    .await
}

/// The Managed Stub Placement: It Reports on `control` and Announces `announced` in `mode`.
fn stub(control: &str, announced: &str, mode: &str) -> Placement {
    Placement::Managed(Managed {
        executable: support::binary("harmos-managed-stub-fixture"),
        directory: None,
        arguments: vec![control.into(), announced.into(), mode.into()],
        environment: Default::default(),
    })
}

/// Read One Line the Stub Reports on Its Control Connection.
async fn report(control: &mut BufReader<TcpStream>) -> String {
    let mut line = String::new();
    control.read_line(&mut line).await.unwrap();
    line.trim().to_owned()
}

/// Wait Until the Stub Closed Its Control Connection, Which Happens Only When the Host Killed and Reaped It.
async fn reaped(mut control: BufReader<TcpStream>) {
    let mut rest = Vec::new();
    let _ = control.read_to_end(&mut rest).await;
}

async fn announce(endpoint: &str, token: &str, address: &str) -> Result<(), tonic::Status> {
    let mut client =
        LaunchesClient::connect(format!("http://{endpoint}")).await.map_err(|error| tonic::Status::unavailable(error.to_string()))?;
    client
        .announce(LaunchesAnnounceRequest {
            token: token.into(),
            address: address.into(),
        })
        .await
        .map(|_| ())
}

/* === Tests === */

#[tokio::test]
async fn an_exit_before_announcing_reports_its_status_and_bounded_standard_error() {
    // A Child That Writes a Hundred Lines to Standard Error, Then Exits Without Announcing.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let origin = source.read().instant;
    let refused = runtime.attach_grpc(stub("127.0.0.1:9", "127.0.0.1:9", "exit"), support::settings()).await;

    // The Exit Itself Fails the Load at Once, Carrying the Status and Only the Newest Lines.
    let Err(Error::Exited { process, status, diagnostics }) = refused else { panic!("Report the Early Exit: {:?}", refused.err()) };
    assert_eq!(status.code(), Some(7));
    assert!(!process.contains("Some("), "Print the Process Identifier as a Number: {process}");
    let tail = diagnostics.lines().filter(|line| line.starts_with("startup failure line")).collect::<Vec<_>>();
    assert_eq!(
        tail,
        [
            "startup failure line 96",
            "startup failure line 97",
            "startup failure line 98",
            "startup failure line 99",
            "startup failure line 100"
        ]
    );
    assert_eq!(source.read().instant, origin, "Fail Before Any Deadline Elapses");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn an_unreachable_announced_address_fails_and_reaps_the_child() {
    // Announce a Loopback Port Nothing Serves.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let vacated = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().to_string();
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = stub(&control.local_addr().unwrap().to_string(), &vacated, "announce");
    let (refused, accepted) = tokio::join!(runtime.attach_grpc(placement, support::settings()), control.accept());

    // The Load Names the Endpoint; the Child Is Gone Before the Failure Returns.
    let error = refused.err().expect("Refuse an Unreachable Announcement");
    assert!(matches!(&error, Error::Invalid(message) if message.contains(&vacated)), "Name the Announced Endpoint: {error}");
    reaped(BufReader::new(accepted.unwrap().0)).await;
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn the_announcement_reply_precedes_admission() {
    // A Plugin Listener That Accepts Connections but Never Describes Itself Holds Admission Open.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = stub(&control.local_addr().unwrap().to_string(), &silent.local_addr().unwrap().to_string(), "announce");
    // Boxed, So Dropping It Ends the Attachment Rather Than a Pinned Reference.
    let mut attaching = Box::pin(runtime.attach_grpc(placement, support::settings()));
    let (stream, _) = tokio::select! {
        biased;
        _ = &mut attaching => panic!("Admit Only After the Plugin Describes Itself"),
        accepted = control.accept() => accepted.unwrap(),
    };

    // The Plugin Receives Its Reply While the Host Still Waits to Describe It.
    let mut control = BufReader::new(stream);
    assert_eq!(report(&mut control).await, "announced");
    assert!(pending(attaching.as_mut()).await, "Keep Admission Pending After the Reply");
    drop(attaching);
    reaped(control).await;
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_silent_launch_fails_at_the_load_deadline_and_refuses_its_late_announcement() {
    // A Child That Never Announces Reports Its Credential to the Story.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = stub(&control.local_addr().unwrap().to_string(), "127.0.0.1:9", "silent");
    let deadline = source.read().instant + LOAD;
    let attaching = runtime.attach_grpc(placement, support::settings());
    tokio::pin!(attaching);
    let (stream, _) = tokio::select! {
        biased;
        _ = &mut attaching => panic!("Wait for the Announcement Until the Deadline"),
        accepted = control.accept() => accepted.unwrap(),
    };
    let mut control = BufReader::new(stream);
    let credential = report(&mut control).await;
    let (endpoint, token) = credential.split_once(' ').unwrap();

    // The Load Deadline Fails the Startup and Revokes the Credential.
    tokio::select! {
        biased;
        _ = &mut attaching => panic!("Keep Waiting Until the Deadline"),
        () = source.registered_at(deadline) => {},
    }
    source.advance_by(LOAD);
    let refused = attaching.await;
    assert!(
        matches!(&refused, Err(Error::LoadDeadline { timeout, diagnostics, .. }) if *timeout == LOAD && !diagnostics.is_empty()),
        "Fail at the Load Deadline With the Process Diagnostics: {:?}",
        refused.err()
    );
    reaped(control).await;
    let late = announce(endpoint, token, "127.0.0.1:9").await.unwrap_err();
    assert_eq!(late.code(), tonic::Code::Unauthenticated);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_cancelled_launch_revokes_its_credential() {
    // Read the Credential of a Child That Never Announces.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = stub(&control.local_addr().unwrap().to_string(), "127.0.0.1:9", "silent");
    // Boxed, So Dropping It Ends the Attachment Rather Than a Pinned Reference.
    let mut attaching = Box::pin(runtime.attach_grpc(placement, support::settings()));
    let (stream, _) = tokio::select! {
        biased;
        _ = &mut attaching => panic!("Wait for the Announcement"),
        accepted = control.accept() => accepted.unwrap(),
    };
    let mut control = BufReader::new(stream);
    let credential = report(&mut control).await;
    let (endpoint, token) = credential.split_once(' ').unwrap();

    // Cancelling the Attachment Reaps the Child and Spends the Credential.
    drop(attaching);
    reaped(control).await;
    let refused = announce(endpoint, token, "127.0.0.1:9").await.unwrap_err();
    assert_eq!(refused.code(), tonic::Code::Unauthenticated);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn a_stopping_runtime_revokes_pending_credentials_and_releases_the_bootstrap_listener() {
    // A Startup Waits on a Child That Never Announces.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = stub(&control.local_addr().unwrap().to_string(), "127.0.0.1:9", "silent");
    let attaching = runtime.attach_grpc(placement, support::settings());
    tokio::pin!(attaching);
    let (stream, _) = tokio::select! {
        biased;
        _ = &mut attaching => panic!("Wait for the Announcement"),
        accepted = control.accept() => accepted.unwrap(),
    };
    let mut control = BufReader::new(stream);
    let credential = report(&mut control).await;
    let (endpoint, token) = credential.split_once(' ').unwrap();

    // Stopping the Runtime Ends the Startup; Nothing Answers on the Bootstrap Endpoint Afterwards.
    let (stopped, refused) = tokio::join!(runtime.stop(), attaching);
    stopped.unwrap();
    assert!(refused.is_err(), "End the Startup With the Runtime");
    reaped(control).await;
    assert!(announce(endpoint, token, "127.0.0.1:9").await.is_err(), "Release the Bootstrap Listener");
}

#[tokio::test]
async fn a_client_listener_has_no_route_to_announce() {
    // A Product Client Listener With Reflection Serves Operations Only.
    let source = Manual::new();
    let runtime = configured(&source).await;
    let attached = runtime.grpc().attach(harmos_runtime::grpc::Settings { reflection: true, ..Default::default() }).await.unwrap();

    // Announce Is Not a Route There, Whatever It Carries.
    let refused = announce(&attached.local_addr().to_string(), "0".repeat(64).as_str(), "127.0.0.1:9").await.unwrap_err();
    assert_eq!(refused.code(), tonic::Code::Unimplemented, "{refused}");
    attached.detach().await.unwrap();
    runtime.stop().await.unwrap();
}
