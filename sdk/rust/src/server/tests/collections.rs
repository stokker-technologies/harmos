/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Collection Authoring and Listener Lifecycle Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

// Third Party Imports
use futures_util::stream;
use prost_types::{Duration, Timestamp};
use tokio::{
    sync::{Notify, mpsc},
    task::{self, LocalSet},
};
use tonic::transport::Channel;
use tonic::{Code, Streaming};

// Local Imports
use crate::server::{self, Job};
use crate::{
    Error, Message,
    plugin::{Collection, CollectionOutput, Context, Observer, Plugin, Stream},
};
use harmos_protocol::plugin::{
    self, InstanceSamplesRequest, InstanceSamplesResponse, instance_client::InstanceClient, instance_samples_request,
    instance_samples_response,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Message)]
#[harmos(package = "fixture", file = "fixture/columns.proto")]
struct Reading {
    value: u64,
}

struct Released(Arc<AtomicUsize>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Released {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn demand(output: &mpsc::Sender<InstanceSamplesRequest>, input: &mut Streaming<InstanceSamplesResponse>, streams: &[&str]) {
    output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Demand(plugin::CollectionDemand {
                streams: streams.iter().map(|stream| (*stream).to_owned()).collect(),
            })),
        })
        .await
        .unwrap();
    let frame = input.message().await.unwrap().unwrap();
    assert!(matches!(frame.frame, Some(instance_samples_response::Frame::Demand(_))));
}

fn grant(count: u64) -> InstanceSamplesRequest {
    InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Window(plugin::Window {
            start: Some(Timestamp { seconds: 1, nanos: 0 }),
            period: Some(Duration { seconds: 1, nanos: 0 }),
            count,
            sequence: 1,
            capturing_executions: Vec::new(),
        })),
    }
}

async fn open(
    client: &mut InstanceClient<Channel>, scope: &str,
) -> (mpsc::Sender<InstanceSamplesRequest>, Streaming<InstanceSamplesResponse>) {
    let (output, receiver) = mpsc::channel(1);
    output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Open(plugin::SamplesSelection {
                stream: "fixture.alpha".into(),
                scope: scope.into(),
            })),
        })
        .await
        .unwrap();
    let input = stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) });
    let input = client.samples(input).await.unwrap().into_inner();
    (output, input)
}

/* === Tests === */

#[tokio::test]
async fn ordinary_observer_hooks_follow_first_and_last_demand_and_restart() {
    // Register Resource Acquisition and Cleanup on an Ordinary Observer.
    let starts = Arc::new(AtomicUsize::new(0));
    let stops = Arc::new(AtomicUsize::new(0));
    let stopping = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let cleaned = Arc::new(Notify::new());
    let readings = Stream::<Reading>::new("fixture.alpha");
    let observer = Observer::new(&readings, async |_: &Context| Ok(Reading { value: 7 }))
        .every(Duration { seconds: 1, nanos: 0 })
        .on_start({
            let starts = starts.clone();
            async move |_: &Context| {
                starts.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .on_stop({
            let stops = stops.clone();
            let stopping = stopping.clone();
            let release = release.clone();
            let cleaned = cleaned.clone();
            async move |_: &Context| {
                if stops.load(Ordering::SeqCst) == 1 {
                    stopping.notify_one();
                    release.notified().await;
                }
                if stops.fetch_add(1, Ordering::SeqCst) == 1 {
                    cleaned.notify_one();
                }
                Ok(())
            }
        });
    let plugin = Plugin::new("fixture", "1.0.0")
        .register(|ctx| {
            ctx.register(readings)?;
            ctx.register(observer)
        })
        .unwrap();
    let mut listener = plugin.start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (first, mut first_input) = open(&mut client, "first").await;
    let (second, mut second_input) = open(&mut client, "second").await;
    demand(&first, &mut first_input, &["fixture.alpha"]).await;
    demand(&second, &mut second_input, &["fixture.alpha"]).await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    // Answer a Window, Then Release Each Demand Independently.
    first.send(grant(1)).await.unwrap();
    let frame = first_input.message().await.unwrap().unwrap();
    assert!(matches!(frame.frame, Some(instance_samples_response::Frame::Batch(_))));
    demand(&first, &mut first_input, &[]).await;
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    demand(&second, &mut second_input, &[]).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);

    // A New Period Acquires Again and Carrier Closure Releases It.
    demand(&first, &mut first_input, &["fixture.alpha"]).await;
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    drop(first);
    drop(second);
    drop(first_input);
    drop(second_input);
    stopping.notified().await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    release.notify_one();
    cleaned.notified().await;
    listener.close().await.unwrap();
    assert_eq!(stops.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn collections_publish_zero_and_many_rows_and_share_first_last_hooks() {
    let alpha = Stream::<Reading>::new("fixture.alpha");
    let beta = Stream::<Reading>::new("fixture.beta");
    let silent = Stream::<Reading>::new("fixture.silent");
    let calls = Arc::new(AtomicUsize::new(0));
    let starts = Arc::new(AtomicUsize::new(0));
    let stops = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation({
        let alpha = alpha.clone();
        let beta = beta.clone();
        let calls = calls.clone();
        async move |_: &Context, out: CollectionOutput| {
            let value = calls.fetch_add(1, Ordering::SeqCst) as u64;
            out.publish(&alpha, Reading { value });
            for value in 0..3 {
                out.publish(&beta, Reading { value });
            }
            Ok(())
        }
    })
    .stream(alpha)
    .stream(beta)
    .stream(silent)
    .every(Duration { seconds: 1, nanos: 0 })
    .on_start({
        let starts = starts.clone();
        async move |_: &Context| {
            starts.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    })
    .on_stop({
        let stops = stops.clone();
        async move |_: &Context| {
            stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });
    let plugin = Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap();
    let declarations = plugin.declarations().unwrap();
    assert!(declarations.streams.iter().all(|stream| stream.collection == "fixture.alpha"));
    let mut listener = plugin.start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (first_output, mut first_input) = open(&mut client, "first").await;
    let (second_output, mut second_input) = open(&mut client, "second").await;
    assert_eq!(starts.load(Ordering::SeqCst), 0);
    demand(&first_output, &mut first_input, &["fixture.alpha", "fixture.beta"]).await;
    demand(&second_output, &mut second_input, &["fixture.silent"]).await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    first_output.send(grant(3)).await.unwrap();
    for offset in 0..3 {
        let frame = first_input.message().await.unwrap().unwrap();
        let Some(instance_samples_response::Frame::Collection(batch)) = frame.frame else {
            panic!("Expected Collection Tick");
        };
        assert_eq!((batch.offset, batch.last), (offset, offset == 2));
        let rows: HashMap<_, _> = batch.streams.into_iter().map(|group| (group.stream, group.rows.len())).collect();
        assert_eq!(rows, HashMap::from([("fixture.alpha".into(), 1), ("fixture.beta".into(), 3)]));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    demand(&first_output, &mut first_input, &[]).await;
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    demand(&second_output, &mut second_input, &[]).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    demand(&first_output, &mut first_input, &["fixture.alpha"]).await;
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    drop(first_output);
    assert!(first_input.message().await.unwrap().is_none());
    assert_eq!(stops.load(Ordering::SeqCst), 2);
    drop(second_output);
    assert!(second_input.message().await.unwrap().is_none());
    listener.close().await.unwrap();
}

#[tokio::test]
async fn ending_collection_demand_settles_the_handler_before_last_stop() {
    let entered = Arc::new(Notify::new());
    let released = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation({
        let entered = entered.clone();
        let released = released.clone();
        async move |_: &Context, _: CollectionOutput| {
            let _released = Released(released.clone());
            entered.notify_one();
            future::pending::<()>().await;
            Ok(())
        }
    })
    .stream(Stream::<Reading>::new("fixture.alpha"))
    .every(Duration { seconds: 1, nanos: 0 })
    .on_stop({
        let released = released.clone();
        let stopped = stopped.clone();
        async move |_: &Context| {
            assert_eq!(released.load(Ordering::SeqCst), 1);
            stopped.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });
    let mut listener =
        Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap().start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (output, mut input) = open(&mut client, "first").await;
    demand(&output, &mut input, &["fixture.alpha"]).await;
    output.send(grant(1)).await.unwrap();
    entered.notified().await;
    demand(&output, &mut input, &[]).await;
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
    drop(output);
    assert!(input.message().await.unwrap().is_none());
    listener.close().await.unwrap();
}

#[tokio::test]
async fn failed_collection_start_does_not_acquire_a_demand_share() {
    let starts = Arc::new(AtomicUsize::new(0));
    let stops = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation(async |_: &Context, _: CollectionOutput| Ok(()))
        .stream(Stream::<Reading>::new("fixture.alpha"))
        .every(Duration { seconds: 1, nanos: 0 })
        .on_start({
            let starts = starts.clone();
            async move |_: &Context| {
                if starts.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(Error::Invalid("Open Device: connect the device".into()));
                }
                Ok(())
            }
        })
        .on_stop({
            let stops = stops.clone();
            async move |_: &Context| {
                stops.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
    let mut listener =
        Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap().start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (output, mut input) = open(&mut client, "first").await;
    output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Demand(plugin::CollectionDemand { streams: vec!["fixture.alpha".into()] })),
        })
        .await
        .unwrap();
    assert_eq!(input.message().await.unwrap_err().code(), Code::Internal);
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    drop(output);
    let (output, mut input) = open(&mut client, "second").await;
    demand(&output, &mut input, &["fixture.alpha"]).await;
    output.send(grant(2)).await.unwrap();
    for offset in 0..2 {
        let frame = input.message().await.unwrap().unwrap();
        let Some(instance_samples_response::Frame::Collection(batch)) = frame.frame else {
            panic!("Expected Empty Tick");
        };
        assert_eq!(batch.offset, offset);
        assert!(batch.streams.is_empty());
    }
    drop(output);
    assert!(input.message().await.unwrap().is_none());
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    listener.close().await.unwrap();
}

#[tokio::test]
async fn failed_collection_stop_releases_its_share_once_and_allows_new_demand() {
    let stops = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation(async |_: &Context, _: CollectionOutput| Ok(()))
        .stream(Stream::<Reading>::new("fixture.alpha"))
        .every(Duration { seconds: 1, nanos: 0 })
        .on_stop({
            let stops = stops.clone();
            async move |_: &Context| {
                if stops.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(Error::Invalid("Close Device: inspect the connection".into()));
                }
                Ok(())
            }
        });
    let mut listener =
        Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap().start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (output, mut input) = open(&mut client, "first").await;
    demand(&output, &mut input, &["fixture.alpha"]).await;
    output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Demand(plugin::CollectionDemand { streams: vec![] })),
        })
        .await
        .unwrap();
    assert_eq!(input.message().await.unwrap_err().code(), Code::Internal);
    drop(output);
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    let (output, mut input) = open(&mut client, "second").await;
    demand(&output, &mut input, &["fixture.alpha"]).await;
    drop(output);
    assert!(input.message().await.unwrap().is_none());
    assert_eq!(stops.load(Ordering::SeqCst), 2);
    listener.close().await.unwrap();
}

#[tokio::test]
async fn collection_output_refuses_unregistered_members_and_still_runs_cleanup() {
    let stopped = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation(async |_: &Context, output: CollectionOutput| {
        output.publish(&Stream::<Reading>::new("fixture.missing"), Reading { value: 1 });
        Ok(())
    })
    .stream(Stream::<Reading>::new("fixture.alpha"))
    .every(Duration { seconds: 1, nanos: 0 })
    .on_stop({
        let stopped = stopped.clone();
        async move |_: &Context| {
            stopped.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });
    let mut listener =
        Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap().start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (output, mut input) = open(&mut client, "first").await;
    demand(&output, &mut input, &["fixture.alpha"]).await;
    output.send(grant(1)).await.unwrap();
    assert_eq!(input.message().await.unwrap_err().code(), Code::InvalidArgument);
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
    drop(output);
    listener.close().await.unwrap();
}

#[tokio::test]
async fn closing_demand_during_acquisition_cancels_and_settles_the_start_hook() {
    let entered = Arc::new(Notify::new());
    let released = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicUsize::new(0));
    let collection = Collection::operation(async |_: &Context, _: CollectionOutput| Ok(()))
        .stream(Stream::<Reading>::new("fixture.alpha"))
        .every(Duration { seconds: 1, nanos: 0 })
        .on_start({
            let entered = entered.clone();
            let released = released.clone();
            async move |_: &Context| {
                let _released = Released(released.clone());
                entered.notify_one();
                future::pending::<()>().await;
                Ok(())
            }
        })
        .on_stop({
            let stopped = stopped.clone();
            async move |_: &Context| {
                stopped.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
    let mut listener =
        Plugin::new("fixture", "1.0.0").register(|ctx| ctx.register(collection)).unwrap().start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    let (output, mut input) = open(&mut client, "first").await;
    output
        .send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Demand(plugin::CollectionDemand { streams: vec!["fixture.alpha".into()] })),
        })
        .await
        .unwrap();
    entered.notified().await;
    drop(output);
    assert_eq!(input.message().await.unwrap_err().code(), Code::Cancelled);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(stopped.load(Ordering::SeqCst), 0);
    listener.close().await.unwrap();
}

#[tokio::test]
async fn completed_acquisition_is_released_when_its_reply_is_lost() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let acquired = Arc::new(AtomicUsize::new(0));
            let stopped = Arc::new(AtomicUsize::new(0));
            let collection = Collection::operation(async |_: &Context, _: CollectionOutput| Ok(()))
                .stream(Stream::<Reading>::new("fixture.alpha"))
                .every(Duration { seconds: 1, nanos: 0 })
                .on_start({
                    let acquired = acquired.clone();
                    async move |_: &Context| {
                        acquired.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })
                .on_stop({
                    let acquired = acquired.clone();
                    let stopped = stopped.clone();
                    async move |_: &Context| {
                        assert_eq!(acquired.fetch_sub(1, Ordering::SeqCst), 1);
                        stopped.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                });
            let (jobs, mut input) = mpsc::channel::<Job>(1);
            let executor = task::spawn_local(async move {
                while let Some(Job {
                    handler,
                    context,
                    payload,
                    started,
                    mut complete,
                    settled,
                }) = input.recv().await
                {
                    let result = handler.execute(&context, payload).await;
                    let _ = started.send(());
                    // Withhold Completion Until Cancellation Has Closed Its Receiver.
                    complete.closed().await;
                    let _ = complete.send(result);
                    let _ = settled.send(());
                }
            });
            let (output, receiver) = mpsc::channel(1);
            drop(receiver);
            let context = Context::new(plugin::Request::default(), "fixture".into(), Arc::new(HashMap::new()));
            let lifecycle = &collection.observer.lifecycle;
            server::transition(&jobs, lifecycle, &context, true, None, &output).await.unwrap();
            assert_eq!(acquired.load(Ordering::SeqCst), 1);
            assert_eq!(*lifecycle.active.lock().await, 1);
            let result = server::transition(&jobs, lifecycle, &context, false, None, &output).await;
            result.unwrap();
            assert_eq!(acquired.load(Ordering::SeqCst), 0);
            assert_eq!(stopped.load(Ordering::SeqCst), 1);
            assert_eq!(*lifecycle.active.lock().await, 0);
            drop(jobs);
            executor.await.unwrap();
        })
        .await;
}
