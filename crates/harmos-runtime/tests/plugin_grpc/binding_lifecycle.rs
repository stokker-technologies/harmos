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
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc as blocking,
    },
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
use tracing_subscriber::layer::SubscriberExt as _;

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

/// Records the `action` of Every Warning Emitted While Installed.
#[derive(Clone, Default)]
struct Actions(Arc<Mutex<Vec<String>>>);

/// Reads One Event's `action` Field.
struct Action(Option<String>);

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

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Actions {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut action = Action(None);
        event.record(&mut action);
        self.0.lock().unwrap().extend(action.0);
    }
}

impl tracing::field::Visit for Action {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "action" {
            self.0 = Some(value.into());
        }
    }

    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

impl Actions {
    /// Observe Warnings on This Story's Thread; Every Task of a Current-Thread Story Runs There.
    fn install(&self) -> (tracing::Dispatch, tracing::subscriber::DefaultGuard) {
        // Register the Unobserved Dispatcher Too: Parallel Stories Register Callsites Without a
        // Subscriber, Which Tracing's Single-Dispatcher Cache Otherwise Treats as Disabled.
        let unobserved = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        (unobserved, tracing::subscriber::set_default(tracing_subscriber::registry().with(self.clone())))
    }

    fn contains(&self, action: &str) -> bool {
        self.0.lock().unwrap().iter().any(|recorded| recorded == action)
    }
}

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.1.send(self.0.clone());
    }
}

impl Hung {
    /// Declare One Device Stream, or an Interval Stream When `interval` Is Given.
    async fn start(interval: Option<prost_types::Duration>) -> Self {
        let remote = Remote::start().await;
        let mut catalog = model::Declarations {
            types: Some(harmos_protocol::clock::declarations_types()),
            streams: vec![model::StreamDeclaration {
                id: "probe".into(),
                row: "harmos.host.v1.TimeGetResponse".into(),
                interval,
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

/// An Interval-Stream Plugin That Answers Scoped Windows Only While Its Unit Is Registered.
async fn windowed(registered: Arc<AtomicBool>, refused: Arc<AtomicUsize>) -> Remote {
    let remote = Remote::start().await;
    let mut catalog = model::Declarations {
        types: Some(harmos_protocol::clock::declarations_types()),
        streams: vec![model::StreamDeclaration {
            id: "probe".into(),
            row: "harmos.host.v1.TimeGetResponse".into(),
            interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    remote.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(1);
        let registered = registered.clone();
        let refused = refused.clone();
        tokio::spawn(async move {
            let Some(RequestFrame::Open(_)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected Open");
            };
            // Answer Each Window Until the Host Half-Closes the Demand.
            while let Some(request) = input.message().await.ok().flatten() {
                let Some(RequestFrame::Window(window)) = request.frame else { continue };
                if !registered.load(Ordering::SeqCst) {
                    refused.fetch_add(1, Ordering::SeqCst);
                    let _ = output.send(Err(Status::failed_precondition("Start the Unit Before Requesting Its Samples"))).await;
                    continue;
                }
                let row = TimeGetResponse {
                    now: Some(prost_types::Timestamp::default()),
                }
                .encode_to_vec();
                let _ = output.send(Ok(support::answer(window.count, row.repeat(window.count as usize)))).await;
            }
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
async fn managed_stop_reaps_a_plugin_whose_scoped_drain_outlives_stop_grace() {
    // Bind One Scoped Unit to a Managed Plugin on the Controlled Host Clock.
    let host = HostFixture::new().await;
    let grace = settings().stop_grace;
    let attached = host.runtime.attach_grpc(host.managed_sdk("scoped-publications"), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let process = activation.process_id.unwrap();
    let (unit_clock, unit_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/slow", unit_clock).await.unwrap();
    binding.start_samples().await.unwrap();
    unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/slow") == 1).await.unwrap();

    // Hold the Next Accepted Scoped Sample So Its Drain Outlives the Grace.
    host.signals.storage_hold.store(true, Ordering::SeqCst);
    let advancing = unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/slow") == 2);
    tokio::pin!(advancing);
    tokio::select! {
        result = &mut advancing => panic!("Ingestion Is Still Held: {result:?}"),
        () = host.signals.storage_entered.notified() => {},
    }

    // Stop Spends the Whole Grace on the Drain, Then Stops and Reaps the Child.
    let deadline = host.clock.read().instant + grace;
    let stopping = attached.stop("Story Stop");
    tokio::pin!(stopping);
    tokio::select! {
        result = &mut stopping => panic!("Stop Finished Before Its Drain Deadline: {result:?}"),
        () = host.clock.registered_at(deadline) => {},
    }
    host.clock.advance_by(grace);
    let error = stopping.await.unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Settle Plugin Publications" }), "Report the Drain Deadline: {error}");
    assert!(!support::running(process), "Reap the Managed Plugin");
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
    let mut hung = Hung::start(None).await;
    let attached = runtime.attach_grpc(Placement::Remote(hung.remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = runtime.bind_grpc(&activation, "unit/hung", unit_clock.clone()).await.unwrap();
    binding.start_samples().await.unwrap();
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
    dropped.start_samples().await.unwrap();
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
    let actions = Actions::default();
    let _observed = actions.install();
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
    binding.start_samples().await.unwrap();
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
    assert!(actions.contains("Settle Plugin Publications"), "Report the Attachment's Settlement Failure");
    assert_eq!(rows(&host, "unit/exit"), 1, "Deliver Exactly the Accepted Scoped Sample");
    assert!(matches!(attached.stop("Already Exited").await.unwrap_err().as_ref(), Error::Exit(_)));
    assert!(!support::running(process), "Reap the Exited Process");
    assert!(host.runtime.bind_grpc(&activation, "unit/late", Clock::virtual_at(SystemTime::UNIX_EPOCH).0).await.is_err());
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bind_reaching_the_lock_after_attachment_stop_is_refused() {
    // Park a Bind Before It Takes the Attachment's Bindings Lock.
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
    binding.start_samples().await.unwrap();
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
    let actions = Actions::default();
    let _observed = actions.install();
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
    binding.start_samples().await.unwrap();
    while rows(&host, "unit/cut") == 0 {
        host.signals.storage_ingested.notified().await;
    }

    // Losing the Connection Settles the Binding, Then Kills and Reaps the Process.
    remote.drop_connection(100).await;
    support::offline(&mut status).await;
    assert!(matches!(binding.close().await.unwrap_err().as_ref(), Error::Transport(_)), "Report the Lost Connection");
    assert!(matches!(attached.stop("Already Disconnected").await.unwrap_err().as_ref(), Error::Disconnected));
    assert!(actions.contains("Settle Plugin Publications"), "Report the Attachment's Settlement Failure");
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
    live.start_samples().await.unwrap();
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
    bound.start_samples().await.unwrap();
    while rows(&host, "unit/next") == 0 {
        host.signals.storage_ingested.notified().await;
    }
    bound.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert_eq!((rows(&host, "unit/live"), rows(&host, "unit/next")), (1, 1));
}

#[tokio::test]
async fn a_virtual_runtime_clock_reaches_teardown_deadlines_against_a_plugin_ignoring_half_close() {
    // Attach on a Virtual Runtime Clock; the Root Demand's First Window Lies Beyond Every Deadline.
    let grace = settings().stop_grace;
    let (clock, time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = support::runtime(Some(clock)).await;
    let mut hung = Hung::start(Some(prost_types::Duration { seconds: 3600, nanos: 0 })).await;
    let attached = runtime.attach_grpc(Placement::Remote(hung.remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = runtime.bind_grpc(&activation, "unit/virtual", unit_clock).await.unwrap();
    binding.start_samples().await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "unit/virtual");

    // Closing Parks While It Waits on the Plugin, So Advancing the Grace Expires the Drain.
    let closing = tokio::spawn(binding.close());
    assert_eq!(hung.half_closed.recv().await.unwrap(), "unit/virtual");
    time.advance_by(grace).await;
    let error = closing.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Close Scoped Plugin" }), "Name the Bounded Action: {error}");
    assert_eq!(hung.released.recv().await.unwrap(), "unit/virtual");

    // Stop Expires the Root Drain the Same Way, Then Still Tells the Plugin to Stop.
    let stopping = tokio::spawn(async move { attached.stop("Story Complete").await });
    assert_eq!(hung.half_closed.recv().await.unwrap(), "");
    time.advance_by(grace).await;
    let error = stopping.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Settle Plugin Publications" }), "Report the Root Drain Deadline: {error}");
    assert_eq!(hung.remote.stops(), 1, "Tell the Plugin to Stop After Its Drain Deadline");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_bind_registered_before_attachment_stop_is_settled_by_the_stop() {
    // Register a Binding Under the Attachment's Lock and Hold Its Samples Start.
    let host = HostFixture::new().await;
    let remote = device().await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let (entered, abandoned) = remote.hold_samples();
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/preparing", unit_clock).await.unwrap();
    let (stopped, started) = {
        let starting = binding.start_samples();
        tokio::pin!(starting);
        tokio::select! {
            result = &mut starting => panic!("Samples Started Before Their Response: {result:?}"),
            entered = entered => entered.unwrap(),
        }

        // Stop Takes the Lock Next, Owns the Registered Binding and Settles It.
        tokio::join!(attached.stop("Story Stop"), starting)
    };
    stopped.unwrap();
    let error = started.expect_err("Refuse Samples Its Attachment Settled");
    assert!(matches!(error, Error::Invalid(_)), "Refuse as Invalid: {error}");
    binding.close().await.unwrap();
    abandoned.await.unwrap();
    assert_eq!(remote.stops(), 1);
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_drain_that_spends_the_grace_still_tells_the_managed_plugin_to_stop_before_the_kill() {
    // A Managed Process Fronts a Plugin Whose Root Samples Response Never Ends.
    let manual = Manual::new();
    let grace = settings().stop_grace;
    let runtime = support::runtime(Some(Clock::with_clock(manual.clone()))).await;
    let mut hung = Hung::start(None).await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-managed-stub-fixture"),
        arguments: vec![control.local_addr().unwrap().to_string().into(), hung.remote.address.trim_start_matches("http://").into()],
        environment: Default::default(),
    });
    let (attached, accepted) = tokio::join!(runtime.attach_grpc(placement, settings()), control.accept());
    let attached = attached.unwrap();
    let _control = accepted.unwrap();
    let process = attached.activation().unwrap().process_id.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let received = hung.remote.hold_stop();

    // Settlement Spends the Whole Grace on the Unended Root Demand.
    let settled = manual.read().instant + grace;
    let stopping = attached.stop("Story Stop");
    tokio::pin!(stopping);
    tokio::select! {
        result = &mut stopping => panic!("Stop Finished Before Its Drain Deadline: {result:?}"),
        () = manual.registered_at(settled) => {},
    }
    manual.advance_by(grace);

    // The Plugin Is Told to Stop, and Stop With the Exit Wait Get a Fresh Grace Before the Kill.
    tokio::select! {
        biased;
        result = &mut stopping => panic!("Killed Without Telling the Plugin to Stop: {result:?}"),
        received = received => received.unwrap(),
    }
    assert!(manual.timer_at(settled + grace), "Bound Stop and the Exit by a Fresh Grace");
    assert!(support::running(process), "Keep the Process Until Its Stop Deadline");
    manual.advance_by(grace);
    let error = stopping.await.unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Settle Plugin Publications" }), "Report the Drain Deadline First: {error}");
    assert!(!support::running(process), "Kill and Reap the Managed Plugin");
    assert_eq!(hung.remote.stops(), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_remote_cleanup_that_misses_its_deadline_still_reconnects_cleanly() {
    // Hold a Scoped Sample's Ingestion on a Remote Attachment.
    let host = HostFixture::new().await;
    let grace = settings().stop_grace;
    let remote = device().await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let mut status = attached.status();
    host.signals.storage_hold.store(true, Ordering::SeqCst);
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let old = host.runtime.bind_grpc(&activation, "unit/old", unit_clock.clone()).await.unwrap();
    old.start_samples().await.unwrap();
    host.signals.storage_entered.notified().await;

    // Losing the Connection Starts a Cleanup Whose Drain Misses the Deadline.
    let deadline = host.clock.read().instant + grace;
    remote.drop_connection(200).await;
    host.clock.registered_at(deadline).await;
    host.clock.advance_by(grace);
    let error = old.close().await.unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { action: "Close Scoped Plugin" }), "Report the Missed Drain: {error}");
    support::offline(&mut status).await;
    let failure = status.borrow().failure.clone().expect("Report the Missed Cleanup");
    assert!(matches!(failure.as_ref(), Error::Deadline { action: "Settle Plugin Publications" }), "Name the Cleanup: {failure}");

    // The Next Activation Admits Its Own Leases and Serves a New Scope.
    host.clock.advance_next().await;
    let next = support::activation(&mut status, 2).await;
    let bound = host.runtime.bind_grpc(&next, "unit/next", unit_clock.clone()).await.unwrap();
    bound.start_samples().await.unwrap();
    assert!(host.runtime.bind_grpc(&activation, "unit/late", unit_clock).await.is_err(), "Refuse the Old Activation");

    // Late Ingestion Under the Old Leases Lands Without Disturbing the Next Activation.
    host.signals.storage_release.notify_one();
    while rows(&host, "unit/next") == 0 {
        host.signals.storage_ingested.notified().await;
    }
    assert_eq!(rows(&host, "unit/old"), 1, "Deliver the Sample Accepted Before the Deadline");
    assert!(next.is_connected(), "Keep the Next Activation Connected");
    bound.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert_eq!(rows(&host, "unit/next"), 1);
}

#[tokio::test]
async fn sample_demand_runs_only_between_start_and_stop_while_calls_stay_bound() {
    // A Plugin That Refuses Windows for a Unit It Has Not Registered.
    let host = HostFixture::new().await;
    let registered = Arc::new(AtomicBool::new(false));
    let refused = Arc::new(AtomicUsize::new(0));
    let remote = windowed(registered.clone(), refused.clone()).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings()).await.unwrap();
    let activation = attached.activation().unwrap();
    let (unit_clock, unit_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&activation, "unit/window", unit_clock).await.unwrap();

    // Unit Time Passes Before the Plugin Registers the Unit, and No Window Reaches It.
    unit_time.advance_by(Duration::from_secs(3)).await;
    assert_eq!(refused.load(Ordering::SeqCst), 0, "Grant No Window Before Samples Start");

    // Demand Starts After Registration and Grants Windows on the Unit Clock.
    registered.store(true, Ordering::SeqCst);
    binding.start_samples().await.unwrap();
    unit_time.advance_until(Duration::from_secs(5), || rows(&host, "unit/window") == 2).await.unwrap();

    // Demand Stops Before the Plugin's Cleanup Ends the Unit; Later Unit Time Grants Nothing.
    binding.stop_samples().await.unwrap();
    registered.store(false, Ordering::SeqCst);
    unit_time.advance_by(Duration::from_secs(3)).await;
    assert_eq!(refused.load(Ordering::SeqCst), 0, "Grant No Window After Samples Stop");

    // The Binding Still Holds Its Scope, and the Run Closes Clean.
    assert!(host.runtime.bind_grpc(&activation, "unit/window", Clock::virtual_at(SystemTime::UNIX_EPOCH).0).await.is_err());
    binding.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert_eq!(rows(&host, "unit/window"), 2, "Commit Every Sample Accepted Inside the Window");
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
        // The Test Runner Bounds a Regression That Never Exits.
        let ready = unsafe { libc::poll(&mut watched, 1, -1) };
        (ready, watched.revents)
    });
    parent.kill().await.unwrap();

    // The Plugin Ends by Its Parent-Death Signal, Without a Stop Request.
    let (ready, events) = exited.await.unwrap();
    assert_eq!((ready, events & libc::POLLIN), (1, libc::POLLIN), "Observe the Plugin's Exit by Its Process Identifier");
    assert!(!cleaned.exists(), "End the Plugin Without Running Its Stop Hooks");
}

#[tokio::test]
async fn a_close_after_an_abandoned_stop_keeps_the_stop_deadline() {
    // A Scope Samples a Plugin That Never Ends Its Samples Response, on a Virtual Runtime Clock.
    let grace = settings().stop_grace;
    let (clock, time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = support::runtime(Some(clock.clone())).await;
    let mut hung = Hung::start(Some(prost_types::Duration { seconds: 3600, nanos: 0 })).await;
    let attached = runtime.attach_grpc(Placement::Remote(hung.remote.address.clone()), settings()).await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = runtime.bind_grpc(&attached.activation().unwrap(), "unit/abandoned", unit_clock).await.unwrap();
    binding.start_samples().await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "unit/abandoned");

    // The Caller Gives Up on a Stop the Plugin Never Lets Finish.
    let stopped_at = clock.now();
    {
        let stopping = binding.stop_samples();
        tokio::pin!(stopping);
        tokio::select! {
            result = &mut stopping => panic!("Stop Finished Before Its Drain: {result:?}"),
            scope = hung.half_closed.recv() => assert_eq!(scope.unwrap(), "unit/abandoned"),
        }
    }

    // Close Settles by the Stop's Own Deadline: One Grace, Not Two.
    let closing = tokio::spawn(binding.close());
    time.advance_until(grace, || closing.is_finished()).await.unwrap();
    assert_eq!(clock.now(), stopped_at + grace, "Close Within the Abandoned Stop's Grace");
    let error = closing.await.unwrap().unwrap_err();
    assert!(matches!(error.as_ref(), Error::Deadline { .. }), "Report the Spent Grace: {error}");

    // The Root Demand Expires the Same Way.
    let stopping = tokio::spawn(async move { attached.stop("Story Complete").await });
    assert_eq!(hung.half_closed.recv().await.unwrap(), "");
    time.advance_by(grace).await;
    stopping.await.unwrap().unwrap_err();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn sample_requests_during_settlement_are_refused_at_once() {
    // An Attachment Stop Settles a Live Binding Against a Plugin That Never Ends Its Response.
    let grace = settings().stop_grace;
    let (clock, time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = support::runtime(Some(clock.clone())).await;
    let mut hung = Hung::start(Some(prost_types::Duration { seconds: 3600, nanos: 0 })).await;
    let attached = runtime.attach_grpc(Placement::Remote(hung.remote.address.clone()), settings()).await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "");
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = runtime.bind_grpc(&attached.activation().unwrap(), "unit/settling", unit_clock).await.unwrap();
    binding.start_samples().await.unwrap();
    assert_eq!(hung.opened.recv().await.unwrap(), "unit/settling");
    let stopping = tokio::spawn(async move { attached.stop("Story Complete").await });
    let mut drained = BTreeSet::new();
    while drained.len() < 2 {
        drained.insert(hung.half_closed.recv().await.unwrap());
    }

    // Start and Stop Answer While Settlement Waits on the Plugin, Without Time Passing.
    let started = clock.now();
    let error = binding.start_samples().await.unwrap_err();
    assert!(error.to_string().contains("close()"), "Name the Action: {error}");
    binding.stop_samples().await.unwrap_err();
    assert_eq!(clock.now(), started, "Refuse Without Waiting for the Grace");
    time.advance_by(grace).await;
    stopping.await.unwrap().unwrap_err();
    binding.close().await.unwrap_err();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_withdrawn_start_opens_no_demand() {
    // Withhold the Scope's Samples Response, Then Withdraw the Start Waiting on It.
    let host = HostFixture::new().await;
    let remote = device().await;
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings()).await.unwrap();
    let (entered, abandoned) = remote.hold_samples();
    let (unit_clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&attached.activation().unwrap(), "unit/withdrawn", unit_clock).await.unwrap();
    {
        let starting = binding.start_samples();
        tokio::pin!(starting);
        tokio::select! {
            result = &mut starting => panic!("Samples Started Before Their Response: {result:?}"),
            entered = entered => entered.unwrap(),
        }
    }

    // The Withheld Open Is Abandoned, and a New Start Finds No Open Demand.
    abandoned.await.unwrap();
    binding.start_samples().await.unwrap();
    binding.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
