/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Sample Tick Child Call Journeys

/* === Imports === */

use super::{
    invoke_support::{Host, Reply, payload},
    manager::support::{self as manager, Backends, Host as ManagedHost},
    support::Remote,
};
use futures_util::stream;
use harmos_operations::{
    Failure,
    operations::{OperationSpec, WireDeclaration},
};
use harmos_protocol::{
    client,
    plugin::{self, instance_samples_request as request, instance_samples_response as response},
    v1::{self, child_response},
};
use harmos_runtime::{
    clock::Clock,
    operations::{
        ExecutionContext, FunctionContext, Grpc, HostContext, OperationFactory, OperationPolicy, ProtobufOperation, ProtobufOutput,
        Resource, RetryPolicy, ServingError, Visibility,
    },
    plugin::grpc::CallingPlugin,
};
use prost::Message;
use prost_reflect::ReflectMessage;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};
use tokio::sync::{Notify, mpsc};

/* === Constants === */

static DECLARATION: WireDeclaration = WireDeclaration::request::<Host>(
    "fixture.tick_child",
    OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    },
)
.visibility(Visibility::Public)
.grpc(Grpc::new("fixture.plugin.host.v1", "TickChildren", "Call"));

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
    type Request = client::TimeGetRequest;
    type Response = client::TimeGetResponse;
    type Endpoints = ProtobufOutput<client::TimeGetResponse>;
    const ID: &'static str = "fixture.tick_watch";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(context: ExecutionContext, _: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        output
            .send(client::TimeGetResponse {
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

fn expected_failure() -> v1::Failure {
    v1::Failure {
        classification: v1::Classification::Transient as i32,
        code: "tick_failure".into(),
        operation: "fixture.tick_child".into(),
        plugin: "host".into(),
        message: "fixture message".into(),
        diagnostics: "fixture diagnostics".into(),
        details: [("key".into(), "value".into())].into(),
        cause: Some(Box::new(v1::Failure {
            code: "cause".into(),
            ..Default::default()
        })),
        ..Default::default()
    }
}
fn operation(signals: Arc<Signals>) -> OperationFactory {
    OperationFactory::function(
        OperationSpec::<Host>::new(&DECLARATION),
        const { &[Resource::of::<CallingPlugin>(), Resource::of::<HostContext>()] },
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
                    let clock = context.resource::<HostContext>().unwrap().borrow().scope().clock().clone();
                    clock.sleep(Duration::from_secs(60)).await;
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
async fn frame(output: &mpsc::Sender<Result<plugin::InstanceSamplesResponse, tonic::Status>>, frame: response::Frame) {
    output.send(Ok(plugin::InstanceSamplesResponse { samples: vec![], frame: Some(frame) })).await.unwrap();
}
async fn child(
    output: &mpsc::Sender<Result<plugin::InstanceSamplesResponse, tonic::Status>>, call: u64, operation: &str, fingerprint: Vec<u8>,
    payload: Vec<u8>,
) {
    frame(
        output,
        response::Frame::Child(v1::ChildRequest {
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
async fn samples_children_correlate_preserve_failures_and_end_with_the_tick() {
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let mut host = ManagedHost::with_clock(root.path(), &backends, clock).await;
    let signals = Arc::new(Signals::default());
    let factory = operation(signals.clone());
    let contract = factory.contract().unwrap();
    let binding = contract.grpc().unwrap();
    let descriptor = harmos_runtime::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, &contract)]).unwrap();
    let method = descriptor.methods().next().unwrap();
    let fingerprint = harmos_protocol::schema::fingerprint(
        "fixture.tick_child",
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
    let pool = client::TimeGetResponse::default().descriptor().parent_pool().clone();
    let mut files: BTreeMap<_, _> = pool.files().map(|file| (file.name().to_owned(), file.file_descriptor_proto().clone())).collect();
    files.extend(descriptor.parent_pool().files().map(|file| (file.name().to_owned(), file.file_descriptor_proto().clone())));
    let types = prost_types::FileDescriptorSet { file: files.into_values().collect() };
    let settings_id = "harmos.plugin.host.v1.Settings/List";
    let settings_fingerprint = harmos_protocol::schema::fingerprint(
        settings_id,
        &types,
        "harmos.plugin.host.v1.SettingsListRequest",
        "harmos.plugin.host.v1.SettingsListResponse",
    )
    .unwrap();
    let clock_id = "harmos.plugin.host.v1.Time/Sleep";
    let clock_fingerprint = harmos_protocol::schema::fingerprint(
        clock_id,
        &types,
        "harmos.plugin.host.v1.TimeSleepRequest",
        "harmos.plugin.host.v1.TimeSleepResponse",
    )
    .unwrap();
    let mut catalog = v1::Catalog {
        types: Some(types),
        requires: vec![
            v1::Requirement {
                id: Watch::ID.into(),
                fingerprint: watch_fingerprint.clone(),
            },
            v1::Requirement {
                id: "fixture.tick_child".into(),
                fingerprint: fingerprint.clone(),
            },
            v1::Requirement {
                id: settings_id.into(),
                fingerprint: settings_fingerprint.clone(),
            },
            v1::Requirement {
                id: clock_id.into(),
                fingerprint: clock_fingerprint.clone(),
            },
        ],
        streams: ["alpha", "beta", "outside", "device", "duplicate", "mixed", "unknown_demand"]
            .map(|id| v1::StreamDeclaration {
                id: id.into(),
                row: "harmos.client.v1.TimeGetResponse".into(),
                interval: (id != "device").then_some(prost_types::Duration { seconds: 1, nanos: 0 }),
                ..Default::default()
            })
            .into(),
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
    let remote = Remote::start().await;
    remote.catalog(catalog);
    let peer_signals = signals.clone();
    remote.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(32);
        let signals = peer_signals.clone();
        let fingerprint = fingerprint.clone();
        let settings_fingerprint = settings_fingerprint.clone();
        let clock_fingerprint = clock_fingerprint.clone();
        let watch_fingerprint = watch_fingerprint.clone();
        tokio::spawn(async move {
            let Some(request::Frame::Open(open)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected Open");
            };
            if open.stream == "outside" || open.stream == "device" {
                child(&output, 1, "fixture.tick_child", fingerprint, payload(b"outside")).await;
                assert!(input.message().await.unwrap().is_none());
                signals.refusals.fetch_add(1, Ordering::SeqCst);
                if open.stream == "outside" {
                    signals.rejected.notify_one();
                }
                return;
            }
            let mut sequence = 0;
            while let Some(next) = input.message().await.unwrap() {
                let Some(request::Frame::Tick(tick)) = next.frame else {
                    panic!("Expected Tick");
                };
                sequence += 1;
                if open.stream == "duplicate" {
                    child(&output, 1, "fixture.tick_child", fingerprint.clone(), payload(b"duplicate")).await;
                    if sequence == 1 {
                        let Some(request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                            panic!("Expected Child Reply");
                        };
                        assert_eq!(reply.call, 1);
                        output
                            .send(Ok(plugin::InstanceSamplesResponse {
                                samples: vec![v1::Sample {
                                    at: tick.at,
                                    sequence,
                                    row: client::TimeGetResponse { now: tick.at }.encode_to_vec(),
                                }],
                                frame: None,
                            }))
                            .await
                            .unwrap();
                        continue;
                    }
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                if open.stream == "mixed" {
                    output
                        .send(Ok(plugin::InstanceSamplesResponse {
                            samples: vec![v1::Sample { at: tick.at, sequence, row: vec![] }],
                            frame: Some(response::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
                        }))
                        .await
                        .unwrap();
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                if open.stream == "unknown_demand" {
                    frame(&output, response::Frame::ChildDemand(v1::ChildDemand { call: 99 })).await;
                    assert!(input.message().await.unwrap().is_none());
                    signals.refusals.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                let base = sequence * 10;
                let key = format!("{}:{sequence}", open.stream);
                // Equal IDs On Independent Streams Are Independent; IDs Change On Later Ticks.
                child(&output, base + 1, "fixture.tick_child", fingerprint.clone(), payload(format!("first:{key}").as_bytes())).await;
                child(&output, base + 2, "fixture.tick_child", fingerprint.clone(), payload(b"second")).await;
                child(&output, base + 3, "fixture.tick_child", fingerprint.clone(), payload(b"fail")).await;
                child(&output, base + 4, settings_id, settings_fingerprint.clone(), vec![]).await;
                let cancelled = format!("{key}:cancel");
                child(&output, base + 5, "fixture.tick_child", fingerprint.clone(), payload(format!("park:{cancelled}").as_bytes())).await;
                signals.parked(&cancelled).entered.notified().await;
                frame(&output, response::Frame::ChildCancel(v1::ChildCancel { call: base + 5 })).await;
                signals.parked(&cancelled).cancelled.notified().await;
                let ended = format!("{key}:end");
                child(&output, base + 6, "fixture.tick_child", fingerprint.clone(), payload(format!("park:{ended}").as_bytes())).await;
                signals.parked(&ended).entered.notified().await;
                child(
                    &output,
                    base + 7,
                    clock_id,
                    clock_fingerprint.clone(),
                    plugin::host::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 0, nanos: 250_000_000 }),
                    }
                    .encode_to_vec(),
                )
                .await;
                frame(
                    &output,
                    response::Frame::Child(v1::ChildRequest {
                        call: base + 8,
                        operation: Watch::ID.into(),
                        fingerprint: watch_fingerprint.clone(),
                        watch: true,
                        max_response_bytes: 4096,
                        ..Default::default()
                    }),
                )
                .await;
                frame(&output, response::Frame::ChildDemand(v1::ChildDemand { call: base + 8 })).await;
                let mut replies = BTreeMap::new();
                for _ in 0..6 {
                    let Some(request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                        panic!("Expected Child Reply");
                    };
                    assert!(replies.insert(reply.call, reply.result.clone()).is_none());
                    if reply.call == base + 8 {
                        let Some(child_response::Result::Item(item)) = &reply.result else {
                            panic!("Expected Watch Item");
                        };
                        assert_eq!(client::TimeGetResponse::decode(item.as_slice()).unwrap().now.unwrap().seconds, 5);
                        frame(&output, response::Frame::ChildCancel(v1::ChildCancel { call: base + 8 })).await;
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
                assert!(plugin::host::SettingsListResponse::decode(settings.as_slice()).unwrap().value.unwrap().values.is_empty());
                assert!(matches!(&replies[&(base + 7)], Some(child_response::Result::Payload(_))));
                let at = tick.at.unwrap();
                output
                    .send(Ok(plugin::InstanceSamplesResponse {
                        samples: vec![v1::Sample {
                            at: Some(at),
                            sequence,
                            row: client::TimeGetResponse { now: Some(at) }.encode_to_vec(),
                        }],
                        frame: None,
                    }))
                    .await
                    .unwrap();
                signals.parked(&ended).cancelled.notified().await;
                signals.answered.fetch_add(1, Ordering::SeqCst);
            }
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let mut watch = host.client.watch(client::PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(manager::remote("tick-owner", &remote.address)).await.unwrap();
    manager::state(&mut watch, "tick-owner", client::PluginState::Ready, 1).await;
    signals.rejected.notified().await;
    let mut feed = host.runtime.streams.follow(harmos_runtime::storage::Cursor::ORIGIN);
    let collect = async {
        let mut rows = 0;
        while rows < 4 {
            rows +=
                feed.next().await.unwrap().publications().filter(|(definition, _, _)| ["alpha", "beta"].contains(&definition.id())).count();
        }
        drop(feed);
    };
    let drive = controller.advance_until(Duration::from_secs(10), || {
        signals.answered.load(Ordering::SeqCst) >= 4 && signals.refusals.load(Ordering::SeqCst) == 5
    });
    let ((), result) = tokio::join!(collect, drive);
    result.unwrap();
    assert!(signals.callers.lock().unwrap().iter().all(|(entry, reported)| entry.as_deref() == Some("tick-owner") && reported == "remote"));
    host.close().await;
}
