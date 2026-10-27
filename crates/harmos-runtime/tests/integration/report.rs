/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Qualify Native Span Lifetimes, Parent Tracks And Causal Flows

/* === Modules === */

#[path = "../support/report.rs"]
#[allow(dead_code, reason = "qualification uses isolated collectors instead of the system runner environment")]
mod capture;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

// Third Party Imports
use prost::Message;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;
use tracing_perfetto_file::PerfettoLayer;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

struct Unwritable;

#[cfg(feature = "grpc")]
#[harmos_runtime::message]
struct CodecTrace {
    bytes: Vec<u8>,
}

// Projections Of The Public Perfetto Wire Schema; Unknown Fields Are Skipped.
#[derive(Clone, PartialEq, Message)]
struct Trace {
    #[prost(message, repeated, tag = "1")]
    packets: Vec<Packet>,
}

#[derive(Clone, PartialEq, Message)]
struct Packet {
    #[prost(message, optional, tag = "11")]
    event: Option<Event>,

    #[prost(message, optional, tag = "60")]
    descriptor: Option<Descriptor>,
}

#[derive(Clone, PartialEq, Message)]
struct Event {
    #[prost(int32, tag = "9")]
    kind: i32,

    #[prost(uint64, tag = "11")]
    track: u64,

    #[prost(message, repeated, tag = "4")]
    annotations: Vec<Annotation>,

    #[prost(fixed64, repeated, packed = "false", tag = "47")]
    flows: Vec<u64>,

    #[prost(fixed64, repeated, packed = "false", tag = "48")]
    terminating: Vec<u64>,
}

#[derive(Clone, PartialEq, Message)]
struct Descriptor {
    #[prost(uint64, tag = "1")]
    uuid: u64,

    #[prost(string, tag = "2")]
    name: String,

    #[prost(message, optional, tag = "3")]
    process: Option<Process>,

    #[prost(uint64, tag = "5")]
    parent: u64,
}

#[derive(Clone, PartialEq, Message)]
struct Process {
    #[prost(int32, tag = "1")]
    pid: i32,

    #[prost(string, repeated, tag = "2")]
    command: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
struct Annotation {
    #[prost(uint64, optional, tag = "3")]
    unsigned: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for Unwritable {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("controlled trace output failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("controlled trace output failure"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "grpc")]
#[test]
fn codec_stage_lifetimes() {
    let output = Output::default();
    let (layer, guard) = PerfettoLayer::builder(output.clone()).with_debug_annotations().build();
    let layer = layer.with_filter(filter_fn(capture::instrumented));
    let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(layer));
    let secret = b"codec-payload-sentinel";
    tracing::dispatcher::with_default(&dispatch, || {
        let value = CodecTrace { bytes: secret.to_vec() };
        let mut bytes = Vec::new();
        harmos_sdk::grpc::encode(&value, &mut bytes).unwrap();
        let input = harmos_sdk::grpc::DecodeInput::new(&bytes);
        let prepared = harmos_sdk::grpc::prepare::<CodecTrace>(input).unwrap();
        assert_eq!(prepared.decode().unwrap().bytes, secret);
    });

    guard.flush().unwrap();
    let bytes = output.0.lock().unwrap().clone();
    assert!(!bytes.windows(secret.len()).any(|window| window == secret));
    let trace = Trace::decode(bytes.as_slice()).unwrap();
    let descriptors = trace.packets.iter().filter_map(|packet| packet.descriptor.as_ref());
    let descriptors: Vec<_> = descriptors.collect();
    for name in ["grpc.protobuf.encode", "grpc.prepare", "grpc.postcard.decode"] {
        let stage = descriptors.iter().find(|descriptor| descriptor.name == name).unwrap();
        let events = trace.packets.iter().filter_map(|packet| packet.event.as_ref());
        let events: Vec<_> = events.filter(|event| event.track == stage.uuid).collect();
        let beginnings = events.iter().filter(|event| event.kind == 1).count();
        let endings = events.iter().filter(|event| event.kind == 2).count();
        assert!(beginnings > 0);
        assert_eq!(beginnings, endings);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_causal_lifetimes() {
    let disabled = capture::capture(None);
    drop(disabled);
    let output = Output::default();
    let (layer, guard) =
        PerfettoLayer::builder(output.clone()).with_debug_annotations().with_source_locations().with_poll_slices().with_counters().build();
    let guard = Arc::new(guard);
    let layer = layer.with_filter(filter_fn(capture::instrumented));
    let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(layer));
    let (root, source, child) = tracing::dispatcher::with_default(&dispatch, || {
        let root = tracing::debug_span!("system.story");
        let source = tracing::debug_span!(parent: &root, "operations.report_source");
        let child = tracing::debug_span!(parent: &root, "storage.report_child", request = 17u64);
        tracing::error!(target: "harmos::plugin::sidecar", stderr = "unrelated-payload-sentinel", "private application event");
        tracing::debug!(target: "harmos::carrier", remote_pid = 77u64, "sidecar process attached");
        child.follows_from(&source);
        (root, source, child)
    });
    let report = capture::Report {
        guard: Some(Arc::clone(&guard)),
        story: Some(root),
        dispatch: Some(dispatch.clone()),
    };
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, waiting) = tokio::sync::oneshot::channel();
    tracing::dispatcher::with_default(&dispatch, || drop(source));
    let work = async move {
        let action = async move {
            waiting.await.unwrap();
        };
        let mut action = Box::pin(action.instrument(child));
        let mut entered = Some(entered);
        std::future::poll_fn(|context| {
            let outcome = action.as_mut().poll(context);
            if let Some(entered) = entered.take() {
                entered.send(()).unwrap();
            }
            outcome
        })
        .await;
        drop(action);
    }
    .with_subscriber(dispatch.clone());
    let running = tokio::spawn(work);
    ready.await.unwrap();
    guard.flush().unwrap();
    let bytes = output.0.lock().unwrap().clone();
    let secret = b"unrelated-payload-sentinel";
    assert!(!bytes.windows(secret.len()).any(|window| window == secret));
    let open = Trace::decode(bytes.as_slice()).unwrap();
    let descriptors = open.packets.iter().map(|packet| &packet.descriptor);
    let descriptors: Vec<_> = descriptors.flatten().collect();
    let processes = descriptors.iter().map(|descriptor| &descriptor.process);
    let processes = processes.flatten();
    let process = i32::try_from(std::process::id()).unwrap();
    let native_process = processes.into_iter().find(|descriptor| descriptor.pid == process).unwrap();
    let arguments: Vec<_> = std::env::args().collect();
    assert_eq!(native_process.command, arguments);
    let track = |name: &str| {
        let matching = descriptors.iter().copied().find(|item| item.name == name);
        matching.unwrap()
    };
    let root_track = track("system.story").uuid;
    let source_track = track("operations.report_source").uuid;
    let child_track = track("storage.report_child").uuid;
    assert_eq!(track("storage.report_child").parent, root_track);
    assert_eq!(track("operations.report_source").parent, root_track);
    assert_ne!(source_track, child_track);
    let open_events = open.packets.iter().map(|packet| &packet.event);
    let open_events: Vec<_> = open_events.flatten().collect();
    let child_events: Vec<_> = open_events.iter().filter(|event| event.track == child_track).collect();
    let source_events: Vec<_> = open_events.iter().filter(|event| event.track == source_track).collect();
    assert!(source_events.iter().any(|event| event.kind == 2 && !event.flows.is_empty()));
    let beginnings = child_events.iter().filter(|event| event.kind == 1).count();
    let endings = child_events.iter().filter(|event| event.kind == 2).count();
    assert_eq!(beginnings, endings + 1, "the pending child keeps its lifetime slice open after its poll exits");
    let annotations = child_events.iter().flat_map(|event| &event.annotations);
    assert!(annotations.into_iter().any(|annotation| annotation.unsigned == Some(17)));
    let carrier_annotations = open_events.iter().flat_map(|event| &event.annotations);
    assert!(carrier_annotations.into_iter().any(|annotation| annotation.unsigned == Some(77)));

    release.send(()).unwrap();
    running.await.unwrap();
    drop(report);
    let bytes = output.0.lock().unwrap().clone();
    let closed = Trace::decode(bytes.as_slice()).unwrap();
    let events = closed.packets.iter().map(|packet| &packet.event);
    let events: Vec<_> = events.flatten().collect();
    for track in [root_track, source_track, child_track] {
        let slices: Vec<_> = events.iter().filter(|event| event.track == track).collect();
        let beginnings = slices.iter().filter(|event| event.kind == 1).count();
        let endings = slices.iter().filter(|event| event.kind == 2).count();
        assert!(endings > 0, "track {track} did not close: {events:?}");
        assert_eq!(beginnings, endings, "track {track} retained an open lifetime or poll slice");
    }
    let outgoing: Vec<_> = events.iter().filter(|event| event.track == source_track).flat_map(|event| &event.flows).collect();
    let incoming: Vec<_> = events.iter().filter(|event| event.track == child_track).flat_map(|event| &event.terminating).collect();
    assert!(!outgoing.is_empty());
    assert!(incoming.iter().any(|flow| outgoing.contains(flow)));
}

#[test]
fn flush_preserves_failure() {
    let (layer, guard) = PerfettoLayer::builder(Unwritable).build();
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        let _span = tracing::debug_span!("unwritable_report");
    });
    assert!(guard.flush().is_err());
    assert!(guard.flush().is_err());
    let report = capture::Report {
        guard: Some(Arc::new(guard)),
        story: None,
        dispatch: None,
    };
    let drop_report = || drop(report);
    let dropping = std::panic::AssertUnwindSafe(drop_report);
    assert!(std::panic::catch_unwind(dropping).is_err());
}

#[test]
fn unwind_flushes_artifact() {
    let output = Output::default();
    let (layer, guard) = PerfettoLayer::builder(output.clone()).build();
    let subscriber = tracing_subscriber::registry().with(layer);
    let unwind = std::panic::AssertUnwindSafe(|| {
        let _report = capture::Report {
            guard: Some(Arc::new(guard)),
            story: None,
            dispatch: None,
        };
        tracing::subscriber::with_default(subscriber, || {
            let _span = tracing::debug_span!("unwinding_report").entered();
            panic!("controlled story failure");
        });
    });
    assert!(std::panic::catch_unwind(unwind).is_err());

    let bytes = output.0.lock().unwrap().clone();
    let trace = Trace::decode(bytes.as_slice()).unwrap();
    let events = trace.packets.iter().map(|packet| &packet.event);
    let events: Vec<_> = events.flatten().collect();
    assert!(events.iter().any(|event| event.kind == 1));
    assert!(events.iter().any(|event| event.kind == 2));
}
