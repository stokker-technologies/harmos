/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Sample Window Child Call Journeys

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use prost_reflect::ReflectMessage;
use tokio::sync::{Notify, mpsc, mpsc::Sender};
use tonic::Status;

// Local Imports
use super::manager;
use super::manager::support::Backends;
use super::{
    invoke_support::{Host, Reply},
    support::{self, Remote},
};
use harmos_operations::{
    Failure,
    operations::{OperationSpec, WireDeclaration},
};
use harmos_protocol::host;
use harmos_protocol::host::TimeGetRequest;
use harmos_protocol::host::TimeGetResponse;
use harmos_protocol::model;
use harmos_protocol::plugin;
use harmos_protocol::plugin::ChildCancel;
use harmos_protocol::plugin::ChildDemand;
use harmos_protocol::plugin::ChildRequest;
use harmos_protocol::plugin::InstanceSamplesResponse;
use harmos_protocol::plugin::child_response;
use harmos_protocol::plugin::instance_samples_request;
use harmos_protocol::plugin::instance_samples_response;
use harmos_runtime::{
    clock::Clock,
    operations::{
        ExecutionContext, FunctionContext, Grpc, OperationFactory, OperationPolicy, ProtobufOperation, ProtobufOutput, Resource,
        RetryPolicy, ServingError, Visibility,
    },
    plugin::grpc::CallingPlugin,
};

/* === Constants === */

static DECLARATION: WireDeclaration = WireDeclaration::request::<Host>(
    "fixture.window_child",
    OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(Visibility::Public)
.grpc(Grpc::new("fixture.host.v1", "WindowChildren", "Call"));

/* === Structs === */

#[derive(Default)]
struct Signals {
    first: Mutex<BTreeMap<String, Arc<Notify>>>,
    parked: Mutex<BTreeMap<String, Arc<Parked>>>,
    callers: Mutex<Vec<(Option<String>, String)>>,
    answered: AtomicUsize,
    rejected: Notify,
    refusals: AtomicUsize,
}
#[derive(Default)]
struct Parked {
    entered: Notify,
    cancelled: Notify,
}
struct Cancelled(Arc<Parked>);
struct Watch;

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = TimeGetRequest;
    type Response = TimeGetResponse;
    type Endpoints = ProtobufOutput<TimeGetResponse>;
    const ID: &'static str = "fixture.window_watch";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(context: ExecutionContext, _: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        output
            .send(host::TimeGetResponse {
                now: Some(prost_types::Timestamp { seconds: 5, nanos: 0 }),
            })
            .await
            .map_err(ServingError::Stream)?;
        context.cancellation.cancelled().await;
        Err(ServingError::Closing)
    }
}
impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.cancelled.notify_one();
    }
}
impl Signals {
    fn first(&self, key: &str) -> Arc<Notify> {
        self.first.lock().unwrap().entry(key.into()).or_default().clone()
    }
    fn parked(&self, key: &str) -> Arc<Parked> {
        self.parked.lock().unwrap().entry(key.into()).or_default().clone()
    }
}

/* === Private Functions === */

fn payload(value: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    harmos_operations::execution::grpc::encode(&Host { value: value.into() }, &mut bytes).unwrap();
    bytes
}
fn expected_failure() -> model::Failure {
    model::Failure {
        classification: model::Classification::Transient as i32,
        code: "window_failure".into(),
        operation: "fixture.window_child".into(),
        plugin: "host".into(),
        message: "fixture message".into(),
        diagnostics: "fixture diagnostics".into(),
        details: [("key".into(), "value".into())].into(),
        cause: Some(Box::new(model::Failure {
            code: "cause".into(),
            ..Default::default()
        })),
        ..Default::default()
    }
}
fn operation(signals: Arc<Signals>) -> OperationFactory {
    OperationFactory::function(
        OperationSpec::<Host>::new(&DECLARATION),
        const { &[Resource::of::<CallingPlugin>()] },
        move |request: Host, context: FunctionContext<Host>| {
            let signals = signals.clone();
            async move {
                let caller = context.resource::<CallingPlugin>().unwrap();
                signals.callers.lock().unwrap().push((caller.name.clone(), caller.description.id.clone()));
                let value = String::from_utf8(request.value.clone()).unwrap();
                if let Some(key) = value.strip_prefix("first:") {
                    signals.first(key).notified().await;
                }
                if let Some(key) = value.strip_prefix("park:") {
                    let parked = signals.parked(key);
                    let _cancelled = Cancelled(parked.clone());
                    parked.entered.notify_one();
                    std::future::pending::<()>().await;
                }
                if value == "fail" {
                    return Err(Failure::from(expected_failure()));
                }
                Ok(harmos_runtime::Response::new(Reply { value: request.value }))
            }
        },
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
    .unwrap()
}
async fn frame(output: &Sender<Result<InstanceSamplesResponse, Status>>, frame: instance_samples_response::Frame) {
    output.send(Ok(plugin::InstanceSamplesResponse { samples: vec![], frame: Some(frame) })).await.unwrap();
}
async fn child(
    output: &Sender<Result<InstanceSamplesResponse, Status>>, call: u64, operation: &str, fingerprint: Vec<u8>, payload: Vec<u8>,
) {
    frame(
        output,
        instance_samples_response::Frame::Child(ChildRequest {
            call,
            operation: operation.into(),
            fingerprint,
            payload,
            ..Default::default()
        }),
    )
    .await;
}

/* === Tests === */

#[tokio::test]
async fn samples_children_correlate_preserve_failures_and_end_with_the_window() {
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let mut host = manager::support::Host::with_clock(root.path(), &backends, clock).await;
    let signals = Arc::new(Signals::default());
    let factory = operation(signals.clone());
    let contract = factory.contract().unwrap();
    let binding = contract.grpc().unwrap();
    let descriptor = harmos_runtime::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, &contract)]).unwrap();
    let method = descriptor.methods().next().unwrap();
    let fingerprint = harmos_protocol::schema::fingerprint(
        "fixture.window_child",
        &prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        },
        method.input().full_name(),
        method.output().full_name(),
    )
    .unwrap();
    let watch_factory = OperationFactory::protobuf::<Watch>().unwrap();
    let watch_fingerprint = watch_factory.contract().unwrap().fingerprint().0.to_vec();
    host.runtime.operations.register_operation(watch_factory, Default::default()).await.unwrap();
    host.runtime.operations.register_operation(factory, Default::default()).await.unwrap();
    let pool = host::TimeGetResponse::default().descriptor().parent_pool().clone();
    let mut files: BTreeMap<_, _> = pool.files().map(|file| (file.name().to_owned(), file.file_descriptor_proto().clone())).collect();
    files.extend(descriptor.parent_pool().files().map(|file| (file.name().to_owned(), file.file_descriptor_proto().clone())));
    files.extend(support::clock_types().file.into_iter().map(|file| (file.name.clone().unwrap(), file)));
    let types = prost_types::FileDescriptorSet { file: files.into_values().collect() };
    let settings_id = "harmos.plugin.v1.plugins.Settings/List";
    let settings_fingerprint = harmos_protocol::schema::fingerprint(
        settings_id,
        &types,
        "harmos.host.v1.plugins.SettingsListRequest",
        "harmos.host.v1.plugins.SettingsListResponse",
    )
    .unwrap();
    let clock_id = "harmos.plugin.v1.Time/Sleep";
    let clock_fingerprint =
        harmos_protocol::schema::fingerprint(clock_id, &types, "harmos.host.v1.TimeSleepRequest", "harmos.host.v1.TimeSleepResponse")
            .unwrap();
    let mut catalog = model::Declarations {
        types: Some(types),
        requires: vec![
            model::Requirement {
                id: Watch::ID.into(),
                fingerprint: watch_fingerprint.clone(),
            },
            model::Requirement {
                id: "fixture.window_child".into(),
                fingerprint: fingerprint.clone(),
            },
            model::Requirement {
                id: settings_id.into(),
                fingerprint: settings_fingerprint.clone(),
            },
            model::Requirement {
                id: clock_id.into(),
                fingerprint: clock_fingerprint.clone(),
            },
        ],
        streams: [
            "fixture.alpha",
            "fixture.beta",
            "fixture.outside",
            "fixture.device",
            "fixture.duplicate",
            "fixture.mixed",
            "fixture.unknown_demand",
        ]
        .map(|id| model::StreamDeclaration {
            id: id.into(),
            row: "fixture.ClockRow".into(),
            interval: (id != "fixture.device").then_some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        })
        .into(),
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
    let remote = Remote::start().await;
    remote.declarations(catalog);
    let peer_signals = signals.clone();
    remote.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(32);
        let signals = peer_signals.clone();
        let fingerprint = fingerprint.clone();
        let settings_fingerprint = settings_fingerprint.clone();
        let clock_fingerprint = clock_fingerprint.clone();
        let watch_fingerprint = watch_fingerprint.clone();
        tokio::spawn(async move {
            let Some(instance_samples_request::Frame::Open(open)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected Open");
            };
            if open.stream == "fixture.outside" || open.stream == "fixture.device" {
                child(&output, 1, "fixture.window_child", fingerprint, payload(b"fixture.outside")).await;
                assert!(input.message().await.unwrap().is_none());
                signals.refusals.fetch_add(1, Ordering::SeqCst);
                if open.stream == "fixture.outside" {
                    signals.rejected.notify_one();
                }
                return;
            }
            let mut sequence = 0;
            while let Some(next) = input.message().await.unwrap() {
                let Some(instance_samples_request::Frame::Window(window)) = next.frame else {
                    panic!("Expected a Window");
                };
                sequence += 1;
                if open.stream == "fixture.duplicate" {
                    child(&output, 1, "fixture.window_child", fingerprint.clone(), payload(b"fixture.duplicate")).await;
                    if sequence == 1 {
                        let Some(instance_samples_request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                            panic!("Expected Child Reply");
                        };
                        assert_eq!(reply.call, 1);
                        output.send(Ok(support::answer(1, host::TimeGetResponse { now: window.start }.encode_to_vec()))).await.unwrap();
                        continue;
                    }
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                if open.stream == "fixture.mixed" {
                    output
                        .send(Ok(plugin::InstanceSamplesResponse {
                            samples: vec![plugin::Sample { at: window.start, sequence, row: vec![] }],
                            frame: Some(instance_samples_response::Frame::ChildCancel(ChildCancel { call: 1 })),
                        }))
                        .await
                        .unwrap();
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                if open.stream == "fixture.unknown_demand" {
                    frame(&output, instance_samples_response::Frame::ChildDemand(ChildDemand { call: 99 })).await;
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                let base = sequence * 10;
                let key = format!("{}:{sequence}", open.stream);
                // Equal IDs On Independent Streams Are Independent; IDs Change On Later Windows.
                child(&output, base + 1, "fixture.window_child", fingerprint.clone(), payload(format!("first:{key}").as_bytes())).await;
                child(&output, base + 2, "fixture.window_child", fingerprint.clone(), payload(b"second")).await;
                child(&output, base + 3, "fixture.window_child", fingerprint.clone(), payload(b"fail")).await;
                child(&output, base + 4, settings_id, settings_fingerprint.clone(), vec![]).await;
                let cancelled = format!("{key}:cancel");
                child(&output, base + 5, "fixture.window_child", fingerprint.clone(), payload(format!("park:{cancelled}").as_bytes()))
                    .await;
                signals.parked(&cancelled).entered.notified().await;
                frame(&output, instance_samples_response::Frame::ChildCancel(ChildCancel { call: base + 5 })).await;
                signals.parked(&cancelled).cancelled.notified().await;
                child(
                    &output,
                    base + 7,
                    clock_id,
                    clock_fingerprint.clone(),
                    host::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 0, nanos: 250_000_000 }),
                    }
                    .encode_to_vec(),
                )
                .await;
                frame(
                    &output,
                    instance_samples_response::Frame::Child(ChildRequest {
                        call: base + 8,
                        operation: Watch::ID.into(),
                        fingerprint: watch_fingerprint.clone(),
                        watch: true,
                        max_response_bytes: 4096,
                        ..Default::default()
                    }),
                )
                .await;
                frame(&output, instance_samples_response::Frame::ChildDemand(ChildDemand { call: base + 8 })).await;
                let mut replies = BTreeMap::new();
                for _ in 0..6 {
                    let Some(instance_samples_request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                        panic!("Expected Child Reply");
                    };
                    assert!(replies.insert(reply.call, reply.result.clone()).is_none());
                    if reply.call == base + 8 {
                        let Some(child_response::Result::Item(item)) = &reply.result else {
                            panic!("Expected Watch Item");
                        };
                        assert_eq!(host::TimeGetResponse::decode(item.as_slice()).unwrap().now.unwrap().seconds, 5);
                        frame(&output, instance_samples_response::Frame::ChildCancel(ChildCancel { call: base + 8 })).await;
                    }
                    if reply.call == base + 2 {
                        assert!(!replies.contains_key(&(base + 1)));
                        signals.first(&key).notify_one();
                    }
                }
                assert_eq!(replies[&(base + 1)], Some(child_response::Result::Payload(payload(format!("first:{key}").as_bytes()))));
                assert_eq!(replies[&(base + 2)], Some(child_response::Result::Payload(payload(b"second"))));
                assert_eq!(replies[&(base + 3)], Some(child_response::Result::Failure(expected_failure())));
                let Some(child_response::Result::Payload(settings)) = &replies[&(base + 4)] else {
                    panic!("Expected Settings");
                };
                assert!(host::plugins::SettingsListResponse::decode(settings.as_slice()).unwrap().value.unwrap().values.is_empty());
                assert!(matches!(&replies[&(base + 7)], Some(child_response::Result::Payload(_))));
                let ended = format!("{key}:end");
                child(&output, base + 6, "fixture.window_child", fingerprint.clone(), payload(format!("park:{ended}").as_bytes())).await;
                signals.parked(&ended).entered.notified().await;
                let at = window.start.unwrap();
                output.send(Ok(support::answer(1, host::TimeGetResponse { now: Some(at) }.encode_to_vec()))).await.unwrap();
                signals.parked(&ended).cancelled.notified().await;
                signals.answered.fetch_add(1, Ordering::SeqCst);
            }
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let mut watch = host.client.watch(host::PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(manager::support::remote("window_owner", &remote.address)).await.unwrap();
    manager::support::state(&mut watch, "window_owner", host::PluginState::Ready, 1).await;
    drop(watch);
    // The Unscoped Demand Grants Windows Only on Watched Streams.
    let _watching = ["fixture.alpha", "fixture.beta", "fixture.outside", "fixture.duplicate", "fixture.mixed", "fixture.unknown_demand"]
        .map(|id| host.runtime.streams.current_publication(id).unwrap().watch());
    signals.rejected.notified().await;
    let mut feed = host.runtime.streams.follow(harmos_runtime::storage::Cursor::ORIGIN);
    let collect = async {
        let mut rows = 0;
        while rows < 4 {
            rows += feed
                .next()
                .await
                .unwrap()
                .publications()
                .filter(|(definition, _, _)| ["fixture.alpha", "fixture.beta"].contains(&definition.id()))
                .count();
        }
        drop(feed);
    };
    let drive = controller.advance_until(Duration::from_secs(10), || {
        signals.answered.load(Ordering::SeqCst) >= 4 && signals.refusals.load(Ordering::SeqCst) == 5
    });
    let collected = async {
        collect.await;
        Ok(())
    };
    tokio::try_join!(collected, drive).unwrap();
    assert!(
        signals.callers.lock().unwrap().iter().all(|(entry, reported)| entry.as_deref() == Some("window_owner") && reported == "fixture")
    );
    host.close().await;
}
