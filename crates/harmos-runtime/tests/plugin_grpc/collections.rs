/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Collection Window Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use prost_reflect::ReflectMessage;
use tokio::sync::mpsc;
use tonic::{Code, Status};

// Local Imports
use super::{
    invoke_support::HostFixture,
    support::{self, Remote},
};
use harmos_protocol::{
    host::TimeGetResponse,
    model,
    plugin::{self, instance_samples_request, instance_samples_response},
};
use harmos_runtime::clock::Clock;
use harmos_runtime::plugin::grpc::{Error, Placement, PlacementSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Fixture {
    remote: Remote,
    ticks: Arc<Mutex<BTreeMap<String, usize>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    async fn start(fail: bool, silent: bool) -> Self {
        let remote = Remote::start().await;
        let pool = TimeGetResponse::default().descriptor().parent_pool().clone();
        let mut declarations = model::Declarations {
            types: Some(prost_types::FileDescriptorSet {
                file: pool.files().map(|file| file.file_descriptor_proto().clone()).collect(),
            }),
            streams: ["alpha", "beta", "silent"]
                .map(|id| model::StreamDeclaration {
                    id: id.into(),
                    collection: "alpha".into(),
                    row: "harmos.host.v1.TimeGetResponse".into(),
                    interval_setting: "interval".into(),
                    window_setting: if id == "silent" { "window".into() } else { String::new() },
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        };
        declarations.sections.push(model::Section {
            r#type: "harmos.model.v1.SettingsDeclarations".into(),
            payload: model::SettingsDeclarations { values: [("interval", 1), ("window", 4)].map(|(name, seconds)| model::SettingDeclaration {
                name: name.into(), default: Some(model::SettingValue { value: Some(model::setting_value::Value::Duration(prost_types::Duration { seconds, nanos: 0 })) }),
                ..Default::default()
            }).into() }.encode_to_vec(),
        });
        declarations.digest = harmos_protocol::schema::declarations_digest(&declarations);
        remote.declarations(declarations);
        let ticks = Arc::new(Mutex::new(BTreeMap::new()));
        let counts = ticks.clone();
        remote.samples(move |mut input| {
            let (output, receiver) = mpsc::channel(1);
            let counts = counts.clone();
            tokio::spawn(async move {
                let Some(instance_samples_request::Frame::Open(open)) = input.message().await.unwrap().unwrap().frame else {
                    panic!("Expected Open");
                };
                assert_eq!(open.stream, "alpha");
                while let Some(request) = input.message().await.unwrap() {
                    let frame = match request.frame.unwrap() {
                        instance_samples_request::Frame::Demand(_) => {
                            instance_samples_response::Frame::Demand(plugin::CollectionDemandResponse {})
                        }
                        instance_samples_request::Frame::Window(window) => {
                            for offset in 0..window.count {
                                *counts.lock().unwrap().entry(open.scope.clone()).or_default() += 1;
                                let row = TimeGetResponse {
                                    now: Some(prost_types::Timestamp {
                                        seconds: (window.sequence + offset) as i64,
                                        nanos: 0,
                                    }),
                                }
                                .encode_to_vec();
                                let response = plugin::CollectionBatch {
                                    offset,
                                    last: offset + 1 == window.count,
                                    streams: if silent {
                                        vec![]
                                    } else {
                                        vec![
                                            plugin::CollectionRows {
                                                stream: "alpha".into(),
                                                rows: vec![row.clone()],
                                            },
                                            plugin::CollectionRows {
                                                stream: "beta".into(),
                                                rows: vec![row.clone(), row.clone(), row],
                                            },
                                        ]
                                    },
                                };
                                if fail && offset == window.count - 1 {
                                    let _ = output.send(Err(Status::failed_precondition("Read Device: reconnect the device"))).await;
                                    return;
                                }
                                if output
                                    .send(Ok(plugin::InstanceSamplesResponse {
                                        samples: vec![],
                                        frame: Some(instance_samples_response::Frame::Collection(response)),
                                    }))
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            continue;
                        }
                        _ => panic!("Expected Collection Demand or Window"),
                    };
                    if output.send(Ok(plugin::InstanceSamplesResponse { samples: vec![], frame: Some(frame) })).await.is_err() {
                        return;
                    }
                }
            });
            Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
        });
        Self { remote, ticks }
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn stored(host: &HostFixture, count: usize) {
    loop {
        let changed = host.signals.storage_ingested.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if host.rows().len() >= count {
            return;
        }
        changed.await;
    }
}

fn settings() -> PlacementSettings {
    PlacementSettings {
        interval_window: Duration::from_secs(4),
        ..support::settings()
    }
}

/* === Tests === */

#[tokio::test]
async fn collection_ticks_are_shared_sparse_ordered_and_host_stamped() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let origin = host.clock.read().instant;
    let watches = [host.watch("alpha"), host.watch("beta"), host.watch("silent")];
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    stored(&host, 16).await;
    assert_eq!(fixture.ticks.lock().unwrap().get(""), Some(&4));
    let rows = host.stored.lock().unwrap();
    for (id, per_tick) in [("alpha", 1), ("beta", 3), ("silent", 0)] {
        let actual = rows
            .iter()
            .filter(|(definition, _)| definition.id() == id)
            .map(|(_, row)| row.published_unix_micros.unwrap())
            .collect::<Vec<_>>();
        let expected = (1..=4).flat_map(|tick| std::iter::repeat_n(tick * 1_000_000, per_tick)).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
    drop(rows);
    drop(watches);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_handler_failure_discards_the_whole_unanswered_window() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(true, false).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let origin = host.clock.read().instant;
    let watches = [host.watch("alpha"), host.watch("beta")];
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    let mut status = attached.status();
    let status = status.wait_for(|status| status.failure.is_some()).await.unwrap();
    assert!(matches!(status.failure.as_deref(), Some(Error::WindowFailed { status: Code::FailedPrecondition, .. })));
    assert!(host.rows().is_empty());
    drop(status);
    drop(watches);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_scopes_share_members_but_keep_independent_clocks_and_selection() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let (first_clock, first_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let (second_clock, second_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let first = host.runtime.bind_grpc(&attached.activation().unwrap(), "first", first_clock).await.unwrap();
    let second = host.runtime.bind_grpc(&attached.activation().unwrap(), "second", second_clock).await.unwrap();
    first.start_samples(["alpha"]).await.unwrap();
    second.start_samples(["beta"]).await.unwrap();
    first_time.advance_by(Duration::from_secs(4)).await;
    stored(&host, 4).await;
    assert_eq!(*fixture.ticks.lock().unwrap(), BTreeMap::from([("first".into(), 4)]));
    second_time.advance_by(Duration::from_secs(4)).await;
    stored(&host, 16).await;
    assert_eq!(*fixture.ticks.lock().unwrap(), BTreeMap::from([("first".into(), 4), ("second".into(), 4)]));
    assert!(host.stored.lock().unwrap().iter().all(|(definition, row)| {
        let scope = serde_json::from_str::<serde_json::Value>(row.context.as_ref().unwrap()).unwrap();
        (definition.id() == "alpha" && scope["scope"] == "first") || (definition.id() == "beta" && scope["scope"] == "second")
    }));
    first.close().await.unwrap();
    second.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn entirely_silent_collection_windows_complete_without_publication() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, true).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let watching = host.watch("silent");
    let origin = host.clock.read().instant;
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    // Registration of the Next Window Proves the Empty Answer Settled.
    host.clock.registered_at(origin + Duration::from_secs(8)).await;
    assert_eq!(fixture.ticks.lock().unwrap().get(""), Some(&4));
    assert!(host.rows().is_empty());
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_window_uses_the_shortest_member_even_when_that_member_is_idle() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false).await;
    let settings = PlacementSettings { interval_window: Duration::from_secs(8), ..settings() };
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings).await.unwrap();
    let watching = host.watch("alpha");
    let origin = host.clock.read().instant;
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    stored(&host, 4).await;
    assert_eq!(fixture.ticks.lock().unwrap().get(""), Some(&4));
    assert!(host.stored.lock().unwrap().iter().all(|(definition, _)| definition.id() == "alpha"));
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
