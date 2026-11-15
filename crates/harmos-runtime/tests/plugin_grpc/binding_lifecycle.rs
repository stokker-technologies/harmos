/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Scoped Binding Lifecycle Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::host::TimeGetResponse;
use harmos_protocol::plugin::InstanceSamplesResponse;
use harmos_protocol::plugin::Sample;
use harmos_protocol::plugin::instance_samples_request::Frame as RequestFrame;

// Standard Library Imports
use std::{
    sync::{atomic::Ordering, mpsc as blocking},
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message as _;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tonic::Status;

// Local Imports
use super::{
    invoke_support::{HostFixture, pending},
    support::{self, Manual, Remote},
};
use harmos_protocol::model;
use harmos_runtime::{
    clock::{Clock, Source as _},
    plugin::grpc::{Error, Managed, Placement, PlacementSettings},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Scope Name Whose Conversion Parks the Binding Thread Inside `bind_grpc`.
struct Racing {
    name: &'static str,
    entered: oneshot::Sender<()>,
    release: blocking::Receiver<()>,
}

/// Reports a Samples Response Dropped by the Host.
struct Released(String, mpsc::UnboundedSender<String>);

/// A Plugin That Opens Demands but Never Ends a Samples Response.
struct Hung {
    remote: Remote,
    opened: mpsc::UnboundedReceiver<String>,
    half_closed: mpsc::UnboundedReceiver<String>,
    released: mpsc::UnboundedReceiver<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<Racing> for String {
    fn from(racing: Racing) -> Self {
        racing.entered.send(()).unwrap();
        racing.release.recv().unwrap();
        racing.name.into()
    }
}

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.1.send(self.0.clone());
    }
}

impl Hung {
    async fn start() -> Self {
        let remote = Remote::start().await;
        let mut catalog = model::Declarations {
            types: Some(harmos_protocol::clock::declarations_types()),
            streams: vec![model::StreamDeclaration {
                id: "probe".into(),
                row: "harmos.host.v1.TimeGetResponse".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
        remote.declarations(catalog);
        let (open, opened) = mpsc::unbounded_channel();
        let (half_close, half_closed) = mpsc::unbounded_channel();
        let (release, released) = mpsc::unbounded_channel();
        remote.samples(move |mut input| {
            let (scope_sender, scope) = oneshot::channel::<String>();
            let open = open.clone();
            let half_close = half_close.clone();
            tokio::spawn(async move {
                let Some(RequestFrame::Open(selection)) = input.message().await.unwrap().unwrap().frame else {
                    panic!("Expected Open");
                };
                open.send(selection.scope.clone()).unwrap();
                let _ = scope_sender.send(selection.scope.clone());
                // Observe the Host's Half-Close and Still Never End the Response.
                while input.message().await.ok().flatten().is_some() {}
                let _ = half_close.send(selection.scope);
            });
            let release = release.clone();
            Box::pin(stream::unfold(Some((scope, release)), |state| async move {
                let (scope, release) = state?;
                let scope = scope.await.unwrap_or_default();
                let _released = Released(scope, release);
                std::future::pending::<Option<(Result<InstanceSamplesResponse, Status>, _)>>().await
            }))
        });
        Self { remote, opened, half_closed, released }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// A Device-Stream Plugin That Answers Each Scoped Demand With One Sample.
async fn device() -> Remote {
    let remote = Remote::start().await;
    let mut catalog = model::Declarations {
        types: Some(harmos_protocol::clock::declarations_types()),
        streams: vec![model::StreamDeclaration {
            id: "probe".into(),
            row: "harmos.host.v1.TimeGetResponse".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    remote.samples(|mut input| {
        let (output, receiver) = mpsc::channel(1);
        tokio::spawn(async move {
            let Some(RequestFrame::Open(selection)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected Open");
            };
            if !selection.scope.is_empty() {
                let sample = Sample {
                    at: None,
                    sequence: 1,
                    row: TimeGetResponse::default().encode_to_vec(),
                };
                let _ = output.send(Ok(InstanceSamplesResponse { samples: vec![sample], frame: None })).await;
            }
            // End the Response Once the Host Half-Closes or Drops the Demand.
            while input.message().await.ok().flatten().is_some() {}
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    remote
}

/// A Grace No Operation Policy Shares, So Its Deadline Identifies the Teardown Timer.
fn settings() -> PlacementSettings {
    PlacementSettings {
        stop_grace: Duration::from_secs(45),
        ..support::settings()
    }
}

fn rows(host: &HostFixture, scope: &str) -> usize {
    host.rows()
        .iter()
        .filter(|row| {
            row.context
                .as_ref()
                .and_then(|context| serde_json::from_str::<serde_json::Value>(context).ok())
                .is_some_and(|context| context["scope"] == scope)
        })
        .count()
}

/* === Tests === */

#[tokio::test]
async fn managed_stop_kills_a_plugin_whose_scoped_drain_outlives_stop_grace() {
    // Bind One Scoped Unit to a Managed Plugin on the Controlled Host Clock.
    let host = HostFixture::new().await;
    let grace = settings().stop_grace;
    let attached = host.runtime.attach_grpc(host.managed_sdk("scoped-publications"), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let process = activation.process_id.unwrap();
    let (unit_clock, unit_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/slow", unit_clock).await.unwrap();
    unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/slow") == 1).await.unwrap();

    // Hold the Next Accepted Scoped Sample So Its Drain Outlives the Grace.
    host.signals.storage_hold.store(true, Ordering::SeqCst);
    let advancing = unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/slow") == 2);
    tokio::pin!(advancing);
    tokio::select! {
        result = &mut advancing => panic!("Ingestion Is Still Held: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }

    // Stop Spends the Whole Grace on the Drain, Then Kills and Reaps the Child.
    let deadline = host.clock.read().instant + grace;
    let stopping = attached.stop("Story Stop");
    tokio::pin!(stopping);
    tokio::select! {
        result = &mut stopping => panic!("Stop Finished Before Its Drain Deadline: {result:?}"),
        () = host.clock.registered_at(deadline) => {},
    }
    host.clock.advance_by(grace);
    let error = stopping.await.unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { .. }), "Report the Drain Deadline: {error}");
    assert!(!support::running(process), "Kill and Reap the Managed Plugin");
    assert!(binding.close().await.is_err(), "Report the Cancelled Scoped Drain");

    // Shutdown Succeeds and the Accepted Sample Is Still Delivered.
    host.signals.storage_release.notify_one();
    advancing.await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert_eq!(rows(&host, "unit/slow"), 2);
}

#[tokio::test]
async fn closing_against_a_plugin_that_ignores_half_close_is_bounded_by_stop_grace() {
    // Attach a Plugin Whose Samples Responses Never End.
    let manual = Manual::new();
    let grace = settings().stop_grace;
    let runtime = support::runtime(Some(Clock::with_clock(manual.clone()))).await;
    let mut hung = Hung::start().await;
    let attached = runtime.attach_grpc(Placement::Remote(hung.remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = runtime.bind_grpc(&activation, "unit/hung", unit_clock.clone()).await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "unit/hung");

    // Close Half-Closes the Demand and Bounds the Wait for Its End.
    let deadline = manual.read().instant + grace;
    let closing = tokio::spawn(binding.close());
    assert_eq!(hung.half_closed.recv().await.unwrap(), "unit/hung");
    assert!(manual.timer_at(deadline), "Bound the Close Before Awaiting the Plugin");
    manual.advance_by(grace);
    let error = closing.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Close Scoped Plugin" }), "Name the Bounded Action: {error}");
    assert_eq!(hung.released.recv().await.unwrap(), "unit/hung", "Cancel the Unended Samples Response");
    assert!(activation.is_connected(), "Keep the Attachment Connected");

    // A Dropped Binding Settles Under the Attachment With the Same Bound.
    let dropped = runtime.bind_grpc(&activation, "unit/hung", unit_clock).await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "unit/hung");
    let deadline = manual.read().instant + grace;
    drop(dropped);
    assert_eq!(hung.half_closed.recv().await.unwrap(), "unit/hung");
    manual.registered_at(deadline).await;
    manual.advance_by(grace);
    assert_eq!(hung.released.recv().await.unwrap(), "unit/hung");

    // Stopping the Attachment Bounds Its Own Unended Demand the Same Way.
    let deadline = manual.read().instant + grace;
    let stopping = tokio::spawn(async move { attached.stop("Story Complete").await });
    assert_eq!(hung.half_closed.recv().await.unwrap(), "");
    manual.registered_at(deadline).await;
    manual.advance_by(grace);
    let error = stopping.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { .. }), "Report the Root Drain Deadline: {error}");
    assert_eq!(hung.released.recv().await.unwrap(), "");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn plugin_exit_settles_its_live_binding_before_reporting() {
    // A Managed Process Fronts an In-Process Plugin, So Its Exit Leaves the Connection Open.
    let host = HostFixture::new().await;
    let grace = settings().stop_grace;
    let remote = device().await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-managed-stub-fixture"),
        arguments: vec![control.local_addr().unwrap().to_string().into(), remote.address.trim_start_matches("http://").into()],
        environment: Default::default(),
    });
    let (attached, accepted) = tokio::join!(host.runtime.attach_grpc(placement, settings()), control.accept());
    let attached = attached.unwrap();
    let (control, _) = accepted.unwrap();
    let activation = attached.activation().unwrap();
    let process = activation.process_id.unwrap();
    let mut status = attached.status();

    // Hold the Scoped Sample's Ingestion, Then End the Managed Process.
    host.signals.storage_hold.store(true, Ordering::SeqCst);
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/exit", unit_clock).await.unwrap();
    host.signals.storage_entered.notified().await;
    let deadline = host.clock.read().instant + grace;
    drop(control);

    // The Attachment Reports the Exit Only After Its Binding Settled the Accepted Sample.
    let offline = support::offline(&mut status);
    tokio::pin!(offline);
    tokio::select! {
        () = &mut offline => panic!("Report Exit Only After the Binding Settles"),
        () = host.clock.registered_at(deadline) => {},
    }
    let closing = binding.close();
    tokio::pin!(closing);
    assert!(pending(closing.as_mut()).await, "Hold Scoped Settlement on Accepted Ingestion");
    host.signals.storage_release.notify_one();
    offline.await;
    assert!(matches!(closing.await.unwrap_err().as_ref(), Error::Transport(_)), "Report the Lost Connection");
    assert_eq!(rows(&host, "unit/exit"), 1, "Deliver Exactly the Accepted Scoped Sample");
    assert!(matches!(attached.stop("Already Exited").await.unwrap_err().as_ref(), Error::Exit(_)));
    assert!(!support::running(process), "Reap the Exited Process");
    assert!(host.runtime.bind_grpc(&activation, "unit/late", Clock::virtual_at(SystemTime::UNIX_EPOCH).0).await.is_err());
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bind_racing_attachment_stop_is_refused() {
    // Park a Bind Between Its Checks and Its Registration.
    let host = HostFixture::new().await;
    let sdk = host.sdk("caller").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let (entered, entering) = oneshot::channel();
    let (release, parked) = blocking::channel();

    // Stop the Attachment Completely While the Bind Is Parked.
    let stopping = tokio::spawn(async move {
        entering.await.unwrap();
        let result = attached.stop("Story Stop").await;
        release.send(()).unwrap();
        result
    });
    let racing = Racing {
        name: "unit/racing",
        entered,
        release: parked,
    };
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let bound = host.runtime.bind_grpc(&activation, racing, unit_clock).await;
    stopping.await.unwrap().unwrap();
    let error = bound.err().expect("Refuse a Binding After Its Attachment Stopped");
    assert!(matches!(error, Error::Invalid(_)), "Refuse as Invalid: {error}");
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn runtime_shutdown_settles_a_live_binding_and_reaps_its_plugin() {
    // Leave One Scoped Binding Open on a Managed Plugin.
    let host = HostFixture::new().await;
    let attached = host.runtime.attach_grpc(host.managed_sdk("scoped-publications"), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let process = activation.process_id.unwrap();
    let (unit_clock, unit_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/shutdown", unit_clock.clone()).await.unwrap();
    unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/shutdown") == 1).await.unwrap();

    // Shutdown Settles the Binding, Stops the Plugin and Reaps Its Process.
    host.runtime.shutdown().await.unwrap();
    assert!(!support::running(process), "Reap the Managed Plugin");
    binding.close().await.unwrap();
    attached.stop("Already Stopped").await.unwrap();
    assert_eq!(rows(&host, "unit/shutdown"), 1, "Keep Exactly the Accepted Scoped Row");
    assert!(host.runtime.bind_grpc(&activation, "unit/late", unit_clock).await.is_err());
}

#[tokio::test]
async fn managed_disconnect_settles_its_binding_and_kills_the_process() {
    // A Live Managed Process Whose Plugin Connection the Story Can Cut.
    let host = HostFixture::new().await;
    let remote = device().await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-managed-stub-fixture"),
        arguments: vec![control.local_addr().unwrap().to_string().into(), remote.address.trim_start_matches("http://").into()],
        environment: Default::default(),
    });
    let (attached, accepted) = tokio::join!(host.runtime.attach_grpc(placement, support::settings()), control.accept());
    let attached = attached.unwrap();
    let _control = accepted.unwrap();
    let activation = attached.activation().unwrap();
    let process = activation.process_id.unwrap();
    let mut status = attached.status();
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/cut", unit_clock).await.unwrap();
    while rows(&host, "unit/cut") == 0 {
        host.signals.storage_ingested.notified().await;
    }

    // Losing the Connection Settles the Binding, Then Kills and Reaps the Process.
    remote.drop_connection(100).await;
    support::offline(&mut status).await;
    assert!(matches!(binding.close().await.unwrap_err().as_ref(), Error::Transport(_)), "Report the Lost Connection");
    assert!(matches!(attached.stop("Already Disconnected").await.unwrap_err().as_ref(), Error::Disconnected));
    assert!(!support::running(process), "Kill and Reap the Disconnected Process");
    assert_eq!(rows(&host, "unit/cut"), 1);
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn binding_during_reconnect_waits_for_the_next_activation() {
    // Serve One Scoped Row Before the Remote Connection Drops.
    let host = HostFixture::new().await;
    let remote = device().await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let mut status = attached.status();
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let live = host.runtime.bind_grpc(&activation, "unit/live", unit_clock.clone()).await.unwrap();
    while rows(&host, "unit/live") == 0 {
        host.signals.storage_ingested.notified().await;
    }

    // While Reconnecting, the Old Activation Refuses Binds and Its Binding Has Settled.
    remote.drop_connection(200).await;
    support::offline(&mut status).await;
    assert!(matches!(live.close().await.unwrap_err().as_ref(), Error::Transport(_)), "Report the Lost Connection");
    let refused = host.runtime.bind_grpc(&activation, "unit/next", unit_clock.clone()).await;
    assert!(matches!(refused, Err(Error::Invalid(_))), "Refuse Binds on a Disconnected Activation");

    // The Next Activation Admits the Same Streams and Serves the Scope.
    host.clock.advance_next().await;
    let next = support::activation(&mut status, 2).await;
    let bound = host.runtime.bind_grpc(&next, "unit/next", unit_clock).await.unwrap();
    while rows(&host, "unit/next") == 0 {
        host.signals.storage_ingested.notified().await;
    }
    bound.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert_eq!((rows(&host, "unit/live"), rows(&host, "unit/next")), (1, 1));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_killed_host_takes_its_managed_plugin_with_it() {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use tokio::io::{AsyncBufReadExt, BufReader};

    // Start a Separate Host Process That Attaches a Managed Plugin.
    let evidence = tempfile::tempdir().unwrap();
    let cleaned = evidence.path().join("cleanup");
    let mut parent = tokio::process::Command::new(support::binary("harmos-managed-parent-fixture"))
        .arg(support::binary("harmos-sdk-host-fixture"))
        .arg(&cleaned)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(parent.stdout.take().unwrap()).lines();
    let plugin = loop {
        let line = lines.next_line().await.unwrap().expect("Host Reports Its Plugin");
        if let Some(process) = line.strip_prefix("plugin pid ") {
            break process.parse::<libc::pid_t>().unwrap();
        }
    };

    // Watch the Plugin by Process Identifier, Then Kill Its Host.
    // SAFETY: pidfd_open Returns a New Descriptor That This Story Owns.
    #[allow(unsafe_code)]
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, plugin, 0) };
    assert!(descriptor >= 0, "Open the Plugin's Process Descriptor");
    // SAFETY: The Descriptor Was Just Opened and Is Owned Exactly Once.
    #[allow(unsafe_code)]
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor as i32) };
    let exited = tokio::task::spawn_blocking(move || {
        let mut watched = libc::pollfd {
            fd: descriptor.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: One Valid Entry Is Polled Until the Process Exits.
        #[allow(unsafe_code)]
        let ready = unsafe { libc::poll(&mut watched, 1, -1) };
        (ready, watched.revents)
    });
    parent.kill().await.unwrap();

    // The Plugin Ends by Its Parent-Death Signal, Without a Stop Request.
    let (ready, events) = exited.await.unwrap();
    assert_eq!((ready, events & libc::POLLIN), (1, libc::POLLIN), "Observe the Plugin's Exit by Its Process Identifier");
    assert!(!cleaned.exists(), "End the Plugin Without Running Its Stop Hooks");
}
