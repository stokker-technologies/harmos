/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../support/mod.rs"]
mod support;

#[path = "../streams/allocation.rs"]
mod allocation;

#[path = "retention.rs"]
mod retention;

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
#[path = "../streams/listeners.rs"]
mod listeners;

#[path = "series.rs"]
mod series;

#[path = "stream_batches.rs"]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
pub mod stream_batches;

#[path = "streams_macros.rs"]
mod streams_macros;

mod colliding {
    use serde::{Deserialize, Serialize};

    #[harmos_runtime::stream(id = "pulse", window = 1)]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct Imposter;

    #[harmos_runtime::streams]
    pub enum Bank {
        Imposter(Imposter),
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos_runtime::{
    Runtime, state::At, state::Position, storage::Batch, storage::Cursor, storage::StreamConsumer, streams::BeneathWindow,
    streams::Definition, streams::Item, streams::Stream, streams::StreamManager,
};
use support::{Bank, Ledger, Pulse, Reading, deposit, pulse, streaming, streams};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Selecting;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StreamConsumer<Bank> for Selecting {
    fn resume(&self) -> Cursor {
        Cursor::ORIGIN
    }

    fn selections(&self) -> Vec<Definition> {
        vec![Pulse::definition()]
    }

    async fn drain(&mut self, batch: Batch<Bank>) -> Cursor {
        batch.reaches()
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn next<S: Clone + 'static>(watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin)) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rows_gaps_account() {
    let runtime = streaming().await;
    let mut live = runtime.streams.watch::<Pulse>();
    let streams = runtime.streams.clone();

    // A bare-thread publisher races an actively polling watcher. However the
    // two interleave, the invariant is total accounting: every published row
    // is either delivered or inside an exact gap count.
    let published: u64 = 50_000;
    let publisher = std::thread::spawn(move || {
        for sample in 1..=published {
            streams.publish(pulse(sample));
        }
    });

    let mut delivered: u64 = 0;
    let mut missed: u64 = 0;
    let mut last = 0;
    while delivered + missed < published {
        match next(&mut live).await.expect("the watch is live") {
            Item::Row(At { value, .. }) => {
                assert!(value.sample > last, "delivery is in publish order with no repeats: {} then {}", last, value.sample);
                last = value.sample;
                delivered += 1;
            }
            Item::Gap { missed: count } => missed += count,
        }
    }

    publisher.join().expect("the publisher panics nowhere");
    assert_eq!(delivered + missed, published, "no row is lost silently: what was not delivered was counted");
    assert_eq!(last, published, "the newest row is always delivered");

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn watch_delivery_settles() {
    {
        let runtime = streaming().await;
        // A watcher that makes no progress at all while publishing runs, and a
        // snapshot taken before it: zero consumer progress of any kind.
        let mut stalled = runtime.streams.watch::<Pulse>();
        let held: Vec<At<Pulse>> = runtime.streams.query::<Pulse>(..).await.expect("nothing was evicted yet").collect();
        let streams = runtime.streams.clone();

        // A bare OS thread with no async runtime underneath: a publish that
        // parked, waited on capacity, or required polling could never complete
        // here. Totality under overrun is the whole claim, so no timing is read.
        let published: u64 = 10_000;
        let publisher = std::thread::spawn(move || {
            for sample in 1..=published {
                streams.publish(pulse(sample));
            }
        });
        publisher.join().expect("publish is sync and infallible");

        assert!(held.is_empty(), "the earlier snapshot is unmoved by publishes");

        // The stalled watcher, polled only now, accounts for every row: the exact
        // gap, then the newest window in order.
        let window = Pulse::WINDOW as u64;
        assert_eq!(next(&mut stalled).await, Some(Item::Gap { missed: published - window }));
        for sample in (published - window + 1)..=published {
            assert!(matches!(
                next(&mut stalled).await,
                Some(Item::Row(At { value, .. })) if value == pulse(sample)
            ));
        }

        runtime.shutdown().await.unwrap();
    }
    {
        let runtime = streaming().await;
        let mut live = runtime.streams.watch::<Pulse>();

        runtime.streams.publish(pulse(1));
        runtime.streams.publish(pulse(2));
        runtime.shutdown().await.unwrap();

        assert!(matches!(next(&mut live).await, Some(Item::Row(At { value, .. })) if value == pulse(1)));
        assert!(matches!(next(&mut live).await, Some(Item::Row(At { value, .. })) if value == pulse(2)));
        assert_eq!(next(&mut live).await, None, "every row published before the stop is delivered, then the watch ends");
    }
}

#[tokio::test]
async fn history_respects_cuts() {
    {
        let runtime = streaming().await;

        // Advance the journal between publishes so stamps genuinely move.
        for sample in 1..=3u64 {
            runtime.state.commit(deposit(1)).await.expect("accepted");
            runtime.streams.publish(pulse(sample));
        }

        let complete: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .await
            .expect("a window that never evicted answers any range completely")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(complete, vec![1, 2, 3]);

        // Nine more pushes the first rows out of the window of eight.
        for sample in 4..=12u64 {
            runtime.state.commit(deposit(1)).await.expect("accepted");
            runtime.streams.publish(pulse(sample));
        }

        assert_eq!(
            runtime.streams.query::<Pulse>(..).await.expect_err("an unbounded start reaches beneath the window"),
            BeneathWindow::Unserved,
            "with no series adapter attached the window is the whole served history"
        );
        assert_eq!(
            runtime.streams.query::<Pulse>(Position::ORIGIN..).await.expect_err("the origin lies beneath the floor"),
            BeneathWindow::Unserved,
        );

        // A start the caller actually holds — the stamp of a row still retained —
        // is answerable, and completely so.
        let retained: Vec<u64> = runtime
            .streams
            .query::<Pulse>(Position(5)..)
            .await
            .expect("the range starts at a retained stamp above the floor")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(retained, vec![5, 6, 7, 8, 9, 10, 11, 12]);

        runtime.shutdown().await.unwrap();
    }
    {
        let runtime = streaming().await;

        let first = runtime.state.commit(deposit(10)).await.expect("accepted");
        // The journal now sits idle while rows keep flowing: two publishes at
        // different wall-clock moments carry the same unchanged stamp.
        runtime.streams.publish(pulse(1));
        runtime.streams.publish(pulse(2));

        let second = runtime.state.commit(deposit(20)).await.expect("accepted");
        runtime.streams.publish(pulse(3));

        let cut: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .as_of(first.position)
            .await
            .expect("nothing was evicted")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(
            cut,
            vec![1, 2],
            "later wall-clock rows under the unchanged stamp witnessed state no \
             newer than the cutoff, so both are included and the later row is not"
        );

        let all: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .as_of(second.position)
            .await
            .expect("nothing was evicted")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(all, vec![1, 2, 3]);

        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn demand_tracks_consumers() {
    {
        let runtime = streaming().await;
        let demand = runtime.streams.demand::<Pulse>();

        assert_eq!(*demand.borrow(), 0, "an unobserved stream has zero demand");

        let first = runtime.streams.watch::<Pulse>();
        let second = runtime.streams.watch::<Pulse>();
        let other = runtime.streams.watch::<Reading>();

        assert_eq!(*demand.borrow(), 2);
        assert_eq!(*runtime.streams.demand::<Reading>().borrow(), 1, "demand is per stream: the other feed's watcher is not this one's");

        drop(first);
        assert_eq!(*demand.borrow(), 1);
        drop(second);
        drop(other);
        assert_eq!(*demand.borrow(), 0, "demand falls back to zero on drop");

        runtime.shutdown().await.unwrap();
    }
    {
        let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register(streams());
            setup.settings().attach_streams(Selecting);
        })
        .await
        .expect("the catalog and attachment are honorable");
        let pulse_demand = runtime.streams.demand::<Pulse>();
        let reading_demand = runtime.streams.demand::<Reading>();

        assert_eq!(*pulse_demand.borrow(), 1, "the attached consumer counts for the stream it selected");
        assert_eq!(*reading_demand.borrow(), 0, "selection is per definition, not per facility");

        runtime.shutdown().await.unwrap();

        assert_eq!(*pulse_demand.borrow(), 0, "the consumer leaves demand when its supervisor joins");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn demand_drives_capture() {
    let runtime = streaming().await;
    let capture = Arc::new(AtomicBool::new(false));

    // The capture loop from the spec, verbatim in shape: an application task
    // that turns its source on while anyone is looking and off when nobody is.
    // The acknowledgement channel stands in for the hardware call, so the test
    // observes transitions instead of sleeping.
    let (acknowledged, mut acknowledgements) = tokio::sync::watch::channel(false);
    let mut demand = runtime.streams.demand::<Pulse>();
    let looped = {
        let capture = Arc::clone(&capture);
        tokio::spawn(async move {
            while demand.changed().await.is_ok() {
                if *demand.borrow() > 0 {
                    capture.store(true, Ordering::Release);
                    acknowledged.send_replace(true);
                } else {
                    capture.store(false, Ordering::Release);
                    acknowledged.send_replace(false);
                }
            }
        })
    };

    let watching = runtime.streams.watch::<Pulse>();
    while !*acknowledgements.borrow_and_update() {
        acknowledgements.changed().await.expect("the loop is alive");
    }
    assert!(capture.load(Ordering::Acquire), "a watcher appearing turns capture on");

    drop(watching);
    while *acknowledgements.borrow_and_update() {
        acknowledgements.changed().await.expect("the loop is alive");
    }
    assert!(!capture.load(Ordering::Acquire), "the last watcher leaving turns capture off");

    // The loop ends when the facility is gone entirely — no lifecycle control
    // was ever exercised by the substrate itself. Stopping ends delivery; it is
    // dropping the last `Streams` handle that closes the demand channel, and
    // the runtime holds one for as long as it is alive.
    runtime.shutdown().await.unwrap();
    drop(runtime);
    looped.await.expect("the capture loop ends without panic");
}

#[tokio::test]
async fn definitions_refuse_invalid() {
    {
        let refused = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register(streams());
            setup.settings().register(colliding::Bank::catalog());
        })
        .await;

        let Err(refused) = refused else {
            panic!("two definitions share an id; the boot cannot honor both");
        };
        assert!(refused.to_string().contains("pulse"), "the refusal names the colliding id: {refused}");
    }
    {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
        struct Hollow;

        impl Stream<Bank> for Hollow {
            const ID: &'static str = "hollow";
            const VERSION: u32 = 1;
            const WINDOW: usize = 0;
        }

        let refused = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register([Hollow::definition()]);
        })
        .await;

        let Err(refused) = refused else {
            panic!("a window of zero rows retains nothing and cannot be honored");
        };
        assert!(refused.to_string().contains("hollow"), "the refusal names the stream it cannot honor: {refused}");
    }
}

#[allow(dead_code)]
fn registration_takes_definitions(catalog: [Definition; 2], streams: &StreamManager<Bank>) {
    let _ = catalog;
    let _ = streams;
}

/* === Tests === */
