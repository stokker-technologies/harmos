/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Instance Lane Admission Stories.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    path::Path,
    process::Stdio,
    sync::{Arc, Mutex},
};

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
    sync::{mpsc, oneshot},
};

// Local Imports
use super::{invoke_support, support};
use harmos::Visibility;
use harmos::plugin::{Context, Lane, Operation, Plugin};
use harmos_protocol::{
    model::{LaneDeclaration, lane_declaration},
    schema,
};
use harmos_runtime::{
    Runtime,
    plugin::grpc::{Attached, Placement},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Message)]
struct Marker {
    value: Vec<u8>,
}

#[derive(Default)]
struct Signals {
    entered: Vec<String>,
    release: BTreeMap<String, oneshot::Sender<()>>,
}

struct Fixture {
    id: String,
    address: String,
    python: Option<(Child, Lines<BufReader<ChildStdout>>)>,
    listener: Option<harmos::plugin::Listener>,
    signals: Arc<Mutex<Signals>>,
    events: mpsc::UnboundedReceiver<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    async fn start(language: &str, id: &str, plugin_id: &str) -> Self {
        let signals = Arc::new(Mutex::new(Signals::default()));
        let (events, receiving) = mpsc::unbounded_channel();
        if language == "python" {
            let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/python");
            let executable = if cfg!(target_os = "windows") { ".venv/Scripts/python.exe" } else { ".venv/bin/python" };
            let mut process = Command::new(sdk.join(executable))
                .args(["-m", "tests.fixtures.lanes", id, plugin_id])
                .current_dir(sdk)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut output = BufReader::new(process.stdout.take().unwrap()).lines();
            let address = format!("http://{}", output.next_line().await.unwrap().unwrap().strip_prefix("listening:").unwrap());
            return Self {
                id: id.into(),
                address,
                python: Some((process, output)),
                listener: None,
                signals,
                events: receiving,
            };
        }
        let instrument = Lane::serial("Serial");
        let parallel = Lane::concurrent("parallel", 2);
        let other = Lane::serial("other");
        let mut plugin = Plugin::new(plugin_id, "1.0.0").lane(instrument.clone()).lane(parallel.clone()).lane(other.clone());
        for (name, lane) in [
            ("first", Some(&instrument)),
            ("second", Some(&instrument)),
            ("parallel", Some(&parallel)),
            ("other", Some(&other)),
            ("open", None),
        ] {
            let signals = signals.clone();
            let events = events.clone();
            let mut operation = Operation::new(format!("{id}.{name}"), async move |context: &Context, request: Marker| {
                let marker = String::from_utf8(request.value.clone()).unwrap();
                let (release, waiting) = oneshot::channel();
                {
                    let mut state = signals.lock().unwrap();
                    state.entered.push(marker.clone());
                    state.release.insert(marker.clone(), release);
                }
                events.send(marker).unwrap();
                tokio::select! { _ = waiting => {}, _ = context.cancelled() => {} }
                Ok(request)
            })
            .visibility(Visibility::Public);
            if let Some(lane) = lane {
                operation = operation.lane(lane);
            }
            plugin = plugin.register(|ctx| ctx.register(operation)).unwrap();
        }
        let listener = plugin.start("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.address());
        Self {
            id: id.into(),
            address,
            python: None,
            listener: Some(listener),
            signals,
            events: receiving,
        }
    }

    async fn command(&mut self, value: &str) {
        let (process, _) = self.python.as_mut().unwrap();
        let input = process.stdin.as_mut().unwrap();
        input.write_all(format!("{value}\n").as_bytes()).await.unwrap();
        input.flush().await.unwrap();
    }

    async fn event(&mut self, expected: &str) {
        let actual = match self.python.as_mut() {
            Some((_, output)) => output.next_line().await.unwrap().unwrap().strip_prefix("entered:").unwrap().to_owned(),
            None => self.events.recv().await.unwrap(),
        };
        assert_eq!(actual, expected);
    }

    async fn state(&mut self, expected: &[&str]) {
        let actual = if self.python.is_some() {
            self.command("state").await;
            let (_, output) = self.python.as_mut().unwrap();
            let line = output.next_line().await.unwrap().unwrap();
            line.strip_prefix("state:").unwrap().split(',').map(str::to_owned).collect::<Vec<_>>()
        } else {
            self.signals.lock().unwrap().entered.clone()
        };
        assert_eq!(actual, expected);
    }

    async fn release(&mut self, marker: &str) {
        if self.python.is_some() {
            self.command(marker).await;
        } else {
            self.signals.lock().unwrap().release.remove(marker).unwrap().send(()).unwrap();
        }
    }

    fn submit(&self, runtime: &Runtime<support::App>, name: &str, marker: &str) -> harmos_runtime::operations::EncodedInvocation {
        let payload = invoke_support::payload(marker.as_bytes());
        runtime.operations.submit_encoded(&format!("{}.{name}", self.id), payload).unwrap()
    }

    async fn close(mut self) {
        if self.python.is_some() {
            self.command("quit").await;
            assert!(self.python.as_mut().unwrap().0.wait().await.unwrap().success());
        }
        if let Some(listener) = &mut self.listener {
            listener.close().await.unwrap();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn attach(runtime: &Runtime<support::App>, fixture: &Fixture) -> Attached {
    runtime.attach_grpc(Placement::Remote(fixture.address.clone()), support::settings()).await.unwrap()
}

async fn story(language: &str) {
    let runtime = support::runtime(None).await;
    let mut fixture = Fixture::start(language, "lanes", "lanes").await;
    let attached = attach(&runtime, &fixture).await;

    // Serial Operations Share Capacity and Preserve Submission Order.
    let first = fixture.submit(&runtime, "first", "1");
    fixture.event("1").await;
    let second = fixture.submit(&runtime, "second", "2");
    let third = fixture.submit(&runtime, "first", "3");
    fixture.state(&["1"]).await;
    fixture.release("1").await;
    first.await.unwrap();
    fixture.event("2").await;
    fixture.release("2").await;
    second.await.unwrap();
    fixture.event("3").await;
    fixture.release("3").await;
    third.await.unwrap();

    // Width Two Admits Two and Queues the Third.
    let first = fixture.submit(&runtime, "parallel", "4");
    fixture.event("4").await;
    let second = fixture.submit(&runtime, "parallel", "5");
    fixture.event("5").await;
    let third = fixture.submit(&runtime, "parallel", "6");
    fixture.state(&["1", "2", "3", "4", "5"]).await;
    fixture.release("5").await;
    second.await.unwrap();
    fixture.event("6").await;
    fixture.release("4").await;
    fixture.release("6").await;
    first.await.unwrap();
    third.await.unwrap();

    // Different and Default Lanes Remain Independent.
    let first = fixture.submit(&runtime, "first", "7");
    fixture.event("7").await;
    let other = fixture.submit(&runtime, "other", "8");
    fixture.event("8").await;
    let open = fixture.submit(&runtime, "open", "9");
    fixture.event("9").await;

    // Cancellation Settles a Waiter Without Entering the Plugin.
    let cancelled = fixture.submit(&runtime, "second", "cancelled");
    let settlement = cancelled.settle();
    assert!(matches!(cancelled.cancel().await, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
    settlement.await.unwrap();
    fixture.state(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]).await;

    // Equal Lane Names in Different Plugin Instances Never Share Capacity.
    let mut peer = Fixture::start(language, "peer", "peer").await;
    let peer_attached = attach(&runtime, &peer).await;
    let independent = peer.submit(&runtime, "first", "peer");
    peer.event("peer").await;
    peer.release("peer").await;
    independent.await.unwrap();
    peer_attached.stop("Complete").await.unwrap();
    peer.close().await;
    // Two Instances of One Plugin Also Own Independent Capacity.
    let mut twin = Fixture::start(language, "twin", "lanes").await;
    let twin_attached = attach(&runtime, &twin).await;
    let independent = twin.submit(&runtime, "first", "twin");
    twin.event("twin").await;
    twin.release("twin").await;
    independent.await.unwrap();
    twin_attached.stop("Complete").await.unwrap();
    twin.close().await;
    fixture.release("8").await;
    fixture.release("9").await;
    other.await.unwrap();
    open.await.unwrap();

    // Stop Closes Admission Before Cancelling Running Work.
    let waiting = fixture.submit(&runtime, "second", "stopped");
    let (result, stopped) = tokio::join!(waiting, attached.stop("Complete"));
    assert!(matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Unavailable))));
    stopped.unwrap();
    let _ = first.await;
    fixture.state(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]).await;
    fixture.close().await;
    runtime.shutdown().await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn python_plugin_lane_stories() {
    story("python").await;
}

#[tokio::test]
async fn rust_plugin_lane_stories() {
    story("rust").await;
}

#[tokio::test]
async fn invalid_plugin_lane_facts_are_refused_at_admission() {
    let runtime = support::runtime(None).await;
    let remote = support::Remote::start().await;
    for (name, capacity, duplicate, assignment) in [
        ("", 1, false, ""),
        ("Open", 1, false, "Open"),
        ("harmos.resident", 1, false, "harmos.resident"),
        ("instrument", 1, true, "instrument"),
        ("instrument", 0, false, "instrument"),
        ("instrument", 1, false, "missing"),
    ] {
        let mut catalog = support::catalog();
        catalog.lanes.push(LaneDeclaration {
            name: name.into(),
            capacity: Some(lane_declaration::Capacity::Concurrent(capacity)),
        });
        if duplicate {
            catalog.lanes.push(catalog.lanes[0].clone());
        }
        catalog.operations[0].lane = assignment.into();
        let operation = catalog.operations[0].id.clone();
        catalog.digest = schema::declarations_digest(&catalog);
        remote.declarations(catalog);
        let error = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.err().unwrap().to_string();
        if !assignment.is_empty() {
            assert!(error.contains(assignment), "{error}");
        }
        assert!(error.contains(&operation), "{error}");
    }
    drop(remote);
    runtime.shutdown().await.unwrap();
}
