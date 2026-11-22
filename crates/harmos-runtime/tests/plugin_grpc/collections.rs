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
use std::iter;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use prost_types::Timestamp;
use serde_json::Value;
use tokio::sync::mpsc;
use tonic::{Code, Status};

// Local Imports
use super::{
    invoke_support::{HostFixture, pending},
    support::{self, Remote},
};
use harmos_protocol::{
    model,
    plugin::{self, instance_samples_request, instance_samples_response},
    schema,
};
use harmos_runtime::clock::{Clock, Source};
use harmos_runtime::operations::CallContext;
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
    async fn start(fail: bool, silent: bool, interval_nanos: i32) -> Self {
        let remote = Remote::start().await;
        let mut declarations = model::Declarations {
            types: Some(support::clock_types()),
            streams: ["fixture.alpha", "fixture.beta", "fixture.silent"]
                .map(|id| model::StreamDeclaration {
                    id: id.into(),
                    collection: "fixture.alpha".into(),
                    row: "fixture.ClockRow".into(),
                    interval_setting: "interval".into(),
                    window_setting: if id == "fixture.silent" { "window".into() } else { String::new() },
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        };
        declarations.sections.push(model::Section {
            r#type: "harmos.model.v1.SettingsDeclarations".into(),
            payload: model::SettingsDeclarations {
                values: [("interval", 1), ("window", 4)]
                    .map(|(name, seconds)| model::SettingDeclaration {
                        name: name.into(),
                        default: Some(model::SettingValue {
                            value: Some(model::setting_value::Value::Duration(prost_types::Duration {
                                seconds: if name == "interval" && interval_nanos > 0 { 0 } else { seconds },
                                nanos: if name == "interval" { interval_nanos } else { 0 },
                            })),
                        }),
                        ..Default::default()
                    })
                    .into(),
            }
            .encode_to_vec(),
        });
        declarations.digest = schema::declarations_digest(&declarations);
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
                assert_eq!(open.stream, "fixture.alpha");
                while let Some(request) = input.message().await.unwrap() {
                    let frame = match request.frame.unwrap() {
                        instance_samples_request::Frame::Demand(_) => {
                            instance_samples_response::Frame::Demand(plugin::CollectionDemandResponse {})
                        }
                        instance_samples_request::Frame::Window(window) => {
                            for offset in 0..window.count {
                                *counts.lock().unwrap().entry(open.scope.clone()).or_default() += 1;
                                let row = support::ClockRow {
                                    now: Some(Timestamp {
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
                                                stream: "fixture.alpha".into(),
                                                rows: vec![row],
                                            },
                                            plugin::CollectionRows {
                                                stream: "fixture.beta".into(),
                                                rows: (0..3)
                                                    .map(|index| {
                                                        support::ClockRow {
                                                            now: Some(Timestamp {
                                                                seconds: (window.sequence + offset) as i64 * 10 + index,
                                                                nanos: 0,
                                                            }),
                                                        }
                                                        .encode_to_vec()
                                                    })
                                                    .collect(),
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
    let fixture = Fixture::start(false, false, 0).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let reading = host.clock.read();
    let origin = reading.instant;
    let start: Timestamp = reading.date.into();
    let start_micros = start.seconds * 1_000_000 + i64::from(start.nanos).div_euclid(1000);
    let watches = [host.watch("fixture.alpha"), host.watch("fixture.beta"), host.watch("fixture.silent")];
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    stored(&host, 16).await;
    assert_eq!(fixture.ticks.lock().unwrap().get(""), Some(&4));
    let values = host.values();
    {
        let rows = host.stored.lock().unwrap();
        for (id, per_tick) in [("fixture.alpha", 1), ("fixture.beta", 3), ("fixture.silent", 0)] {
            let actual = rows
                .iter()
                .filter(|(definition, _)| definition.id() == id)
                .map(|(_, row)| row.published_unix_micros.unwrap())
                .collect::<Vec<_>>();
            let expected = (1..=4).flat_map(|tick| iter::repeat_n(start_micros + tick * 1_000_000, per_tick)).collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
        let beta = rows
            .iter()
            .zip(values)
            .filter(|((definition, _), _)| definition.id() == "fixture.beta")
            .map(|(_, value)| value["now"]["seconds"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(beta, (1..=4).flat_map(|tick| (0..3).map(move |index| tick * 10 + index)).collect::<Vec<_>>());
    }
    drop(watches);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_handler_failure_discards_the_whole_unanswered_window() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(true, false, 0).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let origin = host.clock.read().instant;
    let watches = [host.watch("fixture.alpha"), host.watch("fixture.beta")];
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
    let fixture = Fixture::start(false, false, 0).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let (first_clock, first_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let (second_clock, second_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let first = host.runtime.bind_grpc(&attached.activation().unwrap(), "first", first_clock, &CallContext::default()).await.unwrap();
    let second = host.runtime.bind_grpc(&attached.activation().unwrap(), "second", second_clock, &CallContext::default()).await.unwrap();
    first.start_samples(["fixture.alpha"]).await.unwrap();
    second.start_samples(["fixture.beta"]).await.unwrap();
    first_time.advance_by(Duration::from_secs(4)).await;
    stored(&host, 4).await;
    assert_eq!(*fixture.ticks.lock().unwrap(), BTreeMap::from([("first".into(), 4)]));
    second_time.advance_by(Duration::from_secs(4)).await;
    stored(&host, 16).await;
    assert_eq!(*fixture.ticks.lock().unwrap(), BTreeMap::from([("first".into(), 4), ("second".into(), 4)]));
    assert!(host.stored.lock().unwrap().iter().all(|(definition, row)| {
        let scope = serde_json::from_str::<Value>(row.context.as_ref().unwrap()).unwrap();
        (definition.id() == "fixture.alpha" && scope["scope"] == "first")
            || (definition.id() == "fixture.beta" && scope["scope"] == "second")
    }));
    first.close().await.unwrap();
    second.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn entirely_silent_collection_windows_complete_without_publication() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, true, 0).await;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings()).await.unwrap();
    let watching = host.watch("fixture.silent");
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
    let fixture = Fixture::start(false, false, 0).await;
    let settings = PlacementSettings {
        interval_window: Duration::from_secs(8),
        ..settings()
    };
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings).await.unwrap();
    let watching = host.watch("fixture.alpha");
    let origin = host.clock.read().instant;
    host.clock.registered_at(origin + Duration::from_secs(4)).await;
    host.clock.advance_by(Duration::from_secs(4));
    stored(&host, 4).await;
    assert_eq!(fixture.ticks.lock().unwrap().get(""), Some(&4));
    assert!(host.stored.lock().unwrap().iter().all(|(definition, _)| definition.id() == "fixture.alpha"));
    drop(watching);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_rows_preserve_submicrosecond_tick_times() {
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false, 10).await;
    let settings = PlacementSettings {
        interval_window: Duration::from_nanos(30),
        ..settings()
    };
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings).await.unwrap();
    let watches = [host.watch("fixture.alpha"), host.watch("fixture.beta")];
    let reading = host.clock.read();
    let origin = reading.instant;
    host.clock.registered_at(origin + Duration::from_nanos(30)).await;
    host.clock.advance_by(Duration::from_nanos(30));
    stored(&host, 12).await;
    {
        let rows = host.stored.lock().unwrap();
        for (id, per_tick) in [("fixture.alpha", 1), ("fixture.beta", 3)] {
            let times = rows
                .iter()
                .filter(|(definition, _)| definition.id() == id)
                .map(|(_, row)| {
                    let context = serde_json::from_str::<Value>(row.context.as_ref().unwrap()).unwrap();
                    Timestamp {
                        seconds: context["source_time"]["seconds"].as_i64().unwrap(),
                        nanos: context["source_time"]["nanos"].as_i64().unwrap() as i32,
                    }
                })
                .collect::<Vec<_>>();
            let expected = (1..=3)
                .flat_map(|tick| {
                    let start: Timestamp = reading.date.into();
                    let nanos = i128::from(start.seconds) * 1_000_000_000 + i128::from(start.nanos) + tick * 10;
                    let at = Timestamp {
                        seconds: nanos.div_euclid(1_000_000_000) as i64,
                        nanos: nanos.rem_euclid(1_000_000_000) as i32,
                    };
                    iter::repeat_n(at, per_tick)
                })
                .collect::<Vec<_>>();
            assert_eq!(times, expected);
        }
    }
    drop(watches);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn collection_backpressure() {
    // Both Scopes Share the Same Publication Writers and the Real Storage Feed.
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false, 0).await;
    let settings = PlacementSettings {
        stream_window: NonZeroUsize::new(1).unwrap(),
        ..settings()
    };
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings).await.unwrap();
    let (first_clock, first_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let (second_clock, second_time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let activation = attached.activation().unwrap();
    let first = host.runtime.bind_grpc(&activation, "first", first_clock, &CallContext::default()).await.unwrap();
    let second = host.runtime.bind_grpc(&activation, "second", second_clock, &CallContext::default()).await.unwrap();
    first.start_samples(["alpha", "beta"]).await.unwrap();
    second.start_samples(["alpha", "beta"]).await.unwrap();

    // Hold Ingest Before Letting Each Burst Exceed the Retained Window.
    for window in 1..=2 {
        host.signals.storage_hold.store(true, Ordering::SeqCst);
        let advancing = async {
            tokio::join!(first_time.advance_by(Duration::from_secs(4)), second_time.advance_by(Duration::from_secs(4)));
        };
        tokio::pin!(advancing);
        tokio::select! {
            () = &mut advancing => panic!("Keep the Burst Pending Until Ingest Advances"),
            () = host.signals.storage_entered.notified() => {},
        }
        assert!(pending(advancing.as_mut()).await);
        assert!(attached.status().borrow().failure.is_none());
        host.signals.storage_release.notify_one();
        advancing.await;
        assert_eq!(host.rows().len(), window * 32);
    }

    // Every Stream Retains Its Per-Scope Order Across Both Windows, Including Every Row of Each Burst.
    let values = host.values();
    {
        let rows = host.stored.lock().unwrap();
        for scope in ["first", "second"] {
            for id in ["alpha", "beta"] {
                let actual = rows
                    .iter()
                    .zip(&values)
                    .filter(|((definition, row), _)| {
                        let context = serde_json::from_str::<Value>(row.context.as_ref().unwrap()).unwrap();
                        definition.id() == id && context["scope"] == scope
                    })
                    .map(|(_, value)| value["now"]["seconds"].as_i64().unwrap())
                    .collect::<Vec<_>>();
                let expected = if id == "alpha" {
                    (1..=8).collect::<Vec<_>>()
                } else {
                    (1..=8).flat_map(|tick| (0..3).map(move |index| tick * 10 + index)).collect::<Vec<_>>()
                };
                assert_eq!(actual, expected);
            }
        }
    }

    // Successful Settlement Proves Neither Publication Ended on Transient Backlog.
    first.stop_samples().await.unwrap();
    second.stop_samples().await.unwrap();
    first.close().await.unwrap();
    second.close().await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancel_backpressured_collection() {
    // A One-Row Window Cannot Admit the Whole Collection While Storage Is Held.
    let host = HostFixture::new().await;
    let fixture = Fixture::start(false, false, 0).await;
    let settings = PlacementSettings {
        stream_window: NonZeroUsize::new(1).unwrap(),
        ..settings()
    };
    let stop_deadline = host.clock.read().instant + settings.stop_grace;
    let attached = host.runtime.attach_grpc(Placement::Remote(fixture.remote.address.clone()), settings).await.unwrap();
    let (clock, time) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = host.runtime.bind_grpc(&attached.activation().unwrap(), "first", clock, &CallContext::default()).await.unwrap();
    binding.start_samples(["alpha", "beta"]).await.unwrap();
    host.signals.storage_hold.store(true, Ordering::SeqCst);
    let advancing = time.advance_by(Duration::from_secs(4));
    tokio::pin!(advancing);
    tokio::select! {
        () = &mut advancing => panic!("Hold Accepted Rows Until Ingest Advances"),
        () = host.signals.storage_entered.notified() => {},
    }

    // Cancellation Drains Accepted Receipts Without Waiting for the Rest of the Burst.
    let stopping = binding.close();
    tokio::pin!(stopping);
    tokio::select! {
        result = &mut stopping => panic!("Drain Accepted Rows Before Settling: {result:?}"),
        () = host.clock.registered_at(stop_deadline) => {},
    }
    assert!(pending(stopping.as_mut()).await);
    host.signals.storage_release.notify_one();
    stopping.await.unwrap();
    advancing.await;
    let values = host.values();
    assert!(!values.is_empty());
    assert!(values.len() < 16, "Cancel the Unadmitted Rows");
    assert_eq!(values[0]["now"]["seconds"], 1);
    assert!(attached.status().borrow().failure.is_none());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
